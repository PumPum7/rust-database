use super::error::Result;
use crate::storage::wal::{LogRecord, WriteAheadLog};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionId {
    xmin: TransactionId, // Transaction that created this version
    xmax: TransactionId, // Transaction that deleted this version (or 0 if still visible)
}

impl VersionId {
    pub fn new(xmin: TransactionId) -> Self {
        Self {
            xmin,
            xmax: TransactionId(0),
        }
    }

    pub fn is_visible_to(&self, txn_id: TransactionId) -> bool {
        // Version created by an earlier transaction and not deleted
        // or deleted by a later transaction
        self.xmin < txn_id && (self.xmax.0 == 0 || self.xmax > txn_id)
    }
}

pub struct Transaction {
    pub id: TransactionId,
    pub is_active: bool,
    pub wal: Option<Arc<Mutex<WriteAheadLog>>>,
}

pub struct TransactionManager {
    next_txn_id: AtomicU64,
    active_txns: Mutex<Vec<TransactionId>>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            next_txn_id: AtomicU64::new(1),
            active_txns: Mutex::new(Vec::new()),
        }
    }

    pub fn begin_transaction(&self, wal: Arc<Mutex<WriteAheadLog>>) -> Result<Transaction> {
        let txn_id = TransactionId(self.next_txn_id.fetch_add(1, Ordering::SeqCst));

        // Log the begin transaction record
        wal.lock().unwrap().log(LogRecord::Begin(txn_id.0))?;

        let mut active_txns = self.active_txns.lock().unwrap();
        active_txns.push(txn_id);

        Ok(Transaction {
            id: txn_id,
            is_active: true,
            wal: Some(wal),
        })
    }
}

impl Transaction {
    pub fn commit(&mut self) -> Result<()> {
        if !self.is_active {
            return Ok(());
        }
        if let Some(wal) = &self.wal {
            wal.lock().unwrap().log(LogRecord::Commit(self.id.0))?;
        }
        self.is_active = false;
        // Remove from active transactions list would go here
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        if !self.is_active {
            return Ok(());
        }
        if let Some(wal) = &self.wal {
            wal.lock().unwrap().log(LogRecord::Rollback(self.id.0))?;
        }
        self.is_active = false;
        Ok(())
    }

    pub fn id(&self) -> TransactionId {
        self.id
    }
}
