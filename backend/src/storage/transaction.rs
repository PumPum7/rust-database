use super::error::Result;
use crate::storage::wal::{LogRecord, WriteAheadLog};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransactionId(pub u64);

impl From<u64> for TransactionId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<TransactionId> for u64 {
    fn from(value: TransactionId) -> Self {
        value.0
    }
}

impl std::fmt::Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl Default for IsolationLevel {
    fn default() -> Self {
        Self::ReadCommitted
    }
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub txn_id: TransactionId,
    pub active_txns: Vec<TransactionId>,
    pub xmin: TransactionId,
    pub xmax: TransactionId,
}

impl Snapshot {
    pub fn new(txn_id: TransactionId, active_txns: Vec<TransactionId>) -> Self {
        let xmin = active_txns.iter().min().copied().unwrap_or(txn_id);
        let xmax = txn_id;

        Self {
            txn_id,
            active_txns,
            xmin,
            xmax,
        }
    }

    pub fn is_visible(&self, xmin: u64, xmax: u64) -> bool {
        let xmin_tid = TransactionId(xmin);
        let xmax_tid = TransactionId(xmax);

        let created_before_snapshot = xmin_tid < self.txn_id;
        let created_by_committed = !self.active_txns.contains(&xmin_tid);
        let created_by_self = xmin_tid == self.txn_id;

        let not_deleted = xmax == 0;
        let deleted_after_snapshot = xmax_tid > self.txn_id;
        let deleted_by_uncommitted = self.active_txns.contains(&xmax_tid);
        let deleted_by_self = xmax_tid == self.txn_id;

        let insert_visible = (created_before_snapshot && created_by_committed) || created_by_self;
        let delete_not_visible =
            not_deleted || deleted_after_snapshot || (deleted_by_uncommitted && !deleted_by_self);

        insert_visible && delete_not_visible
    }
}

#[derive(Clone)]
pub struct Transaction {
    pub id: TransactionId,
    pub is_active: bool,
    pub isolation_level: IsolationLevel,
    pub snapshot: Option<Snapshot>,
    pub wal: Option<Arc<Mutex<WriteAheadLog>>>,
}

impl Transaction {
    pub fn commit(&mut self) -> Result<()> {
        if !self.is_active {
            return Ok(());
        }
        if let Some(wal) = &self.wal {
            wal.lock().unwrap().log(LogRecord::Commit {
                lsn: 0,
                txn_id: self.id.0,
            })?;
        }
        self.is_active = false;
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        if !self.is_active {
            return Ok(());
        }
        if let Some(wal) = &self.wal {
            wal.lock().unwrap().log(LogRecord::Rollback {
                lsn: 0,
                txn_id: self.id.0,
            })?;
        }
        self.is_active = false;
        Ok(())
    }

    pub fn id(&self) -> TransactionId {
        self.id
    }

    pub fn get_snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }
}

pub struct TransactionManager {
    next_txn_id: AtomicU64,
    active_txns: Mutex<HashSet<TransactionId>>,
    committed_txns: Mutex<HashSet<TransactionId>>,
    aborted_txns: Mutex<HashSet<TransactionId>>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            next_txn_id: AtomicU64::new(1),
            active_txns: Mutex::new(HashSet::new()),
            committed_txns: Mutex::new(HashSet::new()),
            aborted_txns: Mutex::new(HashSet::new()),
        }
    }

    pub fn begin_transaction(&self, wal: Arc<Mutex<WriteAheadLog>>) -> Result<Transaction> {
        self.begin_transaction_with_isolation(wal, IsolationLevel::ReadCommitted)
    }

    pub fn begin_transaction_with_isolation(
        &self,
        wal: Arc<Mutex<WriteAheadLog>>,
        isolation_level: IsolationLevel,
    ) -> Result<Transaction> {
        let txn_id = TransactionId(self.next_txn_id.fetch_add(1, Ordering::SeqCst));

        wal.lock().unwrap().log(LogRecord::Begin {
            lsn: 0,
            txn_id: txn_id.0,
        })?;

        let mut active_txns = self.active_txns.lock().unwrap();

        let snapshot = match isolation_level {
            IsolationLevel::RepeatableRead | IsolationLevel::Serializable => {
                Some(Snapshot::new(txn_id, active_txns.iter().copied().collect()))
            }
            IsolationLevel::ReadCommitted => None,
        };

        active_txns.insert(txn_id);

        Ok(Transaction {
            id: txn_id,
            is_active: true,
            isolation_level,
            snapshot,
            wal: Some(wal),
        })
    }

    pub fn commit_transaction(&self, txn_id: TransactionId) {
        let mut active = self.active_txns.lock().unwrap();
        active.remove(&txn_id);

        let mut committed = self.committed_txns.lock().unwrap();
        committed.insert(txn_id);
    }

    pub fn abort_transaction(&self, txn_id: TransactionId) {
        let mut active = self.active_txns.lock().unwrap();
        active.remove(&txn_id);

        let mut aborted = self.aborted_txns.lock().unwrap();
        aborted.insert(txn_id);
    }

    pub fn get_active_transactions(&self) -> Vec<TransactionId> {
        self.active_txns.lock().unwrap().iter().copied().collect()
    }

    pub fn is_committed(&self, txn_id: TransactionId) -> bool {
        self.committed_txns.lock().unwrap().contains(&txn_id)
    }

    pub fn is_aborted(&self, txn_id: TransactionId) -> bool {
        self.aborted_txns.lock().unwrap().contains(&txn_id)
    }

    pub fn get_aborted_transactions(&self) -> Vec<TransactionId> {
        self.aborted_txns.lock().unwrap().iter().copied().collect()
    }

    pub fn get_snapshot_for_read_committed(&self, txn_id: TransactionId) -> Snapshot {
        let active = self.active_txns.lock().unwrap();
        Snapshot::new(txn_id, active.iter().copied().collect())
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}
