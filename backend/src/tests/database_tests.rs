#[cfg(test)]
mod tests {
    use std::fs;

    use crate::database_handler::database_handler::Database;
    use crate::storage::value::Value;

    #[test]
    fn test_database_operations() -> Result<(), Box<dyn std::error::Error>> {
        let test_db_path = "test_db_ops.db";
        let _ = fs::remove_file(test_db_path);

        let mut db = Database::new(test_db_path)?;

        db.insert(1, &Value::Integer(100))?;
        assert_eq!(db.get(1)?, Some(Value::Integer(100)));

        db.delete(1)?;
        assert_eq!(db.get(1)?, None);

        fs::remove_file(test_db_path)?;
        let _ = fs::remove_file("test_db_range.wal");
        Ok(())
    }

    #[test]
    fn test_database_range_operations() -> Result<(), Box<dyn std::error::Error>> {
        let test_db_path = "test_db_range.db";
        let _ = fs::remove_file(test_db_path);

        let mut db = Database::new(test_db_path)?;

        // Insert test data
        db.insert(10, &Value::Integer(100))?;
        db.insert(20, &Value::Integer(200))?;
        db.insert(30, &Value::Integer(300))?;
        db.insert(40, &Value::Integer(400))?;
        db.insert(50, &Value::Integer(500))?;

        // Test GT (greater than)
        let gt_results = db.range_gt(25)?;
        assert_eq!(gt_results.len(), 3);
        assert_eq!(gt_results[0].0, 30);
        assert_eq!(gt_results[0].1, Value::Integer(300));
        assert_eq!(gt_results[1].0, 40);
        assert_eq!(gt_results[1].1, Value::Integer(400));
        assert_eq!(gt_results[2].0, 50);
        assert_eq!(gt_results[2].1, Value::Integer(500));

        // Test LT (less than)
        let lt_results = db.range_lt(35)?;
        assert_eq!(lt_results.len(), 3);
        assert_eq!(lt_results[0].0, 10);
        assert_eq!(lt_results[0].1, Value::Integer(100));
        assert_eq!(lt_results[1].0, 20);
        assert_eq!(lt_results[1].1, Value::Integer(200));
        assert_eq!(lt_results[2].0, 30);
        assert_eq!(lt_results[2].1, Value::Integer(300));

        // Test GTE (greater than or equal)
        let gte_results = db.range_gte(30)?;
        assert_eq!(gte_results.len(), 3);
        assert_eq!(gte_results[0].0, 30);
        assert_eq!(gte_results[0].1, Value::Integer(300));
        assert_eq!(gte_results[1].0, 40);
        assert_eq!(gte_results[1].1, Value::Integer(400));
        assert_eq!(gte_results[2].0, 50);
        assert_eq!(gte_results[2].1, Value::Integer(500));

        // Test LTE (less than or equal)
        let lte_results = db.range_lte(30)?;
        assert_eq!(lte_results.len(), 3);
        assert_eq!(lte_results[0].0, 10);
        assert_eq!(lte_results[0].1, Value::Integer(100));
        assert_eq!(lte_results[1].0, 20);
        assert_eq!(lte_results[1].1, Value::Integer(200));
        assert_eq!(lte_results[2].0, 30);
        assert_eq!(lte_results[2].1, Value::Integer(300));

        // Test BETWEEN
        let between_results = db.range_between(20, 40)?;
        assert_eq!(between_results.len(), 3);
        assert_eq!(between_results[0].0, 20);
        assert_eq!(between_results[0].1, Value::Integer(200));
        assert_eq!(between_results[1].0, 30);
        assert_eq!(between_results[1].1, Value::Integer(300));
        assert_eq!(between_results[2].0, 40);
        assert_eq!(between_results[2].1, Value::Integer(400));

        // Test edge cases
        let gt_empty = db.range_gt(50)?;
        assert_eq!(gt_empty.len(), 0);

        let lt_empty = db.range_lt(10)?;
        assert_eq!(lt_empty.len(), 0);

        let between_empty = db.range_between(5, 8)?;
        assert_eq!(between_empty.len(), 0);

        fs::remove_file(test_db_path)?;
        let _ = fs::remove_file("test_db_range.wal");
        Ok(())
    }
}
