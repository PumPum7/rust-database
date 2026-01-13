use super::buffer_pool::BufferPool;
use super::error::{DatabaseError, Result};
use crc32fast::Hasher;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Mutex;

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
    committed_txns: Mutex<HashMap<u64, bool>>,
}

impl WriteAheadLog {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let log_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| DatabaseError::IoError(e))?;

        Ok(Self {
            log_file,
            sequence: 0,
            committed_txns: Mutex::new(HashMap::new()),
        })
    }

    pub fn get_sequence(&self) -> u64 {
        self.sequence
    }

    pub fn log(&mut self, record: LogRecord) -> Result<()> {
        self.log_file.seek(SeekFrom::End(0))?;
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
                self.committed_txns.lock().unwrap().insert(txn_id, true);
            }
            LogRecord::Rollback(txn_id) => {
                self.log_file.write_all(&[3])?;
                self.log_file.write_all(&txn_id.to_le_bytes())?;
                hasher.update(&[3]);
                hasher.update(&txn_id.to_le_bytes());
                self.committed_txns.lock().unwrap().insert(txn_id, false);
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
                hasher.update(&[4]);
                hasher.update(&txn_id.to_le_bytes());
                hasher.update(&page_id.to_le_bytes());
                hasher.update(&offset.to_le_bytes());
                hasher.update(&(data.len() as u32).to_le_bytes());
                hasher.update(&data);
            }
        }
        // Write checksum
        let checksum = hasher.finalize();
        self.log_file.write_all(&checksum.to_le_bytes())?;
        self.log_file.flush()?;
        Ok(())
    }

    pub fn recover(&mut self, buffer_pool: &mut BufferPool) -> Result<()> {
        self.log_file.seek(SeekFrom::Start(0))?;
        let mut pending_txns: HashMap<u64, Vec<LogRecord>> = HashMap::new();
        let mut current_sequence = 0u64;

        loop {
            let mut seq_bytes = [0u8; 8];
            if let Err(e) = self.log_file.read_exact(&mut seq_bytes) {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    break;
                }
                return Err(DatabaseError::IoError(e));
            }

            current_sequence = u64::from_le_bytes(seq_bytes);

            let mut type_byte = [0u8; 1];
            self.log_file.read_exact(&mut type_byte)?;

            let mut hasher = Hasher::new();
            hasher.update(&seq_bytes);
            hasher.update(&type_byte);

            let record = match type_byte[0] {
                1 => {
                    let mut txn_id_bytes = [0u8; 8];
                    self.log_file.read_exact(&mut txn_id_bytes)?;
                    let txn_id = u64::from_le_bytes(txn_id_bytes);
                    hasher.update(&txn_id_bytes);
                    LogRecord::Begin(txn_id)
                }
                2 => {
                    let mut txn_id_bytes = [0u8; 8];
                    self.log_file.read_exact(&mut txn_id_bytes)?;
                    let txn_id = u64::from_le_bytes(txn_id_bytes);
                    hasher.update(&txn_id_bytes);
                    LogRecord::Commit(txn_id)
                }
                3 => {
                    let mut txn_id_bytes = [0u8; 8];
                    self.log_file.read_exact(&mut txn_id_bytes)?;
                    let txn_id = u64::from_le_bytes(txn_id_bytes);
                    hasher.update(&txn_id_bytes);
                    LogRecord::Rollback(txn_id)
                }
                4 => {
                    let mut txn_id_bytes = [0u8; 8];
                    let mut page_id_bytes = [0u8; 4];
                    let mut offset_bytes = [0u8; 2];
                    let mut len_bytes = [0u8; 4];

                    self.log_file.read_exact(&mut txn_id_bytes)?;
                    self.log_file.read_exact(&mut page_id_bytes)?;
                    self.log_file.read_exact(&mut offset_bytes)?;
                    self.log_file.read_exact(&mut len_bytes)?;

                    let txn_id = u64::from_le_bytes(txn_id_bytes);
                    let page_id = u32::from_le_bytes(page_id_bytes);
                    let offset = u16::from_le_bytes(offset_bytes);
                    let len = u32::from_le_bytes(len_bytes);

                    hasher.update(&txn_id_bytes);
                    hasher.update(&page_id_bytes);
                    hasher.update(&offset_bytes);
                    hasher.update(&len_bytes);

                    let mut data = vec![0u8; len as usize];
                    self.log_file.read_exact(&mut data)?;
                    hasher.update(&data);

                    LogRecord::Write {
                        txn_id,
                        page_id,
                        offset,
                        data,
                    }
                }
                _ => {
                    return Err(DatabaseError::InvalidData(
                        "Unknown log record type".to_string(),
                    ));
                }
            };

            let mut checksum_bytes = [0u8; 4];
            self.log_file.read_exact(&mut checksum_bytes)?;
            let stored_checksum = u32::from_le_bytes(checksum_bytes);
            let computed_checksum = hasher.finalize();

            if stored_checksum != computed_checksum {
                return Err(DatabaseError::InvalidData(
                    "Checksum mismatch in WAL".to_string(),
                ));
            }

            match record {
                LogRecord::Begin(txn_id) => {
                    pending_txns.insert(txn_id, vec![record]);
                }
                LogRecord::Commit(txn_id) => {
                    if let Some(records) = pending_txns.get(&txn_id) {
                        for r in records {
                            if let LogRecord::Write {
                                page_id,
                                offset,
                                data,
                                ..
                            } = r
                            {
                                if let Ok(page) = buffer_pool.get_page(*page_id) {
                                    let start = *offset as usize;
                                    let end = start + data.len();
                                    if end <= page.data.len() {
                                        page.data[start..end].copy_from_slice(data);
                                        page.mark_dirty();
                                    }
                                }
                            }
                        }
                        pending_txns.remove(&txn_id);
                        self.committed_txns.lock().unwrap().insert(txn_id, true);
                    }
                }
                LogRecord::Rollback(txn_id) => {
                    pending_txns.remove(&txn_id);
                    self.committed_txns.lock().unwrap().insert(txn_id, false);
                }
                LogRecord::Write { txn_id, .. } => {
                    pending_txns
                        .entry(txn_id)
                        .or_insert_with(Vec::new)
                        .push(record);
                }
            }
        }

        self.sequence = current_sequence;

        let committed = self.committed_txns.lock().unwrap();
        for (txn_id, was_committed) in committed.iter() {
            if *was_committed && pending_txns.contains_key(txn_id) {
                pending_txns.remove(txn_id);
            }
        }

        Ok(())
    }

    pub fn truncate(&mut self) -> Result<()> {
        self.log_file.set_len(0)?;
        self.log_file.seek(SeekFrom::Start(0))?;
        self.sequence = 0;
        self.committed_txns.lock().unwrap().clear();
        Ok(())
    }
}
