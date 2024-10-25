pub mod index;
pub mod storage;

use index::BTree;
use std::path::Path;
pub use storage::{BufferPool, DiskManager, Transaction, TransactionManager, Value};

pub struct Database {
    buffer_pool: BufferPool,
    transaction_manager: TransactionManager,
    index: BTree,
}

impl Database {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let disk_manager = DiskManager::new(path.as_ref().to_str().unwrap())?;
        let mut buffer_pool = BufferPool::new(1000, disk_manager);

        // Create and initialize root page for index
        let root_page_id = buffer_pool.new_page()?.header.page_id;
        let btree = BTree::new(root_page_id);

        // Initialize the B-tree with an empty root node
        btree.init(&mut buffer_pool)?;

        Ok(Self {
            buffer_pool,
            transaction_manager: TransactionManager::new(),
            index: btree,
        })
    }

    pub fn begin_transaction(&mut self) -> Result<Transaction, Box<dyn std::error::Error>> {
        Ok(self.transaction_manager.begin_transaction()?)
    }

    pub fn insert(&mut self, key: i32, value: Value) -> Result<(), Box<dyn std::error::Error>> {
        // Add error handling and logging for debugging
        match self.index.insert(key, value, &mut self.buffer_pool) {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("Error inserting key {}: {}", key, e);
                Err(Box::new(e))
            }
        }
    }

    pub fn get(&mut self, key: i32) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        match self.index.search(key, &mut self.buffer_pool) {
            Ok(result) => Ok(result),
            Err(e) => {
                eprintln!("Error searching for key {}: {}", key, e);
                Err(Box::new(e))
            }
        }
    }

    pub fn delete(&mut self, key: i32) -> Result<(), Box<dyn std::error::Error>> {
        match self.index.delete(key, &mut self.buffer_pool) {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("Error deleting key {}: {}", key, e);
                Err(Box::new(e))
            }
        }
    }

    pub fn update(&mut self, key: i32, value: Value) -> Result<(), Box<dyn std::error::Error>> {
        self.delete(key)?;
        self.insert(key, value)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_database_operations() -> Result<(), Box<dyn std::error::Error>> {
        let test_db_path = "test_db_ops.db";

        // Clean up any existing test database
        let _ = fs::remove_file(test_db_path);

        // Create new database
        let mut db = Database::new(test_db_path)?;

        // Test insert
        db.insert(1, Value::Integer(100))?;

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

        let db = Database::new(test_db_path)?;

        // Verify that the database was created
        assert!(Path::new(test_db_path).exists());

        // Clean up
        fs::remove_file(test_db_path)?;

        Ok(())
    }
}
