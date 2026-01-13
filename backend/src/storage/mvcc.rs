use crate::storage::value::Value;
use crate::storage::TransactionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionedValue {
    pub value: Value,
    pub xmin: u64,
    pub xmax: u64,
    pub created_at: u64,
}

impl VersionedValue {
    pub fn new(value: Value, txn_id: TransactionId) -> Self {
        Self {
            value,
            xmin: txn_id.0,
            xmax: 0,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn mark_deleted(&mut self, txn_id: TransactionId) {
        self.xmax = txn_id.0;
    }

    pub fn is_visible_to(&self, txn_id: TransactionId, active_txns: &[TransactionId]) -> bool {
        let xmin_committed =
            self.xmin < txn_id.0 && !active_txns.contains(&TransactionId(self.xmin));
        let xmin_is_self = self.xmin == txn_id.0;

        let not_deleted = self.xmax == 0;
        let deleted_by_future = self.xmax > txn_id.0;
        let deleted_by_uncommitted = active_txns.contains(&TransactionId(self.xmax));

        let created_visible = xmin_committed || xmin_is_self;
        let delete_not_visible = not_deleted || deleted_by_future || deleted_by_uncommitted;

        created_visible && delete_not_visible
    }

    pub fn is_deleted(&self) -> bool {
        self.xmax != 0
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&self.xmin.to_le_bytes());
        buffer.extend_from_slice(&self.xmax.to_le_bytes());
        buffer.extend_from_slice(&self.created_at.to_le_bytes());
        buffer.extend(self.value.serialize());
        buffer
    }

    pub fn deserialize(buffer: &[u8]) -> Result<(Self, usize), Box<dyn std::error::Error>> {
        if buffer.len() < 24 {
            return Err("Buffer too short for VersionedValue header".into());
        }

        let xmin = u64::from_le_bytes(buffer[0..8].try_into()?);
        let xmax = u64::from_le_bytes(buffer[8..16].try_into()?);
        let created_at = u64::from_le_bytes(buffer[16..24].try_into()?);

        let (value, value_size) = Value::deserialize(&buffer[24..])?;

        Ok((
            Self {
                value,
                xmin,
                xmax,
                created_at,
            },
            24 + value_size,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct MvccManager {
    committed_txns: Vec<TransactionId>,
    aborted_txns: Vec<TransactionId>,
}

impl MvccManager {
    pub fn new() -> Self {
        Self {
            committed_txns: Vec::new(),
            aborted_txns: Vec::new(),
        }
    }

    pub fn mark_committed(&mut self, txn_id: TransactionId) {
        if !self.committed_txns.contains(&txn_id) {
            self.committed_txns.push(txn_id);
        }
    }

    pub fn mark_aborted(&mut self, txn_id: TransactionId) {
        if !self.aborted_txns.contains(&txn_id) {
            self.aborted_txns.push(txn_id);
        }
    }

    pub fn is_committed(&self, txn_id: TransactionId) -> bool {
        self.committed_txns.contains(&txn_id)
    }

    pub fn is_aborted(&self, txn_id: TransactionId) -> bool {
        self.aborted_txns.contains(&txn_id)
    }

    pub fn cleanup_old_versions(&mut self, oldest_active_txn: TransactionId) {
        self.committed_txns.retain(|&txn| txn >= oldest_active_txn);
        self.aborted_txns.retain(|&txn| txn >= oldest_active_txn);
    }
}

impl Default for MvccManager {
    fn default() -> Self {
        Self::new()
    }
}
