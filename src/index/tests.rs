#[cfg(test)]
mod tests {
    use crate::storage::error::Result;
    use crate::storage::DiskManager;
    use crate::{BTree, BufferPool, Value};

    #[test]
    fn test_btree_operations() -> Result<()> {
        let disk_manager = DiskManager::new("test_btree.db")?;
        let mut buffer_pool = BufferPool::new(100, disk_manager);
        let root_page_id = buffer_pool.new_page()?.header.page_id;
        let mut btree = BTree::new(root_page_id);

        // Initialize root node
        btree.init(&mut buffer_pool)?;

        // Insert test data
        btree.insert(5, Value::Integer(50), &mut buffer_pool)?;
        btree.insert(3, Value::Boolean(false), &mut buffer_pool)?;
        btree.insert(7, Value::String("Test".to_string()), &mut buffer_pool)?;

        // Verify insertions
        assert_eq!(btree.search(5, &mut buffer_pool)?, Some(Value::Integer(50)));
        assert_eq!(
            btree.search(3, &mut buffer_pool)?,
            Some(Value::Boolean(false))
        );
        assert_eq!(
            btree.search(7, &mut buffer_pool)?,
            Some(Value::String("Test".to_string()))
        );

        // Delete the file again
        std::fs::remove_file("test_btree.db")?;

        Ok(())
    }

    #[test]
    fn test_btree_delete() -> Result<()> {
        let disk_manager = DiskManager::new("test_btree_delete.db")?;
        let mut buffer_pool = BufferPool::new(100, disk_manager);
        let root_page_id = buffer_pool.new_page()?.header.page_id;
        let mut btree = BTree::new(root_page_id);

        btree.init(&mut buffer_pool)?;

        // Insert test data
        btree.insert(10, Value::Integer(100), &mut buffer_pool)?;
        btree.insert(20, Value::Integer(200), &mut buffer_pool)?;
        btree.insert(30, Value::Integer(300), &mut buffer_pool)?;

        // Delete a key
        btree.delete(20, &mut buffer_pool)?;

        // Verify deletion
        assert_eq!(btree.search(20, &mut buffer_pool)?, None);
        assert_eq!(
            btree.search(10, &mut buffer_pool)?,
            Some(Value::Integer(100))
        );
        assert_eq!(
            btree.search(30, &mut buffer_pool)?,
            Some(Value::Integer(300))
        );

        // Delete the file again
        std::fs::remove_file("test_btree_delete.db")?;

        Ok(())
    }

    #[test]
    fn test_btree_update() -> Result<()> {
        let disk_manager = DiskManager::new("test_btree_update.db")?;
        let mut buffer_pool = BufferPool::new(100, disk_manager);
        let root_page_id = buffer_pool.new_page()?.header.page_id;
        let mut btree = BTree::new(root_page_id);

        btree.init(&mut buffer_pool)?;

        // Insert initial data
        btree.insert(5, Value::String("five".to_string()), &mut buffer_pool)?;

        btree.delete(5, &mut buffer_pool)?;

        // Update the value
        btree.insert(5, Value::String("FIVE".to_string()), &mut buffer_pool)?;

        // Verify update
        assert_eq!(
            btree.search(5, &mut buffer_pool)?,
            Some(Value::String("FIVE".to_string()))
        );

        // Delete the file again
        std::fs::remove_file("test_btree_update.db")?;

        Ok(())
    }
}
