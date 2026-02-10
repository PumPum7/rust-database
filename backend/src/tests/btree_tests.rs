#[cfg(test)]
mod tests {
    use crate::btree::BTree;
    use crate::storage::buffer_pool::BufferPool;
    use crate::storage::disk_manager::DiskManager;
    use crate::storage::error::Result;
    use crate::storage::mvcc::VersionedValue;
    use crate::storage::value::Value;
    use crate::storage::wal::WriteAheadLog;
    use crate::storage::TransactionId;
    use std::sync::{Arc, Mutex};

    fn assert_version_matches(
        versions: &Option<Vec<VersionedValue>>,
        expected_value: &Value,
        expected_xmin: u64,
        expected_xmax: u64,
    ) {
        assert!(versions.is_some(), "Expected Some, got None");
        let versions = versions.as_ref().unwrap();
        assert_eq!(versions.len(), 1, "Expected 1 version");
        assert_eq!(&versions[0].value, expected_value, "Value mismatch");
        assert_eq!(versions[0].xmin, expected_xmin, "xmin mismatch");
        assert_eq!(versions[0].xmax, expected_xmax, "xmax mismatch");
    }

    #[test]
    fn test_btree_operations() -> Result<()> {
        let disk_manager = DiskManager::new("test_btree.db")?;
        let wal = Arc::new(Mutex::new(WriteAheadLog::new("test_btree.wal")?));
        let mut buffer_pool = BufferPool::new(100, disk_manager, Arc::clone(&wal));
        let root_page_id = buffer_pool.new_page()?.header.page_id;
        let mut btree = BTree::new(root_page_id);

        btree.init(&mut buffer_pool)?;

        let txn_id = TransactionId(1);
        btree.insert(
            5,
            vec![VersionedValue::new(Value::Integer(50), txn_id)],
            &mut buffer_pool,
        )?;
        btree.insert(
            3,
            vec![VersionedValue::new(Value::Boolean(false), txn_id)],
            &mut buffer_pool,
        )?;
        btree.insert(
            7,
            vec![VersionedValue::new(
                Value::String("Test".to_string()),
                txn_id,
            )],
            &mut buffer_pool,
        )?;

        assert_version_matches(
            &btree.search(5, &mut buffer_pool)?,
            &Value::Integer(50),
            1,
            0,
        );
        assert_version_matches(
            &btree.search(3, &mut buffer_pool)?,
            &Value::Boolean(false),
            1,
            0,
        );
        assert_version_matches(
            &btree.search(7, &mut buffer_pool)?,
            &Value::String("Test".to_string()),
            1,
            0,
        );

        std::fs::remove_file("test_btree.db")?;
        let _ = std::fs::remove_file("test_btree.wal");

        Ok(())
    }

    #[test]
    fn test_btree_delete() -> Result<()> {
        let disk_manager = DiskManager::new("test_btree_delete.db")?;
        let wal = Arc::new(Mutex::new(WriteAheadLog::new("test_btree_delete.wal")?));
        let mut buffer_pool = BufferPool::new(100, disk_manager, Arc::clone(&wal));
        let root_page_id = buffer_pool.new_page()?.header.page_id;
        let mut btree = BTree::new(root_page_id);

        btree.init(&mut buffer_pool)?;

        let txn_id = TransactionId(1);
        btree.insert(
            10,
            vec![VersionedValue::new(Value::Integer(100), txn_id)],
            &mut buffer_pool,
        )?;
        btree.insert(
            20,
            vec![VersionedValue::new(Value::Integer(200), txn_id)],
            &mut buffer_pool,
        )?;
        btree.insert(
            30,
            vec![VersionedValue::new(Value::Integer(300), txn_id)],
            &mut buffer_pool,
        )?;

        btree.delete(20, &mut buffer_pool)?;

        assert!(btree.search(20, &mut buffer_pool)?.is_none());
        assert_version_matches(
            &btree.search(10, &mut buffer_pool)?,
            &Value::Integer(100),
            1,
            0,
        );
        assert_version_matches(
            &btree.search(30, &mut buffer_pool)?,
            &Value::Integer(300),
            1,
            0,
        );

        std::fs::remove_file("test_btree_delete.db")?;
        let _ = std::fs::remove_file("test_btree_delete.wal");

        Ok(())
    }

    #[test]
    fn test_btree_update() -> Result<()> {
        let disk_manager = DiskManager::new("test_btree_update.db")?;
        let wal = Arc::new(Mutex::new(WriteAheadLog::new("test_btree_update.wal")?));
        let mut buffer_pool = BufferPool::new(100, disk_manager, Arc::clone(&wal));
        let root_page_id = buffer_pool.new_page()?.header.page_id;
        let mut btree = BTree::new(root_page_id);

        btree.init(&mut buffer_pool)?;

        let txn_id = TransactionId(1);
        btree.insert(
            5,
            vec![VersionedValue::new(
                Value::String("five".to_string()),
                txn_id,
            )],
            &mut buffer_pool,
        )?;

        btree.delete(5, &mut buffer_pool)?;

        btree.insert(
            5,
            vec![VersionedValue::new(
                Value::String("FIVE".to_string()),
                txn_id,
            )],
            &mut buffer_pool,
        )?;

        assert_version_matches(
            &btree.search(5, &mut buffer_pool)?,
            &Value::String("FIVE".to_string()),
            1,
            0,
        );

        std::fs::remove_file("test_btree_update.db")?;
        let _ = std::fs::remove_file("test_btree_update.wal");

        Ok(())
    }

