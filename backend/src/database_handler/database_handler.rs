use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use crate::storage::TransactionId;
use crate::{
    btree::BTree,
    storage::{
        buffer_pool::BufferPool,
        disk_manager::DiskManager,
        error::DatabaseError,
        operations,
        transaction::{Transaction, TransactionManager},
        value::Value,
        wal::WriteAheadLog,
    },
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    ReadCommitted,
    RepeatableRead,
}

impl Default for IsolationLevel {
    fn default() -> Self {
        Self::ReadCommitted
    }
}

pub struct Database {
    buffer_pool: BufferPool,
    transaction_manager: TransactionManager,
    index: Arc<Mutex<BTree>>,
    wal: Arc<Mutex<WriteAheadLog>>,
    active_txns: Arc<Mutex<HashMap<u64, Transaction>>>,
}

impl Database {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let file_exists = path.exists();
        let disk_manager = DiskManager::new(path.to_str().unwrap())
            .map_err(|_| DatabaseError::IoError(std::io::Error::last_os_error()))?;
        let mut buffer_pool = BufferPool::new(1000, disk_manager);
        let mut wal = WriteAheadLog::new(path.with_extension("wal"))
            .map_err(|_| DatabaseError::IoError(std::io::Error::last_os_error()))?;

        let root_page_id = if !file_exists {
            let page_id = buffer_pool.new_page()?.header.page_id;
            let btree = BTree::new(page_id);
            btree.init(&mut buffer_pool)?;
            page_id
        } else {
            wal.recover(&mut buffer_pool)?;
            let page = buffer_pool.get_page(0)?;
            page.header.page_id
        };

        let btree = BTree::new(root_page_id);

