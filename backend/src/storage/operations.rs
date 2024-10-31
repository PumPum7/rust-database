use std::sync::{Arc, Mutex};

use crate::btree::BTree;
use crate::storage::error::DatabaseError;
use crate::storage::{buffer_pool::BufferPool, value::Value};

use super::transaction::Transaction;
use super::wal::LogRecord;

pub fn insert(
    txn: &mut Transaction,
    btree: &Arc<Mutex<BTree>>,
    buffer_pool: &mut BufferPool,
    key: i32,
    value: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut btree = btree.lock().unwrap();
    match btree.insert(key, value.clone(), buffer_pool) {
        Ok(()) => {
            if let Some(wal) = &txn.wal {
                wal.lock().unwrap().log(LogRecord::Write {
                    txn_id: txn.id.0,
                    page_id: btree.root_page_id(),
                    offset: 0,
                    data: value.serialize(),
                })?;
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Error inserting key {}: {}", key, e);
            Err(Box::new(e))
        }
    }
}

pub fn delete(
    txn: &mut Transaction,
    btree: &Arc<Mutex<BTree>>,
    buffer_pool: &mut BufferPool,
    key: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut btree = btree.lock().unwrap();
    match btree.delete(key, buffer_pool) {
        Ok(()) => {
            if let Some(wal) = &txn.wal {
                wal.lock().unwrap().log(LogRecord::Write {
                    txn_id: txn.id.0,
                    page_id: btree.root_page_id(),
                    offset: 0,
                    data: vec![],
                })?;
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Error deleting key {}: {}", key, e);
            Err(Box::new(e))
        }
    }
}

pub fn get(
    btree: &Arc<Mutex<BTree>>,
    buffer_pool: &mut BufferPool,
    key: i32,
) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    let btree = btree.lock().unwrap();
    match btree.search(key, buffer_pool) {
        Ok(result) => Ok(result),
        Err(e) => {
            eprintln!("Error searching for key {}: {}", key, e);
            Err(Box::new(e))
        }
    }
}

pub fn update(
    txn: &mut Transaction,
    btree: &Arc<Mutex<BTree>>,
    buffer_pool: &mut BufferPool,
    key: i32,
    value: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut btree = btree.lock().unwrap();
    match btree.update(key, value.clone(), buffer_pool) {
        Ok(()) => {
            if let Some(wal) = &txn.wal {
                wal.lock().unwrap().log(LogRecord::Write {
                    txn_id: txn.id.0,
                    page_id: btree.root_page_id(),
                    offset: 0,
                    data: value.serialize(),
                })?;
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Error updating key {}: {}", key, e);
            Err(Box::new(e))
        }
    }
}

pub fn all(
    btree: &Arc<Mutex<BTree>>,
    buffer_pool: &mut BufferPool,
) -> Result<Vec<(i32, Value)>, Box<dyn std::error::Error>> {
    let btree = btree.lock().unwrap();
    btree.all(buffer_pool).map_err(|e| e.into())
}

pub fn strlen(
    btree: &Arc<Mutex<BTree>>,
    buffer_pool: &mut BufferPool,
    key: i32,
) -> Result<Option<usize>, Box<dyn std::error::Error>> {
    let btree = btree.lock().unwrap();
    match btree.search(key, buffer_pool) {
        Ok(Some(value)) => Ok(Some(value.to_string().len())),
        _ => Err(DatabaseError::KeyNotFound(key).into()),
    }
}

pub fn strcat(
    txn: &mut Transaction,
    btree: &Arc<Mutex<BTree>>,
    buffer_pool: &mut BufferPool,
    key: i32,
    value: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let btree_unlocked = btree.lock().unwrap();
    match btree_unlocked.search(key, buffer_pool) {
        Ok(Some(old_value)) => {
            let concatenated = old_value.add(value)?;
            update(txn, btree, buffer_pool, key, &concatenated)?;
            Ok(())
        }
        _ => Err(DatabaseError::KeyNotFound(key).into()),
    }
}

pub fn substr(
    txn: &mut Transaction,
    btree: &Arc<Mutex<BTree>>,
    buffer_pool: &mut BufferPool,
    key: i32,
    start: usize,
    length: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let btree_unlocked = btree.lock().unwrap();
    match btree_unlocked.search(key, buffer_pool) {
        Ok(Some(value)) => {
            let substr = value.to_string();
            let substr = substr.get(start..start + length).unwrap_or("");
            update(
                txn,
                btree,
                buffer_pool,
                key,
                &Value::String(substr.to_string()),
            )?;
            Ok(())
        }
        _ => Err(DatabaseError::KeyNotFound(key).into()),
    }
}
