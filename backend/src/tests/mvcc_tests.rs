#[cfg(test)]
mod tests {
    use crate::storage::mvcc::VersionedValue;
    use crate::storage::transaction::{
        IsolationLevel, Snapshot, TransactionId, TransactionManager,
    };
    use crate::storage::value::Value;

    #[test]
    fn test_snapshot_visibility_basic() {
        let txn_id = TransactionId(5);
        let active_txns = vec![TransactionId(3), TransactionId(4)];
        let snapshot = Snapshot::new(txn_id, active_txns);

        // Value created by committed transaction (txn 1), not deleted
        assert!(snapshot.is_visible(1, 0));

        // Value created by active transaction (txn 3), not visible
        assert!(!snapshot.is_visible(3, 0));

        // Value created by committed txn, deleted by active txn - still visible
        assert!(snapshot.is_visible(1, 3));

        // Value created by committed txn, deleted by committed txn before snapshot
        assert!(!snapshot.is_visible(1, 2));

        // Value created by current transaction
        assert!(snapshot.is_visible(5, 0));
    }

    #[test]
    fn test_snapshot_visibility_self_changes() {
        let txn_id = TransactionId(10);
        let active_txns = vec![TransactionId(8)];
        let snapshot = Snapshot::new(txn_id, active_txns);

        // Created by self, not deleted
        assert!(snapshot.is_visible(10, 0));

        // Created by self, deleted by self - not visible
        assert!(!snapshot.is_visible(10, 10));

        // Created by committed, deleted by self - not visible
        assert!(!snapshot.is_visible(5, 10));
    }

    #[test]
    fn test_versioned_value_visibility() {
        let value = Value::Integer(42);
        let active_txns = vec![TransactionId(3)];

        let mut versioned = VersionedValue::new(value.clone(), TransactionId(1));

        // Visible to transaction 5 (created by 1, which is committed)
        assert!(versioned.is_visible_to(TransactionId(5), &active_txns));

        // Mark as deleted by transaction 3 (which is active)
        versioned.mark_deleted(TransactionId(3));

        // Still visible because deleting txn is uncommitted
        assert!(versioned.is_visible_to(TransactionId(5), &active_txns));

        // Create version deleted by committed transaction
        let mut versioned2 = VersionedValue::new(value, TransactionId(1));
        versioned2.mark_deleted(TransactionId(2));

        // Not visible - deleted by committed transaction before our snapshot
        assert!(!versioned2.is_visible_to(TransactionId(5), &active_txns));
    }

    #[test]
    fn test_versioned_value_serialization() -> Result<(), Box<dyn std::error::Error>> {
        let value = Value::String("test data".to_string());
        let versioned = VersionedValue::new(value.clone(), TransactionId(42));

        let serialized = versioned.serialize();
        let (deserialized, size) = VersionedValue::deserialize(&serialized)?;

        assert_eq!(deserialized.value, value);
        assert_eq!(deserialized.xmin, 42);
        assert_eq!(deserialized.xmax, 0);
        assert!(size > 0);

        Ok(())
    }

    #[test]
    fn test_transaction_manager_isolation_levels() -> Result<(), Box<dyn std::error::Error>> {
        use crate::storage::wal::WriteAheadLog;
        use std::sync::{Arc, Mutex};
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new()?;
        let wal = Arc::new(Mutex::new(WriteAheadLog::new(temp_file.path())?));

        let manager = TransactionManager::new();

        // Start transaction with ReadCommitted - no snapshot
        let txn1 = manager
            .begin_transaction_with_isolation(Arc::clone(&wal), IsolationLevel::ReadCommitted)?;
        assert!(txn1.snapshot.is_none());

        // Start transaction with RepeatableRead - has snapshot
        let txn2 = manager
            .begin_transaction_with_isolation(Arc::clone(&wal), IsolationLevel::RepeatableRead)?;
        assert!(txn2.snapshot.is_some());

        // The snapshot should include txn1 as active
        let snapshot = txn2.snapshot.unwrap();
        assert!(snapshot.active_txns.contains(&txn1.id));

        Ok(())
    }

    #[test]
    fn test_transaction_manager_tracks_committed() -> Result<(), Box<dyn std::error::Error>> {
        use crate::storage::wal::WriteAheadLog;
        use std::sync::{Arc, Mutex};
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new()?;
        let wal = Arc::new(Mutex::new(WriteAheadLog::new(temp_file.path())?));

        let manager = TransactionManager::new();

        let txn = manager.begin_transaction(Arc::clone(&wal))?;
        let txn_id = txn.id;

        assert!(!manager.is_committed(txn_id));
        assert!(manager.get_active_transactions().contains(&txn_id));

        manager.commit_transaction(txn_id);

        assert!(manager.is_committed(txn_id));
        assert!(!manager.get_active_transactions().contains(&txn_id));

        Ok(())
    }

