#[cfg(test)]
mod tests {
    use crate::storage::buffer_pool::BufferPool;
    use crate::storage::disk_manager::DiskManager;
    use crate::storage::error::Result;
    use crate::storage::page::Page;
    use crate::storage::wal::WriteAheadLog;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_buffer_pool_basic_operations() -> Result<()> {
        let disk_manager = DiskManager::new("test_buffer_pool.db")?;
        let wal = Arc::new(Mutex::new(WriteAheadLog::new("test_buffer_pool.wal")?));
        let mut buffer_pool = BufferPool::new(15, disk_manager, Arc::clone(&wal));

        // Test new page creation
        let page1 = buffer_pool.new_page()?;
        assert_eq!(page1.header.page_id, 0);

        // Test writing and reading a page
        let mut write_page = Page::new(0);
        write_page.data = vec![1, 2, 3, 4];
        buffer_pool.write_page(0, write_page)?;

        let read_page = buffer_pool.get_page(0)?;
        assert_eq!(read_page.data, vec![1, 2, 3, 4]);

        // Clean up
        std::fs::remove_file("test_buffer_pool.db")?;
        let _ = std::fs::remove_file("test_buffer_pool.wal");
        Ok(())
    }

    #[test]
    fn test_buffer_pool_full() -> Result<()> {
        let disk_manager = DiskManager::new("test_buffer_pool_full.db")?;
        let wal = Arc::new(Mutex::new(WriteAheadLog::new("test_buffer_pool_full.wal")?));
        let mut buffer_pool = BufferPool::new(2, disk_manager, Arc::clone(&wal));

        let _page1 = buffer_pool.new_page()?;
        let _page2 = buffer_pool.new_page()?;
        let page3 = buffer_pool.new_page();

        assert!(page3.is_err());

        std::fs::remove_file("test_buffer_pool_full.db")?;
        let _ = std::fs::remove_file("test_buffer_pool_full.wal");
        Ok(())
    }
}
