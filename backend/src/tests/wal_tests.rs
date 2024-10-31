#[cfg(test)]
mod tests {
    use crate::storage::error::Result;
    use crate::storage::wal::{LogRecord, WriteAheadLog};

    #[test]
    fn test_wal_operations() -> Result<()> {
        let mut wal = WriteAheadLog::new("test_wal.db")?;

        wal.log(LogRecord::Begin(1))?;
        wal.log(LogRecord::Commit(1))?;
        wal.log(LogRecord::Rollback(1))?;

        assert_eq!(wal.get_sequence(), 3);

        std::fs::remove_file("test_wal.db")?;
        Ok(())
    }
}
