pub mod index;
pub mod storage;
mod tests;

use index::BTree;
use std::{path::Path, sync::{Arc, Mutex}};
pub use storage::{BufferPool, DiskManager, Transaction, TransactionManager, Value};
use crate::storage::{LogRecord, WriteAheadLog};

pub struct Database {
    buffer_pool: BufferPool,
    transaction_manager: TransactionManager,
    index: BTree,
    wal: Arc<Mutex<WriteAheadLog>>,
}

impl Database {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let file_exists = path.exists();
        let disk_manager = DiskManager::new(path.to_str().unwrap())?;
        let mut buffer_pool = BufferPool::new(1000, disk_manager);
        let wal = WriteAheadLog::new(path.with_extension("wal"))?;

        let root_page_id = if !file_exists {
            // Create and initialize root page for index if this is a new database
            let page_id = buffer_pool.new_page()?.header.page_id;
            let btree = BTree::new(page_id);
            // Initialize the B-tree with an empty root node
            btree.init(&mut buffer_pool)?;
            page_id
        } else {
            // For existing database, read the first page which contains the root
            let page = buffer_pool.get_page(0)?;
            page.header.page_id
        };

        let btree = BTree::new(root_page_id);

        Ok(Self {
            buffer_pool,
            transaction_manager: TransactionManager::new(),
            index: btree,
            wal: Arc::new(Mutex::new(wal)),
        })
    }

    pub fn begin_transaction(&mut self) -> Result<Transaction, Box<dyn std::error::Error>> {
        Ok(self.transaction_manager.begin_transaction(Arc::clone(&self.wal))?)
    }

    pub fn insert(&mut self, key: i32, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
        // Add error handling and logging for debugging
        match self.index.insert(key, value.clone(), &mut self.buffer_pool) {
            Ok(()) => {
                self.wal.lock().unwrap().log(LogRecord::Write {
                    txn_id: 0,
                    page_id: 0,
                    offset: 0,
                    data: value.serialize(),
                })?;
                Ok(())
            },
            Err(e) => {
                eprintln!("Error inserting key {}: {}", key, e);
                Err(Box::new(e))
            }
        }
    }

    pub fn get(&mut self, key: i32) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        match self.index.search(key, &mut self.buffer_pool) {
            Ok(result) => Ok(result),
            Err(e) => {
                eprintln!("Error searching for key {}: {}", key, e);
                Err(Box::new(e))
            }
        }
    }

    pub fn delete(&mut self, key: i32) -> Result<(), Box<dyn std::error::Error>> {
        match self.index.delete(key, &mut self.buffer_pool) {
            Ok(()) => {
                self.wal.lock().unwrap().log(LogRecord::Write {
                    txn_id: 0,
                    page_id: 0,
                    offset: 0,
                    data: vec![],
                })?;
                Ok(())
            },
            Err(e) => {
                eprintln!("Error deleting key {}: {}", key, e);
                Err(Box::new(e))
            }
        }
    }

    pub fn update(&mut self, key: i32, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
        match self.index.update(key, value.clone(), &mut self.buffer_pool) {
            Ok(()) => {
                self.wal.lock().unwrap().log(LogRecord::Write {
                    txn_id: 0,
                    page_id: 0,
                    offset: 0,
                    data: value.serialize(),
                })?;
                Ok(())
            },
            Err(e) => {
                eprintln!("Error updating key {}: {}", key, e);
                Err(Box::new(e))
            }
        }
    }

    pub fn all(&mut self) -> Result<Vec<(i32, Value)>, Box<dyn std::error::Error>> {
        match self.index.all(&mut self.buffer_pool) {
            Ok(result) => Ok(result),
            Err(e) => {
                eprintln!("Error fetching all keys: {}", e);
                Err(Box::new(e))
            }
        }
    }

    pub fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.buffer_pool.flush()?;
        Ok(())
    }
}