        Ok(Self {
            buffer_pool,
            transaction_manager: TransactionManager::new(),
            index: Arc::new(Mutex::new(btree)),
            wal: Arc::new(Mutex::new(wal)),
            active_txns: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn begin_transaction(
        &mut self,
        _isolation: IsolationLevel,
    ) -> Result<TransactionId, Box<dyn std::error::Error>> {
        let txn = self
            .transaction_manager
            .begin_transaction(Arc::clone(&self.wal))
            .map_err(|_| DatabaseError::IoError(std::io::Error::last_os_error()))?;

        let txn_id = txn.id;
        self.active_txns.lock().unwrap().insert(txn_id.0, txn);
        Ok(txn_id)
    }

    pub fn commit_transaction(
        &mut self,
        txn_id: TransactionId,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let txn_opt = {
            let mut active_txns = self.active_txns.lock().unwrap();
            active_txns.remove(&txn_id.0)
        };

        if let Some(mut txn) = txn_opt {
            txn.commit()?;
            self.flush()?;
            Ok(())
        } else {
            Err(Box::new(DatabaseError::TransactionNotActive))
        }
    }

    pub fn rollback_transaction(
        &mut self,
        txn_id: TransactionId,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut active_txns = self.active_txns.lock().unwrap();
        if let Some(mut txn) = active_txns.remove(&txn_id.0) {
            txn.rollback()?;
            Ok(())
        } else {
            Err(Box::new(DatabaseError::TransactionNotActive))
        }
    }

    pub fn get_active_transaction(&self, txn_id: TransactionId) -> Option<Transaction> {
        self.active_txns.lock().unwrap().get(&txn_id.0).cloned()
    }

    pub fn is_transaction_active(&self, txn_id: TransactionId) -> bool {
        self.active_txns.lock().unwrap().contains_key(&txn_id.0)
    }

    pub fn insert(&mut self, key: i32, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let txn_id = self.begin_transaction(IsolationLevel::ReadCommitted)?;
        let mut txn = self
            .get_active_transaction(txn_id)
            .ok_or(DatabaseError::TransactionNotActive)?;
        let result = operations::insert(&mut txn, &self.index, &mut self.buffer_pool, key, value);
        if result.is_ok() {
            self.commit_transaction(txn_id)?;
        } else {
            self.rollback_transaction(txn_id)?;
        }
        result
    }

    pub fn insert_in_transaction(
        &mut self,
        txn_id: TransactionId,
        key: i32,
        value: &Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut txn = self
            .get_active_transaction(txn_id)
            .ok_or(DatabaseError::TransactionNotActive)?;
        operations::insert(&mut txn, &self.index, &mut self.buffer_pool, key, value)?;
        Ok(())
    }

    pub fn delete(&mut self, key: i32) -> Result<(), Box<dyn std::error::Error>> {
        let txn_id = self.begin_transaction(IsolationLevel::ReadCommitted)?;
        let mut txn = self
            .get_active_transaction(txn_id)
            .ok_or(DatabaseError::TransactionNotActive)?;
        let result = operations::delete(&mut txn, &self.index, &mut self.buffer_pool, key);
        if result.is_ok() {
            self.commit_transaction(txn_id)?;
        } else {
            self.rollback_transaction(txn_id)?;
        }
        result
    }

    pub fn delete_in_transaction(
        &mut self,
        txn_id: TransactionId,
        key: i32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut txn = self
            .get_active_transaction(txn_id)
            .ok_or(DatabaseError::TransactionNotActive)?;
        operations::delete(&mut txn, &self.index, &mut self.buffer_pool, key)?;
        Ok(())
    }

    pub fn update(&mut self, key: i32, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let txn_id = self.begin_transaction(IsolationLevel::ReadCommitted)?;
        let mut txn = self
            .get_active_transaction(txn_id)
            .ok_or(DatabaseError::TransactionNotActive)?;
        let result = operations::update(&mut txn, &self.index, &mut self.buffer_pool, key, value);
        if result.is_ok() {
            self.commit_transaction(txn_id)?;
        } else {
            self.rollback_transaction(txn_id)?;
        }
        result
    }

    pub fn update_in_transaction(
        &mut self,
        txn_id: TransactionId,
        key: i32,
        value: &Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut txn = self
            .get_active_transaction(txn_id)
            .ok_or(DatabaseError::TransactionNotActive)?;
        operations::update(&mut txn, &self.index, &mut self.buffer_pool, key, value)?;
        Ok(())
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
        let txn_id = self.begin_transaction(IsolationLevel::ReadCommitted)?;
        let mut txn = self
            .get_active_transaction(txn_id)
            .ok_or(DatabaseError::TransactionNotActive)?;
        let result = operations::strcat(&mut txn, &self.index, &mut self.buffer_pool, key, value);
        if result.is_ok() {
            self.commit_transaction(txn_id)?;
        } else {
            self.rollback_transaction(txn_id)?;
        }
        result
    }

    pub fn strcat_in_transaction(
        &mut self,
        txn_id: TransactionId,
        key: i32,
        value: &Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut txn = self
            .get_active_transaction(txn_id)
            .ok_or(DatabaseError::TransactionNotActive)?;
        operations::strcat(&mut txn, &self.index, &mut self.buffer_pool, key, value)?;
        Ok(())
    }

    pub fn substr(
        &mut self,
        key: i32,
        start: usize,
        length: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let txn_id = self.begin_transaction(IsolationLevel::ReadCommitted)?;
        let mut txn = self
            .get_active_transaction(txn_id)
            .ok_or(DatabaseError::TransactionNotActive)?;
        let result = operations::substr(
            &mut txn,
            &self.index,
            &mut self.buffer_pool,
            key,
            start,
            length,
        );
        if result.is_ok() {
            self.commit_transaction(txn_id)?;
        } else {
            self.rollback_transaction(txn_id)?;
        }
        result
    }

    pub fn substr_in_transaction(
        &mut self,
        txn_id: TransactionId,
        key: i32,
        start: usize,
        length: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut txn = self
            .get_active_transaction(txn_id)
            .ok_or(DatabaseError::TransactionNotActive)?;
        operations::substr(
            &mut txn,
            &self.index,
            &mut self.buffer_pool,
            key,
            start,
            length,
        )?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.buffer_pool
            .flush()
            .map_err(|_| DatabaseError::IoError(std::io::Error::last_os_error()))?;
        Ok(())
    }

    pub fn checkpoint(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.flush()?;
        self.wal.lock().unwrap().truncate()?;
        Ok(())
    }
}
