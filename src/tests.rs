#[cfg(test)]
mod tests {
    use crate::{Database, Path, Value};
    use std::fs;

    #[test]
    fn test_database_operations() -> Result<(), Box<dyn std::error::Error>> {
        let test_db_path = "test_db_ops.db";

        // Clean up any existing test database
        let _ = fs::remove_file(test_db_path);

        // Create new database
        let mut db = Database::new(test_db_path)?;

        // Test insert
        db.insert(1, &Value::Integer(100))?;

        // Test get
        assert_eq!(db.get(1)?, Some(Value::Integer(100)));

        // Test delete
        db.delete(1)?;
        assert_eq!(db.get(1)?, None);

        // Clean up
        fs::remove_file(test_db_path)?;

        Ok(())
    }

    #[test]
    fn test_database_initialization() -> Result<(), Box<dyn std::error::Error>> {
        let test_db_path = "test_db_init.db";
        let _ = fs::remove_file(test_db_path);

        let _ = Database::new(test_db_path)?;

        // Verify that the database was created
        assert!(Path::new(test_db_path).exists());

        // Clean up
        fs::remove_file(test_db_path)?;

        Ok(())
    }
}
