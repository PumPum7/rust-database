use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use crate::{
    btree::BTree,
    storage::{buffer_pool::BufferPool, value::Value},
    storage::{
        disk_manager::DiskManager,
        error::DatabaseError,
        operations,
        transaction::{Transaction, TransactionManager},
        wal::WriteAheadLog,
    },
};

pub struct Database {
    buffer_pool: BufferPool,
    transaction_manager: TransactionManager,
    index: Arc<Mutex<BTree>>,
    wal: Arc<Mutex<WriteAheadLog>>,
}

impl Database {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let file_exists = path.exists();
        let disk_manager = DiskManager::new(path.to_str().unwrap())
            .map_err(|_| DatabaseError::IoError(std::io::Error::last_os_error()))?;
        let mut buffer_pool = BufferPool::new(1000, disk_manager);
        let wal = WriteAheadLog::new(path.with_extension("wal"))
            .map_err(|_| DatabaseError::IoError(std::io::Error::last_os_error()))?;

        let root_page_id = if !file_exists {
            // Create and initialize root page for index if this is a new database
            let page_id = buffer_pool.new_page()?.header.page_id;
            let btree = BTree::new(page_id);
            // Initialize the B-tree with an empty root nodes
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
            index: Arc::new(Mutex::new(btree)),
            wal: Arc::new(Mutex::new(wal)),
        })
    }

    pub fn begin_transaction(&mut self) -> Result<Transaction, Box<dyn std::error::Error>> {
        Ok(self
            .transaction_manager
            .begin_transaction(Arc::clone(&self.wal))
            .map_err(|_| DatabaseError::IoError(std::io::Error::last_os_error()))?)
    }

    pub fn insert(&mut self, key: i32, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let mut txn = self.begin_transaction()?;
        let result = operations::insert(&mut txn, &self.index, &mut self.buffer_pool, key, value);
        if result.is_ok() {
            txn.commit()?;
        } else {
            txn.rollback()?;
        }
        result
    }

    pub fn delete(&mut self, key: i32) -> Result<(), Box<dyn std::error::Error>> {
        let mut txn = self.begin_transaction()?;
        let result = operations::delete(&mut txn, &self.index, &mut self.buffer_pool, key);
        if result.is_ok() {
            txn.commit()?;
        } else {
            txn.rollback()?;
        }
        result
    }

    pub fn update(&mut self, key: i32, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let mut txn = self.begin_transaction()?;
        let result = operations::update(&mut txn, &self.index, &mut self.buffer_pool, key, value);
        if result.is_ok() {
            txn.commit()?;
        } else {
            txn.rollback()?;
        }
        result
    }

    pub fn get(&mut self, key: i32) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        operations::get(&self.index, &mut self.buffer_pool, key)
    }

    pub fn all(&mut self) -> Result<Vec<(i32, Value)>, Box<dyn std::error::Error>> {
        operations::all(&self.index, &mut self.buffer_pool)
    }

    pub fn strlen(&mut self, key: i32) -> Result<Option<usize>, Box<dyn std::error::Error>> {
        operations::strlen(&self.index, &mut self.buffer_pool, key)
    }

    pub fn strcat(&mut self, key: i32, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let mut txn = self.begin_transaction()?;
        let result = operations::strcat(&mut txn, &self.index, &mut self.buffer_pool, key, value);
        if result.is_ok() {
            txn.commit()?;
        } else {
            txn.rollback()?;
        }
        result
    }

    pub fn substr(
        &mut self,
        key: i32,
        start: usize,
        length: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut txn = self.begin_transaction()?;
        let result = operations::substr(
            &mut txn,
            &self.index,
            &mut self.buffer_pool,
            key,
            start,
            length,
        );
        if result.is_ok() {
            txn.commit()?;
        } else {
            txn.rollback()?;
        }
        result
    }

    pub fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.buffer_pool
            .flush()
            .map_err(|_| DatabaseError::IoError(std::io::Error::last_os_error()))?;
        Ok(())
    }
}
