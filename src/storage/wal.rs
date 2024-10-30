use super::error::{DatabaseError, Result};
use crc32fast::Hasher;
use std::fs::{File, OpenOptions};
use std::io::Write;
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
    sequence: u64,
}

impl WriteAheadLog {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| DatabaseError::IoError(e))?;

        Ok(Self {
            log_file,
            sequence: 0,
        })
    }

    pub fn get_sequence(&self) -> u64 {
        self.sequence
    }

    pub fn log(&mut self, record: LogRecord) -> Result<()> {
        self.sequence += 1;
        let mut hasher = Hasher::new();

        // Write sequence number
        let seq_bytes = self.sequence.to_le_bytes();
        self.log_file.write_all(&seq_bytes)?;
        hasher.update(&seq_bytes);

        // Write record
        match record {
            LogRecord::Begin(txn_id) => {
                self.log_file.write_all(&[1])?;
                self.log_file.write_all(&txn_id.to_le_bytes())?;
                hasher.update(&[1]);
                hasher.update(&txn_id.to_le_bytes());
            }
            LogRecord::Commit(txn_id) => {
                self.log_file.write_all(&[2])?;
                self.log_file.write_all(&txn_id.to_le_bytes())?;
                hasher.update(&[2]);
                hasher.update(&txn_id.to_le_bytes());
            }
            LogRecord::Rollback(txn_id) => {
                self.log_file.write_all(&[3])?;
                self.log_file.write_all(&txn_id.to_le_bytes())?;
                hasher.update(&[3]);
                hasher.update(&txn_id.to_le_bytes());
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
                self.log_file
                    .write_all(&(data.len() as u32).to_le_bytes())?;
                self.log_file.write_all(&data)?;
            }
        }
        // Write checksum
        let checksum = hasher.finalize();
        self.log_file.write_all(&checksum.to_le_bytes())?;
        self.log_file.flush()?;
        Ok(())
    }
}
