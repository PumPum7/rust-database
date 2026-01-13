#[cfg(test)]
mod tests {
    use crate::storage::buffer_pool::BufferPool;
    use crate::storage::disk_manager::DiskManager;
    use crate::storage::error::Result;
    use crate::storage::wal::{LogRecord, WriteAheadLog};
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_wal_operations() -> Result<()> {
        let wal_path = "test_wal_ops.wal";
        let _ = std::fs::remove_file(wal_path);

        let mut wal = WriteAheadLog::new(wal_path)?;

        let lsn1 = wal.log(LogRecord::Begin { lsn: 0, txn_id: 1 })?;
        let lsn2 = wal.log(LogRecord::Commit { lsn: 0, txn_id: 1 })?;
        let lsn3 = wal.log(LogRecord::Rollback { lsn: 0, txn_id: 2 })?;

        assert_eq!(lsn1, 1);
        assert_eq!(lsn2, 2);
        assert_eq!(lsn3, 3);
        assert_eq!(wal.get_sequence(), 3);

        std::fs::remove_file(wal_path)?;
        Ok(())
    }

    #[test]
    fn test_wal_write_with_before_after_images() -> Result<()> {
        let wal_path = "test_wal_write.wal";
        let _ = std::fs::remove_file(wal_path);

        let mut wal = WriteAheadLog::new(wal_path)?;

        let before_image = vec![1, 2, 3, 4, 5];
        let after_image = vec![6, 7, 8, 9, 10];

        let lsn = wal.log(LogRecord::Write {
            lsn: 0,
            txn_id: 1,
            page_id: 0,
            offset: 0,
            before_image: before_image.clone(),
            after_image: after_image.clone(),
        })?;

        assert_eq!(lsn, 1);
        assert_eq!(wal.get_sequence(), 1);

        std::fs::remove_file(wal_path)?;
        Ok(())
    }

    #[test]
    fn test_wal_flush_and_flushed_lsn() -> Result<()> {
        let wal_path = "test_wal_flush.wal";
        let _ = std::fs::remove_file(wal_path);

        let mut wal = WriteAheadLog::new(wal_path)?;

        wal.log(LogRecord::Begin { lsn: 0, txn_id: 1 })?;
        wal.log(LogRecord::Begin { lsn: 0, txn_id: 2 })?;

        assert_eq!(wal.get_sequence(), 2);

        wal.flush()?;
        assert_eq!(wal.get_flushed_lsn(), 2);

        std::fs::remove_file(wal_path)?;
        Ok(())
    }

    #[test]
    fn test_wal_truncate() -> Result<()> {
        let wal_path = "test_wal_truncate.wal";
        let _ = std::fs::remove_file(wal_path);

        let mut wal = WriteAheadLog::new(wal_path)?;

        wal.log(LogRecord::Begin { lsn: 0, txn_id: 1 })?;
        wal.log(LogRecord::Commit { lsn: 0, txn_id: 1 })?;

        assert_eq!(wal.get_sequence(), 2);

        wal.truncate()?;

        assert_eq!(wal.get_sequence(), 0);
        assert_eq!(wal.get_flushed_lsn(), 0);

        std::fs::remove_file(wal_path)?;
        Ok(())
    }

