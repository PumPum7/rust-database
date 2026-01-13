#[cfg(test)]
mod tests {
    use crate::database_handler::database_handler::{Database, IsolationLevel};
    use crate::storage::value::Value;
    use std::fs;

    #[test]
    fn test_multi_statement_transaction_commit() -> Result<(), Box<dyn std::error::Error>> {
        let test_db_path = "test_multi_txn_commit.db";
        let _ = fs::remove_file(test_db_path);
        let _ = fs::remove_file(format!("{}.wal", test_db_path.replace(".db", "")));

        let mut db = Database::new(test_db_path)?;

        let txn_id = db.begin_transaction(IsolationLevel::ReadCommitted)?;

        db.insert_in_transaction(txn_id, 1, &Value::Integer(100))?;
        db.insert_in_transaction(txn_id, 2, &Value::Integer(200))?;
        db.insert_in_transaction(txn_id, 3, &Value::Integer(300))?;

        db.commit_transaction(txn_id)?;

        assert_eq!(db.get(1)?, Some(Value::Integer(100)));
        assert_eq!(db.get(2)?, Some(Value::Integer(200)));
        assert_eq!(db.get(3)?, Some(Value::Integer(300)));

        fs::remove_file(test_db_path)?;
        let _ = fs::remove_file(format!("{}.wal", test_db_path.replace(".db", "")));
        Ok(())
    }

    #[test]
    fn test_multi_statement_transaction_rollback() -> Result<(), Box<dyn std::error::Error>> {
        let test_db_path = "test_multi_txn_rollback.db";
        let _ = fs::remove_file(test_db_path);
        let _ = fs::remove_file(format!("{}.wal", test_db_path.replace(".db", "")));

        let mut db = Database::new(test_db_path)?;

        db.insert(10, &Value::Integer(1000))?;

        let txn_id = db.begin_transaction(IsolationLevel::ReadCommitted)?;

        db.insert_in_transaction(txn_id, 1, &Value::Integer(100))?;
        db.insert_in_transaction(txn_id, 2, &Value::Integer(200))?;

        db.rollback_transaction(txn_id)?;

        assert_eq!(db.get(10)?, Some(Value::Integer(1000)));

        fs::remove_file(test_db_path)?;
        let _ = fs::remove_file(format!("{}.wal", test_db_path.replace(".db", "")));
        Ok(())
    }

    #[test]
    fn test_checkpoint_clears_wal() -> Result<(), Box<dyn std::error::Error>> {
        let test_db_path = "test_checkpoint.db";
        let wal_path = "test_checkpoint.wal";
        let _ = fs::remove_file(test_db_path);
        let _ = fs::remove_file(wal_path);

        let mut db = Database::new(test_db_path)?;

        db.insert(1, &Value::Integer(100))?;
        db.insert(2, &Value::Integer(200))?;

        db.checkpoint()?;

        let wal_metadata = fs::metadata(wal_path)?;
        assert_eq!(wal_metadata.len(), 0);

        assert_eq!(db.get(1)?, Some(Value::Integer(100)));
        assert_eq!(db.get(2)?, Some(Value::Integer(200)));

        fs::remove_file(test_db_path)?;
        fs::remove_file(wal_path)?;
        Ok(())
    }

    #[test]
    fn test_transaction_isolation_basic() -> Result<(), Box<dyn std::error::Error>> {
        let test_db_path = "test_isolation.db";
        let _ = fs::remove_file(test_db_path);
        let _ = fs::remove_file(format!("{}.wal", test_db_path.replace(".db", "")));

        let mut db = Database::new(test_db_path)?;

        db.insert(1, &Value::Integer(50))?;

        let txn_id = db.begin_transaction(IsolationLevel::ReadCommitted)?;
        db.update_in_transaction(txn_id, 1, &Value::Integer(100))?;
        db.commit_transaction(txn_id)?;

        assert_eq!(db.get(1)?, Some(Value::Integer(100)));

        fs::remove_file(test_db_path)?;
        let _ = fs::remove_file(format!("{}.wal", test_db_path.replace(".db", "")));
        Ok(())
    }

    #[test]
    fn test_persistence_after_reopen() -> Result<(), Box<dyn std::error::Error>> {
        let test_db_path = "test_persistence.db";
        let wal_path = "test_persistence.wal";
        let _ = fs::remove_file(test_db_path);
        let _ = fs::remove_file(wal_path);

        {
            let mut db = Database::new(test_db_path)?;
            db.insert(1, &Value::Integer(100))?;
            db.insert(2, &Value::String("test".to_string()))?;
            db.checkpoint()?;
        }

        {
            let mut db = Database::new(test_db_path)?;
            assert_eq!(db.get(1)?, Some(Value::Integer(100)));
            assert_eq!(db.get(2)?, Some(Value::String("test".to_string())));
        }

        fs::remove_file(test_db_path)?;
        let _ = fs::remove_file(wal_path);
        Ok(())
    }
}