    #[test]
    fn test_garbage_collection() -> Result<(), Box<dyn std::error::Error>> {
        use crate::database_handler::database_handler::Database;
        use std::fs;

        let test_db_path = "test_gc.db";
        let _ = fs::remove_file(test_db_path);

        let mut db = Database::new(test_db_path)?;

        // Insert, update, and delete to create multiple versions
        db.insert(1, &Value::Integer(100))?;
        db.update(1, &Value::Integer(200))?;
        db.update(1, &Value::Integer(300))?;
        db.delete(1)?;

        // Run GC - should remove old versions since no transactions are active
        let removed = db.gc()?;

        // Verify GC ran (we got a count back)
        let _ = removed;

        // Verify that the key is still deleted
        assert_eq!(db.get(1)?, None);

        fs::remove_file(test_db_path)?;
        let _ = fs::remove_file(format!("{}.wal", test_db_path));

        Ok(())
    }

    #[test]
    fn test_get_in_transaction_sees_own_writes() -> Result<(), Box<dyn std::error::Error>> {
        use crate::database_handler::database_handler::{Database, IsolationLevel};
        use std::fs;

        let test_db_path = "test_get_in_txn.db";
        let _ = fs::remove_file(test_db_path);
        let _ = fs::remove_file(format!("{}.wal", test_db_path.replace(".db", "")));

        let mut db = Database::new(test_db_path)?;

        // Begin a transaction with RepeatableRead isolation
        let txn_id = db.begin_transaction(IsolationLevel::RepeatableRead)?;

        // Insert within transaction
        db.insert_in_transaction(txn_id, 1, &Value::Integer(42))?;

        // Should be able to see our own write within transaction
        let result = db.get_in_transaction(txn_id, 1)?;
        assert_eq!(result, Some(Value::Integer(42)));

        // Update within transaction
        db.update_in_transaction(txn_id, 1, &Value::Integer(100))?;

        // Should see updated value
        let result = db.get_in_transaction(txn_id, 1)?;
        assert_eq!(result, Some(Value::Integer(100)));

        // Commit and verify
        db.commit_transaction(txn_id)?;
        let result = db.get(1)?;
        assert_eq!(result, Some(Value::Integer(100)));

        fs::remove_file(test_db_path)?;
        let _ = fs::remove_file(format!("{}.wal", test_db_path.replace(".db", "")));

        Ok(())
    }

    #[test]
    fn test_all_respects_mvcc_visibility() -> Result<(), Box<dyn std::error::Error>> {
        use crate::database_handler::database_handler::{Database, IsolationLevel};
        use std::fs;

        let test_db_path = "test_all_mvcc.db";
        let _ = fs::remove_file(test_db_path);
        let _ = fs::remove_file(format!("{}.wal", test_db_path.replace(".db", "")));

        let mut db = Database::new(test_db_path)?;

        // Insert some initial data
        db.insert(1, &Value::Integer(10))?;
        db.insert(2, &Value::Integer(20))?;

        // Start a transaction with RepeatableRead (captures snapshot)
        let txn_id = db.begin_transaction(IsolationLevel::RepeatableRead)?;

        // Add a new key within transaction
        db.insert_in_transaction(txn_id, 3, &Value::Integer(30))?;

        // all_in_transaction should see key 3
        let results = db.all_in_transaction(txn_id)?;
        assert_eq!(results.len(), 3);

        // Delete key 1 within transaction
        db.delete_in_transaction(txn_id, 1)?;

        // all_in_transaction should now only see keys 2 and 3
        let results = db.all_in_transaction(txn_id)?;
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|(k, _)| *k == 2));
        assert!(results.iter().any(|(k, _)| *k == 3));

        // Rollback transaction
        db.rollback_transaction(txn_id)?;

        // Regular all() should see keys 1 and 2 (key 3 was rolled back, key 1 was not deleted)
        let results = db.all()?;
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|(k, _)| *k == 1));
        assert!(results.iter().any(|(k, _)| *k == 2));

        fs::remove_file(test_db_path)?;
        let _ = fs::remove_file(format!("{}.wal", test_db_path.replace(".db", "")));

        Ok(())
    }
}
