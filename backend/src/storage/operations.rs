use std::sync::{Arc, Mutex};

use crate::btree::BTree;
use crate::storage::error::DatabaseError;
use crate::storage::mvcc::VersionedValue;
use crate::storage::{buffer_pool::BufferPool, value::Value};

use super::transaction::{Snapshot, Transaction, TransactionId};
use super::wal::LogRecord;

pub fn insert(
    txn: &mut Transaction,
    btree: &Arc<Mutex<BTree>>,
    buffer_pool: &mut BufferPool,
    key: i32,
    value: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut btree = btree.lock().unwrap();

    let versioned_value = VersionedValue::new(value.clone(), txn.id);

    match btree.insert(key, vec![versioned_value], buffer_pool) {
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

    // Get existing versions
    let existing_versions = btree.search(key, buffer_pool);

    let new_versions = match existing_versions {
        Ok(Some(mut versions)) => {
            // Mark the currently visible version as deleted
            let mut found = false;
            for version in &mut versions {
                if version.is_visible_to(txn.id, &[]) {
                    version.mark_deleted(txn.id);
                    found = true;
                    break;
                }
            }

            if !found {
                return Err(DatabaseError::KeyNotFound(key).into());
            }

            versions
        }
        Ok(None) => {
            return Err(DatabaseError::KeyNotFound(key).into());
        }
        Err(e) => {
            eprintln!("Error deleting key {}: {}", key, e);
            return Err(Box::new(e));
        }
    };

    match btree.update(key, new_versions, buffer_pool) {
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
    snapshot: Option<&Snapshot>,
    aborted_txns: &[TransactionId],
) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    let btree = btree.lock().unwrap();
    match btree.search(key, buffer_pool) {
        Ok(Some(versions)) => {
            if let Some(snapshot) = snapshot {
                let active_txns: Vec<TransactionId> =
                    snapshot.active_txns.iter().copied().collect();
                for version in versions.iter().rev() {
                    if aborted_txns.contains(&TransactionId(version.xmin)) {
                        continue;
                    }
                    if version.is_visible_to(snapshot.txn_id, &active_txns) {
                        return Ok(Some(version.value.clone()));
                    }
                }
                Ok(None)
            } else {
                for version in versions.iter().rev() {
                    if aborted_txns.contains(&TransactionId(version.xmin)) {
                        continue;
                    }
                    if !version.is_deleted() || aborted_txns.contains(&TransactionId(version.xmax))
                    {
                        return Ok(Some(version.value.clone()));
                    }
                }
                Ok(None)
            }
        }
        Ok(None) => Ok(None),
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

    // Get existing versions
    let existing_versions = btree.search(key, buffer_pool);

    let new_versions = match existing_versions {
        Ok(Some(mut versions)) => {
            // Mark the currently visible version as deleted
            for version in &mut versions {
                if version.is_visible_to(txn.id, &[]) {
                    version.mark_deleted(txn.id);
                    break;
                }
            }

            // Create new version
            let new_version = VersionedValue::new(value.clone(), txn.id);
            versions.push(new_version);
            versions
        }
        Ok(None) => {
            // Key doesn't exist, just insert it
            vec![VersionedValue::new(value.clone(), txn.id)]
        }
        Err(e) => {
            eprintln!("Error updating key {}: {}", key, e);
            return Err(Box::new(e));
        }
    };

    match btree.update(key, new_versions, buffer_pool) {
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
    snapshot: Option<&Snapshot>,
    aborted_txns: &[TransactionId],
) -> Result<Vec<(i32, Value)>, Box<dyn std::error::Error>> {
    let btree = btree.lock().unwrap();
    let all_entries = btree.all(buffer_pool)?;

    let mut result = Vec::new();
    for (key, versions) in all_entries {
        if let Some(snapshot) = snapshot {
            let active_txns: Vec<TransactionId> = snapshot.active_txns.iter().copied().collect();
            for version in versions.iter().rev() {
                if aborted_txns.contains(&TransactionId(version.xmin)) {
                    continue;
                }
                if version.is_visible_to(snapshot.txn_id, &active_txns) {
                    result.push((key, version.value.clone()));
                    break;
                }
            }
        } else {
            for version in versions.iter().rev() {
                if aborted_txns.contains(&TransactionId(version.xmin)) {
                    continue;
                }
                if !version.is_deleted() || aborted_txns.contains(&TransactionId(version.xmax)) {
                    result.push((key, version.value.clone()));
                    break;
                }
            }
        }
    }

    Ok(result)
}

pub fn strlen(
    btree: &Arc<Mutex<BTree>>,
    buffer_pool: &mut BufferPool,
    key: i32,
    snapshot: Option<&Snapshot>,
    aborted_txns: &[TransactionId],
) -> Result<Option<usize>, Box<dyn std::error::Error>> {
    match get(btree, buffer_pool, key, snapshot, aborted_txns)? {
        Some(value) => Ok(Some(value.to_string().len())),
        None => Err(DatabaseError::KeyNotFound(key).into()),
    }
}

pub fn strcat(
    txn: &mut Transaction,
    btree: &Arc<Mutex<BTree>>,
    buffer_pool: &mut BufferPool,
    key: i32,
    value: &Value,
    aborted_txns: &[TransactionId],
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = txn.get_snapshot();
    match get(btree, buffer_pool, key, snapshot, aborted_txns)? {
        Some(old_value) => {
            let concatenated = old_value.add(value)?;
            update(txn, btree, buffer_pool, key, &concatenated)?;
            Ok(())
        }
        None => Err(DatabaseError::KeyNotFound(key).into()),
    }
}

pub fn substr(
    txn: &mut Transaction,
    btree: &Arc<Mutex<BTree>>,
    buffer_pool: &mut BufferPool,
    key: i32,
    start: usize,
    length: usize,
    aborted_txns: &[TransactionId],
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = txn.get_snapshot();
    match get(btree, buffer_pool, key, snapshot, aborted_txns)? {
        Some(value) => {
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
        None => Err(DatabaseError::KeyNotFound(key).into()),
    }
}