    #[test]
    fn test_wal_recovery_committed_transaction() -> Result<()> {
        let wal_path = "test_wal_recovery_commit.wal";
        let db_path = "test_wal_recovery_commit.db";
        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(db_path);

        // Create WAL with a committed transaction
        {
            let mut wal = WriteAheadLog::new(wal_path)?;

            wal.log(LogRecord::Begin { lsn: 0, txn_id: 1 })?;
            wal.log(LogRecord::Write {
                lsn: 0,
                txn_id: 1,
                page_id: 0,
                offset: 0,
                before_image: vec![0; 10],
                after_image: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            })?;
            wal.log(LogRecord::Commit { lsn: 0, txn_id: 1 })?;
        }

        // Recover
        let mut wal = WriteAheadLog::new(wal_path)?;
        let disk_manager = DiskManager::new(db_path)?;
        let wal_arc = Arc::new(Mutex::new(WriteAheadLog::new(
            "test_wal_recovery_commit_temp.wal",
        )?));
        let mut buffer_pool = BufferPool::new(100, disk_manager, wal_arc);

        // Allocate page 0 so recovery can access it
        buffer_pool.new_page()?;

        wal.recover(&mut buffer_pool)?;

        // Verify page was updated
        let page = buffer_pool.get_page(0)?;
        assert_eq!(&page.data[0..10], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_file("test_wal_recovery_commit_temp.wal");
        Ok(())
    }

    #[test]
    fn test_wal_recovery_uncommitted_transaction() -> Result<()> {
        let wal_path = "test_wal_recovery_uncommit.wal";
        let db_path = "test_wal_recovery_uncommit.db";
        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(db_path);

        // Create WAL with an uncommitted transaction (no commit record)
        {
            let mut wal = WriteAheadLog::new(wal_path)?;

            wal.log(LogRecord::Begin { lsn: 0, txn_id: 1 })?;
            wal.log(LogRecord::Write {
                lsn: 0,
                txn_id: 1,
                page_id: 0,
                offset: 0,
                before_image: vec![0; 10],
                after_image: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            })?;
            // No commit - transaction is uncommitted
        }

        // Recover
        let mut wal = WriteAheadLog::new(wal_path)?;
        let disk_manager = DiskManager::new(db_path)?;
        let wal_arc = Arc::new(Mutex::new(WriteAheadLog::new(
            "test_wal_recovery_uncommit_temp.wal",
        )?));
        let mut buffer_pool = BufferPool::new(100, disk_manager, wal_arc);

        // Allocate page 0 so recovery can access it
        buffer_pool.new_page()?;

        wal.recover(&mut buffer_pool)?;

        // Verify page was restored to before_image (undo)
        let page = buffer_pool.get_page(0)?;
        assert_eq!(&page.data[0..10], &[0; 10]);

        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_file("test_wal_recovery_uncommit_temp.wal");
        Ok(())
    }

    #[test]
    fn test_wal_recovery_rolled_back_transaction() -> Result<()> {
        let wal_path = "test_wal_recovery_rollback.wal";
        let db_path = "test_wal_recovery_rollback.db";
        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(db_path);

        // Create WAL with a rolled back transaction
        {
            let mut wal = WriteAheadLog::new(wal_path)?;

            wal.log(LogRecord::Begin { lsn: 0, txn_id: 1 })?;
            wal.log(LogRecord::Write {
                lsn: 0,
                txn_id: 1,
                page_id: 0,
                offset: 0,
                before_image: vec![0; 10],
                after_image: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            })?;
            wal.log(LogRecord::Rollback { lsn: 0, txn_id: 1 })?;
        }

        // Recover
        let mut wal = WriteAheadLog::new(wal_path)?;
        let disk_manager = DiskManager::new(db_path)?;
        let wal_arc = Arc::new(Mutex::new(WriteAheadLog::new(
            "test_wal_recovery_rollback_temp.wal",
        )?));
        let mut buffer_pool = BufferPool::new(100, disk_manager, wal_arc);

        // Allocate page 0 so recovery can access it
        buffer_pool.new_page()?;

        wal.recover(&mut buffer_pool)?;

        // Verify page was restored to before_image
        let page = buffer_pool.get_page(0)?;
        assert_eq!(&page.data[0..10], &[0; 10]);

        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_file("test_wal_recovery_rollback_temp.wal");
        Ok(())
    }

    #[test]
    fn test_lsn_tracking() -> Result<()> {
        let wal_path = "test_lsn_tracking.wal";
        let db_path = "test_lsn_tracking.db";
        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(db_path);

        let wal = Arc::new(Mutex::new(WriteAheadLog::new(wal_path)?));
        let disk_manager = DiskManager::new(db_path)?;
        let mut buffer_pool = BufferPool::new(100, disk_manager, Arc::clone(&wal));

        // Create a page
        let page = buffer_pool.new_page()?;
        let page_id = page.header.page_id;

        // Initial LSN should be 0
        assert_eq!(page.get_lsn(), 0);

        // Log a write and update page LSN
        let lsn = wal.lock().unwrap().log(LogRecord::Write {
            lsn: 0,
            txn_id: 1,
            page_id,
            offset: 0,
            before_image: vec![],
            after_image: vec![1, 2, 3],
        })?;

        buffer_pool.get_page(page_id)?.set_lsn(lsn);
        assert_eq!(buffer_pool.get_page(page_id)?.get_lsn(), lsn);

        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn test_checkpoint() -> Result<()> {
        let wal_path = "test_checkpoint.wal";
        let _ = std::fs::remove_file(wal_path);

        let mut wal = WriteAheadLog::new(wal_path)?;

        // Start a transaction and write
        wal.log(LogRecord::Begin { lsn: 0, txn_id: 1 })?;
        wal.log(LogRecord::Write {
            lsn: 0,
            txn_id: 1,
            page_id: 0,
            offset: 0,
            before_image: vec![0; 10],
            after_image: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        })?;

        // Take a checkpoint
        let checkpoint_lsn = wal.checkpoint()?;
        assert!(checkpoint_lsn > 0);
        assert_eq!(wal.get_last_checkpoint_lsn(), wal.get_sequence());

        // Verify active transactions and dirty pages are tracked
        let active_txns = wal.get_active_txns();
        assert!(active_txns.contains_key(&1));

        let dirty_pages = wal.get_dirty_pages();
        assert!(dirty_pages.contains_key(&0));

        // Commit the transaction
        wal.log(LogRecord::Commit { lsn: 0, txn_id: 1 })?;

        let _ = std::fs::remove_file(wal_path);
        Ok(())
    }

    #[test]
    fn test_clr_logging() -> Result<()> {
        let wal_path = "test_clr.wal";
        let _ = std::fs::remove_file(wal_path);

        let mut wal = WriteAheadLog::new(wal_path)?;

        // Start a transaction, write, then log CLR for undo
        wal.log(LogRecord::Begin { lsn: 0, txn_id: 1 })?;
        let write_lsn = wal.log(LogRecord::Write {
            lsn: 0,
            txn_id: 1,
            page_id: 0,
            offset: 0,
            before_image: vec![0; 10],
            after_image: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        })?;

        // Log a CLR (compensation log record) for undoing the write
        let clr_lsn = wal.log(LogRecord::Clr {
            lsn: 0,
            txn_id: 1,
            page_id: 0,
            offset: 0,
            undo_image: vec![0; 10],
            undo_next_lsn: 0, // No more writes to undo
        })?;

        assert!(clr_lsn > write_lsn);
        assert_eq!(wal.get_sequence(), 3);

        let _ = std::fs::remove_file(wal_path);
        Ok(())
    }

    #[test]
    fn test_recovery_with_clr() -> Result<()> {
        let wal_path = "test_recovery_clr.wal";
        let db_path = "test_recovery_clr.db";
        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(db_path);

        // Create WAL with transaction that has CLR (already undone)
        {
            let mut wal = WriteAheadLog::new(wal_path)?;

            wal.log(LogRecord::Begin { lsn: 0, txn_id: 1 })?;
            wal.log(LogRecord::Write {
                lsn: 0,
                txn_id: 1,
                page_id: 0,
                offset: 0,
                before_image: vec![0; 10],
                after_image: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            })?;
            wal.log(LogRecord::Rollback { lsn: 0, txn_id: 1 })?;
            // CLR showing the write was undone
            wal.log(LogRecord::Clr {
                lsn: 0,
                txn_id: 1,
                page_id: 0,
                offset: 0,
                undo_image: vec![0; 10],
                undo_next_lsn: 0, // Fully undone
            })?;
        }

        // Recover
        let mut wal = WriteAheadLog::new(wal_path)?;
        let disk_manager = DiskManager::new(db_path)?;
        let wal_arc = Arc::new(Mutex::new(WriteAheadLog::new(
            "test_recovery_clr_temp.wal",
        )?));
        let mut buffer_pool = BufferPool::new(100, disk_manager, wal_arc);

        buffer_pool.new_page()?;
        wal.recover(&mut buffer_pool)?;

        // Redo phase should have applied the CLR (restoring to zeros)
        let page = buffer_pool.get_page(0)?;
        assert_eq!(&page.data[0..10], &[0; 10]);

        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_file("test_recovery_clr_temp.wal");
        Ok(())
    }

    #[test]
    fn test_recovery_with_checkpoint() -> Result<()> {
        let wal_path = "test_recovery_checkpoint.wal";
        let db_path = "test_recovery_checkpoint.db";
        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(db_path);

        // Create WAL with checkpoint
        {
            let mut wal = WriteAheadLog::new(wal_path)?;

            // First committed transaction
            wal.log(LogRecord::Begin { lsn: 0, txn_id: 1 })?;
            wal.log(LogRecord::Write {
                lsn: 0,
                txn_id: 1,
                page_id: 0,
                offset: 0,
                before_image: vec![0; 5],
                after_image: vec![1, 2, 3, 4, 5],
            })?;
            wal.log(LogRecord::Commit { lsn: 0, txn_id: 1 })?;

            // Take checkpoint with second transaction in progress
            wal.log(LogRecord::Begin { lsn: 0, txn_id: 2 })?;
            wal.log(LogRecord::Write {
                lsn: 0,
                txn_id: 2,
                page_id: 0,
                offset: 5,
                before_image: vec![0; 5],
                after_image: vec![6, 7, 8, 9, 10],
            })?;

            // Checkpoint records txn 2 as active
            wal.checkpoint()?;

            // Txn 2 commits after checkpoint
            wal.log(LogRecord::Commit { lsn: 0, txn_id: 2 })?;
        }

        // Recover
        let mut wal = WriteAheadLog::new(wal_path)?;
        let disk_manager = DiskManager::new(db_path)?;
        let wal_arc = Arc::new(Mutex::new(WriteAheadLog::new(
            "test_recovery_checkpoint_temp.wal",
        )?));
        let mut buffer_pool = BufferPool::new(100, disk_manager, wal_arc);

        buffer_pool.new_page()?;
        wal.recover(&mut buffer_pool)?;

        // Both transactions should be redone
        let page = buffer_pool.get_page(0)?;
        assert_eq!(&page.data[0..10], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_file("test_recovery_checkpoint_temp.wal");
        Ok(())
    }
}
