#[cfg(test)]
mod tests {
    use crate::storage::error::Result;
    use crate::storage::{BufferPool, DiskManager, Page, SlottedPage};

    #[test]
    fn test_buffer_pool_basic_operations() -> Result<()> {
        let disk_manager = DiskManager::new("test_buffer_pool.db")?;
        let mut buffer_pool = BufferPool::new(15, disk_manager);

        // Test new page creation
        let page1 = buffer_pool.new_page()?;
        assert_eq!(page1.header.page_id, 0);

        // Test writing and reading a page
        let mut write_page = Page::new(0);
        write_page.data = vec![1, 2, 3, 4];
        buffer_pool.write_page(0, write_page)?;

        let read_page = buffer_pool.get_page(0)?;
        assert_eq!(read_page.data, vec![1, 2, 3, 4]);

        // Test page eviction
        for _ in 1..15 {
            let _ = buffer_pool.new_page()?;
        }
        // Page 0 should have been evicted, so reading it again should load from disk
        let reloaded_page = buffer_pool.get_page(0)?;
        assert_eq!(reloaded_page.data, vec![1, 2, 3, 4]);

        // Clean up
        std::fs::remove_file("test_buffer_pool.db")?;

        Ok(())
    }

    #[test]
    fn test_buffer_pool_full() -> Result<()> {
        let disk_manager = DiskManager::new("test_buffer_pool_full.db")?;
        let mut buffer_pool = BufferPool::new(2, disk_manager);

        // Test new page creation
        let _page1 = buffer_pool.new_page()?;
        let _page2 = buffer_pool.new_page()?;
        let page3 = buffer_pool.new_page();

        assert!(page3.is_err());

        // Clean up
        std::fs::remove_file("test_buffer_pool_full.db")?;

        Ok(())
    }

    #[test]
    fn test_slotted_page() -> Result<()> {
        let mut page = SlottedPage::new(Page::new(0));

        // Test inserting records
        let record1 = vec![1, 2, 3];
        let record2 = vec![4, 5, 6];
        let slot1 = page.insert_record(&record1)?;
        let slot2 = page.insert_record(&record2)?;
        // Test reading records
        assert_eq!(page.get_record(slot1)?, record1);
        assert_eq!(page.get_record(slot2)?, record2);

        Ok(())
    }

    #[test]
    fn test_record_operations() -> Result<()> {
        let page = Page::new(0);
        let mut slotted_page = SlottedPage::new(page);

        // Insert a record
        let record1 = b"Hello, World!";
        let slot1 = slotted_page.insert_record(record1)?;

        // Read it back
        let retrieved = slotted_page.get_record(slot1)?;
        assert_eq!(retrieved, record1);

        // Delete the record
        slotted_page.delete_record(slot1)?;

        // Verify it's deleted
        assert!(slotted_page.get_record(slot1).is_err());

        Ok(())
    }
}