    #[test]
    fn test_big_btree() -> Result<()> {
        let disk_manager = DiskManager::new("test_big_btree.db")?;
        let wal = Arc::new(Mutex::new(WriteAheadLog::new("test_big_btree.wal")?));
        let mut buffer_pool = BufferPool::new(1000, disk_manager, Arc::clone(&wal));
        let root_page_id = buffer_pool.new_page()?.header.page_id;
        let mut btree = BTree::new(root_page_id);

        btree.init(&mut buffer_pool)?;

        let txn_id = TransactionId(1);
        for i in 0..1000 {
            btree.insert(
                i,
                vec![VersionedValue::new(Value::Integer(i as i64), txn_id)],
                &mut buffer_pool,
            )?;
        }

        for i in 0..1000 {
            let result = btree.search(i, &mut buffer_pool)?;
            assert!(result.is_some(), "Key {} should exist", i);
            let versions = result.unwrap();
            assert_eq!(versions.len(), 1, "Key {} should have 1 version", i);
            assert_eq!(
                versions[0].value,
                Value::Integer(i as i64),
                "Key {} should have correct value",
                i
            );
            assert_eq!(
                versions[0].xmin, txn_id.0,
                "Key {} should have correct xmin",
                i
            );
            assert_eq!(versions[0].xmax, 0, "Key {} should not be deleted", i);
        }

        for i in 0..1000 {
            btree.delete(i, &mut buffer_pool)?;
        }

        assert!(
            btree.search(0, &mut buffer_pool)?.is_none(),
            "Testing deletion"
        );

        // Cleanup
        std::fs::remove_file("test_big_btree.db")?;
        let _ = std::fs::remove_file("test_big_btree.wal");

        Ok(())
    }

    #[test]
    fn test_btree_range_queries() -> Result<()> {
        let disk_manager = DiskManager::new("test_btree_range.db")?;
        let wal = Arc::new(Mutex::new(WriteAheadLog::new("test_btree_range.wal")?));
        let mut buffer_pool = BufferPool::new(100, disk_manager, Arc::clone(&wal));
        let root_page_id = buffer_pool.new_page()?.header.page_id;
        let mut btree = BTree::new(root_page_id);

        btree.init(&mut buffer_pool)?;

        let txn_id = TransactionId(1);
        // Insert some test data
        btree.insert(
            10,
            vec![VersionedValue::new(Value::Integer(100), txn_id)],
            &mut buffer_pool,
        )?;
        btree.insert(
            20,
            vec![VersionedValue::new(Value::Integer(200), txn_id)],
            &mut buffer_pool,
        )?;
        btree.insert(
            30,
            vec![VersionedValue::new(Value::Integer(300), txn_id)],
            &mut buffer_pool,
        )?;
        btree.insert(
            40,
            vec![VersionedValue::new(Value::Integer(400), txn_id)],
            &mut buffer_pool,
        )?;
        btree.insert(
            50,
            vec![VersionedValue::new(Value::Integer(500), txn_id)],
            &mut buffer_pool,
        )?;

        // Test GT (greater than)
        let gt_results = btree.range_gt(25, &mut buffer_pool)?;
        assert_eq!(gt_results.len(), 3);
        assert_eq!(gt_results[0].0, 30);
        assert_eq!(gt_results[1].0, 40);
        assert_eq!(gt_results[2].0, 50);

        // Test LT (less than)
        let lt_results = btree.range_lt(35, &mut buffer_pool)?;
        assert_eq!(lt_results.len(), 3);
        assert_eq!(lt_results[0].0, 10);
        assert_eq!(lt_results[1].0, 20);
        assert_eq!(lt_results[2].0, 30);

        // Test GTE (greater than or equal)
        let gte_results = btree.range_gte(30, &mut buffer_pool)?;
        assert_eq!(gte_results.len(), 3);
        assert_eq!(gte_results[0].0, 30);
        assert_eq!(gte_results[1].0, 40);
        assert_eq!(gte_results[2].0, 50);

        // Test LTE (less than or equal)
        let lte_results = btree.range_lte(30, &mut buffer_pool)?;
        assert_eq!(lte_results.len(), 3);
        assert_eq!(lte_results[0].0, 10);
        assert_eq!(lte_results[1].0, 20);
        assert_eq!(lte_results[2].0, 30);

        // Test BETWEEN
        let between_results = btree.range_between(20, 40, &mut buffer_pool)?;
        assert_eq!(between_results.len(), 3);
        assert_eq!(between_results[0].0, 20);
        assert_eq!(between_results[1].0, 30);
        assert_eq!(between_results[2].0, 40);

        // Test edge cases
        let gt_all = btree.range_gt(50, &mut buffer_pool)?;
        assert_eq!(gt_all.len(), 0);

        let lt_all = btree.range_lt(10, &mut buffer_pool)?;
        assert_eq!(lt_all.len(), 0);

        let between_empty = btree.range_between(5, 8, &mut buffer_pool)?;
        assert_eq!(between_empty.len(), 0);

        // Test inverted range (start > end) - should return empty
        let inverted_range = btree.range_between(50, 10, &mut buffer_pool)?;
        assert_eq!(inverted_range.len(), 0);

        // Cleanup
        std::fs::remove_file("test_btree_range.db")?;
        let _ = std::fs::remove_file("test_btree_range.wal");

        Ok(())
    }
}
