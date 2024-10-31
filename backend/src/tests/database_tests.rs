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
        Ok(())
    }
}
