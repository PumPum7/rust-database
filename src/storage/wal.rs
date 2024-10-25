use super::error::{DatabaseError, Result};
use std::fs::{File, OpenOptions};
use std::io::{Write};
use std::path::Path;

pub enum LogRecord {
    Begin(u64),    // Transaction ID
    Commit(u64),   // Transaction ID
    Rollback(u64), // Transaction ID
    Write {
        txn_id: u64,
        page_id: u32,
        offset: u16,
        data: Vec<u8>,
    },
}

pub struct WriteAheadLog {
    log_file: File,
}

impl WriteAheadLog {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| DatabaseError::IoError(e))?;

        Ok(Self { log_file })
    }

    pub fn log(&mut self, record: LogRecord) -> Result<()> {
        // Simple implementation - just serialize and write
        // TODO: Add checksums and sequence numbers
        match record {
            LogRecord::Begin(txn_id) => {
                self.log_file.write_all(&[1])?; // Type
                self.log_file.write_all(&txn_id.to_le_bytes())?;
            }
            LogRecord::Commit(txn_id) => {
                self.log_file.write_all(&[2])?;
                self.log_file.write_all(&txn_id.to_le_bytes())?;
            }
            LogRecord::Rollback(txn_id) => {
                self.log_file.write_all(&[3])?;
                self.log_file.write_all(&txn_id.to_le_bytes())?;
            }
            LogRecord::Write {
                txn_id,
                page_id,
                offset,
                data,
            } => {
                self.log_file.write_all(&[4])?;
                self.log_file.write_all(&txn_id.to_le_bytes())?;
                self.log_file.write_all(&page_id.to_le_bytes())?;
                self.log_file.write_all(&offset.to_le_bytes())?;
                self.log_file.write_all(&(data.len() as u32).to_le_bytes())?;
                self.log_file.write_all(&data)?;
            }
        }
        self.log_file.flush()?;
        Ok(())
    }
}
