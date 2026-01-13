use super::buffer_pool::BufferPool;
use super::error::{DatabaseError, Result};
use crc32fast::Hasher;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Mutex;

#[derive(Clone)]
pub enum LogRecord {
    Begin {
        lsn: u64,
        txn_id: u64,
    },
    Commit {
        lsn: u64,
        txn_id: u64,
    },
    Rollback {
        lsn: u64,
        txn_id: u64,
    },
    Write {
        lsn: u64,
        txn_id: u64,
        page_id: u32,
        offset: u16,
        before_image: Vec<u8>,
        after_image: Vec<u8>,
    },
    Clr {
        lsn: u64,
        txn_id: u64,
        page_id: u32,
        offset: u16,
        undo_image: Vec<u8>,
        undo_next_lsn: u64,
    },
    CheckpointBegin {
        lsn: u64,
    },
    CheckpointEnd {
        lsn: u64,
        active_txns: Vec<(u64, u64)>,
        dirty_pages: Vec<(u32, u64)>,
    },
}

#[derive(Clone, Debug)]
pub struct TransactionState {
    pub last_lsn: u64,
    pub status: TxnStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TxnStatus {
    Active,
    Committed,
    Aborted,
}

#[derive(Clone, Debug)]
pub struct DirtyPageEntry {
    pub rec_lsn: u64,
}

pub struct WriteAheadLog {
    log_file: File,
    sequence: u64,
    flushed_lsn: u64,
    committed_txns: Mutex<HashMap<u64, bool>>,
    active_txns: HashMap<u64, TransactionState>,
    dirty_pages: HashMap<u32, DirtyPageEntry>,
    last_checkpoint_lsn: u64,
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
            flushed_lsn: 0,
            committed_txns: Mutex::new(HashMap::new()),
            active_txns: HashMap::new(),
            dirty_pages: HashMap::new(),
            last_checkpoint_lsn: 0,
        })
    }

    pub fn get_sequence(&self) -> u64 {
        self.sequence
    }

    pub fn get_flushed_lsn(&self) -> u64 {
        self.flushed_lsn
    }

    pub fn flush(&mut self) -> Result<()> {
        self.log_file.sync_all()?;
        self.flushed_lsn = self.sequence;
        Ok(())
    }

    pub fn log(&mut self, record: LogRecord) -> Result<u64> {
        self.log_file.seek(SeekFrom::End(0))?;
        self.sequence += 1;
        let lsn = self.sequence;
        let mut hasher = Hasher::new();

        // Write sequence number (LSN)
        let seq_bytes = lsn.to_le_bytes();
        self.log_file.write_all(&seq_bytes)?;
        hasher.update(&seq_bytes);

        // Write record
        match record {
            LogRecord::Begin { txn_id, .. } => {
                self.log_file.write_all(&[1])?;
                self.log_file.write_all(&txn_id.to_le_bytes())?;
                hasher.update(&[1]);
                hasher.update(&txn_id.to_le_bytes());
                self.active_txns.insert(
                    txn_id,
                    TransactionState {
                        last_lsn: lsn,
                        status: TxnStatus::Active,
                    },
                );
            }
            LogRecord::Commit { txn_id, .. } => {
                self.log_file.write_all(&[2])?;
                self.log_file.write_all(&txn_id.to_le_bytes())?;
                hasher.update(&[2]);
                hasher.update(&txn_id.to_le_bytes());
                self.committed_txns.lock().unwrap().insert(txn_id, true);
                if let Some(state) = self.active_txns.get_mut(&txn_id) {
                    state.status = TxnStatus::Committed;
                    state.last_lsn = lsn;
                }
            }
            LogRecord::Rollback { txn_id, .. } => {
                self.log_file.write_all(&[3])?;
                self.log_file.write_all(&txn_id.to_le_bytes())?;
                hasher.update(&[3]);
                hasher.update(&txn_id.to_le_bytes());
                self.committed_txns.lock().unwrap().insert(txn_id, false);
                if let Some(state) = self.active_txns.get_mut(&txn_id) {
                    state.status = TxnStatus::Aborted;
                    state.last_lsn = lsn;
                }
            }
            LogRecord::Write {
                txn_id,
                page_id,
                offset,
                before_image,
                after_image,
                ..
            } => {
                self.log_file.write_all(&[4])?;
                self.log_file.write_all(&txn_id.to_le_bytes())?;
                self.log_file.write_all(&page_id.to_le_bytes())?;
                self.log_file.write_all(&offset.to_le_bytes())?;
                // Write before_image length and data
                self.log_file
                    .write_all(&(before_image.len() as u32).to_le_bytes())?;
                self.log_file.write_all(&before_image)?;
                // Write after_image length and data
                self.log_file
                    .write_all(&(after_image.len() as u32).to_le_bytes())?;
                self.log_file.write_all(&after_image)?;

                hasher.update(&[4]);
                hasher.update(&txn_id.to_le_bytes());
                hasher.update(&page_id.to_le_bytes());
                hasher.update(&offset.to_le_bytes());
                hasher.update(&(before_image.len() as u32).to_le_bytes());
                hasher.update(&before_image);
                hasher.update(&(after_image.len() as u32).to_le_bytes());
                hasher.update(&after_image);

                if let Some(state) = self.active_txns.get_mut(&txn_id) {
                    state.last_lsn = lsn;
                }
                self.dirty_pages
                    .entry(page_id)
                    .or_insert(DirtyPageEntry { rec_lsn: lsn });
            }
            LogRecord::Clr {
                txn_id,
                page_id,
                offset,
                undo_image,
                undo_next_lsn,
                ..
            } => {
                self.log_file.write_all(&[5])?;
                self.log_file.write_all(&txn_id.to_le_bytes())?;
                self.log_file.write_all(&page_id.to_le_bytes())?;
                self.log_file.write_all(&offset.to_le_bytes())?;
                self.log_file
                    .write_all(&(undo_image.len() as u32).to_le_bytes())?;
                self.log_file.write_all(&undo_image)?;
                self.log_file.write_all(&undo_next_lsn.to_le_bytes())?;

                hasher.update(&[5]);
                hasher.update(&txn_id.to_le_bytes());
                hasher.update(&page_id.to_le_bytes());
                hasher.update(&offset.to_le_bytes());
                hasher.update(&(undo_image.len() as u32).to_le_bytes());
                hasher.update(&undo_image);
                hasher.update(&undo_next_lsn.to_le_bytes());

                if let Some(state) = self.active_txns.get_mut(&txn_id) {
                    state.last_lsn = lsn;
                }
            }
            LogRecord::CheckpointBegin { .. } => {
                self.log_file.write_all(&[6])?;
                hasher.update(&[6]);
            }
            LogRecord::CheckpointEnd {
                active_txns,
                dirty_pages,
                ..
            } => {
                self.log_file.write_all(&[7])?;
                hasher.update(&[7]);

                // Write active transactions
                self.log_file
                    .write_all(&(active_txns.len() as u32).to_le_bytes())?;
                hasher.update(&(active_txns.len() as u32).to_le_bytes());
                for (txn_id, last_lsn) in &active_txns {
                    self.log_file.write_all(&txn_id.to_le_bytes())?;
                    self.log_file.write_all(&last_lsn.to_le_bytes())?;
                    hasher.update(&txn_id.to_le_bytes());
                    hasher.update(&last_lsn.to_le_bytes());
                }

                // Write dirty pages
                self.log_file
                    .write_all(&(dirty_pages.len() as u32).to_le_bytes())?;
                hasher.update(&(dirty_pages.len() as u32).to_le_bytes());
                for (page_id, rec_lsn) in &dirty_pages {
                    self.log_file.write_all(&page_id.to_le_bytes())?;
                    self.log_file.write_all(&rec_lsn.to_le_bytes())?;
                    hasher.update(&page_id.to_le_bytes());
                    hasher.update(&rec_lsn.to_le_bytes());
                }

                self.last_checkpoint_lsn = lsn;
            }
        }
        // Write checksum
        let checksum = hasher.finalize();
        self.log_file.write_all(&checksum.to_le_bytes())?;
        self.log_file.flush()?;
        Ok(lsn)
    }

    pub fn recover(&mut self, buffer_pool: &mut BufferPool) -> Result<()> {
        // ARIES 3-Phase Recovery:
        // 1. Analysis Phase: Scan log to reconstruct active_txns and dirty_pages tables
        // 2. Redo Phase: Redo all logged actions (repeating history)
        // 3. Undo Phase: Undo uncommitted transactions

        self.log_file.seek(SeekFrom::Start(0))?;

        // Phase 1: Analysis - read all records to build tables
        let records = self.read_all_records()?;
        if records.is_empty() {
            return Ok(());
        }

        let mut active_txns: HashMap<u64, TransactionState> = HashMap::new();
        let mut dirty_pages: HashMap<u32, DirtyPageEntry> = HashMap::new();
        let mut txn_writes: HashMap<u64, Vec<(u64, LogRecord)>> = HashMap::new();
        let mut txns_needing_undo: HashMap<u64, bool> = HashMap::new();
        let mut last_lsn = 0u64;

        for (lsn, record) in &records {
            last_lsn = *lsn;
            match record {
                LogRecord::Begin { txn_id, .. } => {
                    active_txns.insert(
                        *txn_id,
                        TransactionState {
                            last_lsn: *lsn,
                            status: TxnStatus::Active,
                        },
                    );
                }
                LogRecord::Commit { txn_id, .. } => {
                    if let Some(state) = active_txns.get_mut(txn_id) {
                        state.status = TxnStatus::Committed;
                        state.last_lsn = *lsn;
                    }
                    self.committed_txns.lock().unwrap().insert(*txn_id, true);
                    txns_needing_undo.remove(txn_id);
                }
                LogRecord::Rollback { txn_id, .. } => {
                    // Rollback was initiated - transaction needs undo unless CLRs complete it
                    if let Some(state) = active_txns.get_mut(txn_id) {
                        state.last_lsn = *lsn;
                    }
                    self.committed_txns.lock().unwrap().insert(*txn_id, false);
                    txns_needing_undo.insert(*txn_id, true);
                }
                LogRecord::Write {
                    txn_id, page_id, ..
                } => {
                    if let Some(state) = active_txns.get_mut(txn_id) {
                        state.last_lsn = *lsn;
                    }
                    dirty_pages
                        .entry(*page_id)
                        .or_insert(DirtyPageEntry { rec_lsn: *lsn });
                    txn_writes
                        .entry(*txn_id)
                        .or_insert_with(Vec::new)
                        .push((*lsn, record.clone()));
                }
                LogRecord::Clr {
                    txn_id,
                    page_id,
                    undo_next_lsn,
                    ..
                } => {
                    if let Some(state) = active_txns.get_mut(txn_id) {
                        state.last_lsn = *lsn;
                    }
                    dirty_pages
                        .entry(*page_id)
                        .or_insert(DirtyPageEntry { rec_lsn: *lsn });
                    // CLR indicates undo progress - remove writes that were undone
                    if *undo_next_lsn == 0 {
                        // Transaction fully undone
                        if let Some(state) = active_txns.get_mut(txn_id) {
                            state.status = TxnStatus::Aborted;
                        }
                        txns_needing_undo.remove(txn_id);
                        txn_writes.remove(txn_id);
                    }
                }
                LogRecord::CheckpointEnd {
                    active_txns: ckpt_txns,
                    dirty_pages: ckpt_pages,
                    ..
                } => {
                    // Initialize from checkpoint
                    for (txn_id, txn_last_lsn) in ckpt_txns {
                        active_txns.entry(*txn_id).or_insert(TransactionState {
                            last_lsn: *txn_last_lsn,
                            status: TxnStatus::Active,
                        });
                    }
                    for (page_id, rec_lsn) in ckpt_pages {
                        dirty_pages
                            .entry(*page_id)
                            .or_insert(DirtyPageEntry { rec_lsn: *rec_lsn });
                    }
                }
                LogRecord::CheckpointBegin { .. } => {}
            }
        }

        // Phase 2: Redo - replay all writes and CLRs from the earliest rec_lsn
        let redo_start_lsn = dirty_pages.values().map(|e| e.rec_lsn).min().unwrap_or(0);

        for (lsn, record) in &records {
            if *lsn < redo_start_lsn {
                continue;
            }

            match record {
                LogRecord::Write {
                    page_id,
                    offset,
                    after_image,
                    ..
                } => {
                    // Only redo if page LSN < this record's LSN
                    if let Ok(page) = buffer_pool.get_page(*page_id) {
                        if page.get_lsn() < *lsn {
                            let start = *offset as usize;
                            let end = start + after_image.len();
                            if end <= page.data.len() {
                                page.data[start..end].copy_from_slice(after_image);
                                page.set_lsn(*lsn);
                            }
                        }
                    }
                }
                LogRecord::Clr {
                    page_id,
                    offset,
                    undo_image,
                    ..
                } => {
                    // CLRs are also redone
                    if let Ok(page) = buffer_pool.get_page(*page_id) {
                        if page.get_lsn() < *lsn {
                            let start = *offset as usize;
                            let end = start + undo_image.len();
                            if end <= page.data.len() {
                                page.data[start..end].copy_from_slice(undo_image);
                                page.set_lsn(*lsn);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Phase 3: Undo - undo uncommitted transactions (those still Active or needing undo)
        let mut loser_txns: Vec<u64> = active_txns
            .iter()
            .filter(|(_, state)| state.status == TxnStatus::Active)
            .map(|(txn_id, _)| *txn_id)
            .collect();

        // Also include transactions that had Rollback but weren't fully undone
        for (txn_id, _) in txns_needing_undo.iter() {
            if !loser_txns.contains(txn_id) {
                loser_txns.push(*txn_id);
            }
        }

        for txn_id in loser_txns {
            if let Some(writes) = txn_writes.get(&txn_id) {
                // Undo in reverse order
                for (_write_lsn, record) in writes.iter().rev() {
                    if let LogRecord::Write {
                        page_id,
                        offset,
                        before_image,
                        ..
                    } = record
                    {
                        if !before_image.is_empty() {
                            if let Ok(page) = buffer_pool.get_page(*page_id) {
                                let start = *offset as usize;
                                let end = start + before_image.len();
                                if end <= page.data.len() {
                                    page.data[start..end].copy_from_slice(before_image);
                                    page.mark_dirty();
                                }
                            }
                        }
                    }
                }
            }
        }

        self.sequence = last_lsn;
        self.flushed_lsn = last_lsn;
        self.active_txns = active_txns
            .into_iter()
            .filter(|(_, state)| state.status == TxnStatus::Active)
            .collect();
        self.dirty_pages = dirty_pages;

        Ok(())
    }

    fn read_all_records(&mut self) -> Result<Vec<(u64, LogRecord)>> {
        self.log_file.seek(SeekFrom::Start(0))?;
        let mut records = Vec::new();

        loop {
            match self.read_single_record() {
                Ok(Some((lsn, record))) => records.push((lsn, record)),
                Ok(None) => break,
                Err(e) => return Err(e),
            }
        }

        Ok(records)
    }

    fn read_single_record(&mut self) -> Result<Option<(u64, LogRecord)>> {
        let mut seq_bytes = [0u8; 8];
        if let Err(e) = self.log_file.read_exact(&mut seq_bytes) {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return Ok(None);
            }
            return Err(DatabaseError::IoError(e));
        }

        let lsn = u64::from_le_bytes(seq_bytes);
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
                LogRecord::Begin { lsn, txn_id }
            }
            2 => {
                let mut txn_id_bytes = [0u8; 8];
                self.log_file.read_exact(&mut txn_id_bytes)?;
                let txn_id = u64::from_le_bytes(txn_id_bytes);
                hasher.update(&txn_id_bytes);
                LogRecord::Commit { lsn, txn_id }
            }
            3 => {
                let mut txn_id_bytes = [0u8; 8];
                self.log_file.read_exact(&mut txn_id_bytes)?;
                let txn_id = u64::from_le_bytes(txn_id_bytes);
                hasher.update(&txn_id_bytes);
                LogRecord::Rollback { lsn, txn_id }
            }
            4 => {
                let mut txn_id_bytes = [0u8; 8];
                let mut page_id_bytes = [0u8; 4];
                let mut offset_bytes = [0u8; 2];
                let mut before_len_bytes = [0u8; 4];

                self.log_file.read_exact(&mut txn_id_bytes)?;
                self.log_file.read_exact(&mut page_id_bytes)?;
                self.log_file.read_exact(&mut offset_bytes)?;
                self.log_file.read_exact(&mut before_len_bytes)?;

                let txn_id = u64::from_le_bytes(txn_id_bytes);
                let page_id = u32::from_le_bytes(page_id_bytes);
                let offset = u16::from_le_bytes(offset_bytes);
                let before_len = u32::from_le_bytes(before_len_bytes);

                hasher.update(&txn_id_bytes);
                hasher.update(&page_id_bytes);
                hasher.update(&offset_bytes);
                hasher.update(&before_len_bytes);

                let mut before_image = vec![0u8; before_len as usize];
                self.log_file.read_exact(&mut before_image)?;
                hasher.update(&before_image);

                let mut after_len_bytes = [0u8; 4];
                self.log_file.read_exact(&mut after_len_bytes)?;
                let after_len = u32::from_le_bytes(after_len_bytes);
                hasher.update(&after_len_bytes);

                let mut after_image = vec![0u8; after_len as usize];
                self.log_file.read_exact(&mut after_image)?;
                hasher.update(&after_image);

                LogRecord::Write {
                    lsn,
                    txn_id,
                    page_id,
                    offset,
                    before_image,
                    after_image,
                }
            }
            5 => {
                let mut txn_id_bytes = [0u8; 8];
                let mut page_id_bytes = [0u8; 4];
                let mut offset_bytes = [0u8; 2];
                let mut undo_len_bytes = [0u8; 4];

                self.log_file.read_exact(&mut txn_id_bytes)?;
                self.log_file.read_exact(&mut page_id_bytes)?;
                self.log_file.read_exact(&mut offset_bytes)?;
                self.log_file.read_exact(&mut undo_len_bytes)?;

                let txn_id = u64::from_le_bytes(txn_id_bytes);
                let page_id = u32::from_le_bytes(page_id_bytes);
                let offset = u16::from_le_bytes(offset_bytes);
                let undo_len = u32::from_le_bytes(undo_len_bytes);

                hasher.update(&txn_id_bytes);
                hasher.update(&page_id_bytes);
                hasher.update(&offset_bytes);
                hasher.update(&undo_len_bytes);

                let mut undo_image = vec![0u8; undo_len as usize];
                self.log_file.read_exact(&mut undo_image)?;
                hasher.update(&undo_image);

                let mut undo_next_lsn_bytes = [0u8; 8];
                self.log_file.read_exact(&mut undo_next_lsn_bytes)?;
                let undo_next_lsn = u64::from_le_bytes(undo_next_lsn_bytes);
                hasher.update(&undo_next_lsn_bytes);

                LogRecord::Clr {
                    lsn,
                    txn_id,
                    page_id,
                    offset,
                    undo_image,
                    undo_next_lsn,
                }
            }
            6 => LogRecord::CheckpointBegin { lsn },
            7 => {
                // Read active transactions
                let mut txn_count_bytes = [0u8; 4];
                self.log_file.read_exact(&mut txn_count_bytes)?;
                let txn_count = u32::from_le_bytes(txn_count_bytes);
                hasher.update(&txn_count_bytes);

                let mut active_txns = Vec::new();
                for _ in 0..txn_count {
                    let mut txn_id_bytes = [0u8; 8];
                    let mut last_lsn_bytes = [0u8; 8];
                    self.log_file.read_exact(&mut txn_id_bytes)?;
                    self.log_file.read_exact(&mut last_lsn_bytes)?;
                    hasher.update(&txn_id_bytes);
                    hasher.update(&last_lsn_bytes);
                    active_txns.push((
                        u64::from_le_bytes(txn_id_bytes),
                        u64::from_le_bytes(last_lsn_bytes),
                    ));
                }

                // Read dirty pages
                let mut page_count_bytes = [0u8; 4];
                self.log_file.read_exact(&mut page_count_bytes)?;
                let page_count = u32::from_le_bytes(page_count_bytes);
                hasher.update(&page_count_bytes);

                let mut dirty_pages = Vec::new();
                for _ in 0..page_count {
                    let mut page_id_bytes = [0u8; 4];
                    let mut rec_lsn_bytes = [0u8; 8];
                    self.log_file.read_exact(&mut page_id_bytes)?;
                    self.log_file.read_exact(&mut rec_lsn_bytes)?;
                    hasher.update(&page_id_bytes);
                    hasher.update(&rec_lsn_bytes);
                    dirty_pages.push((
                        u32::from_le_bytes(page_id_bytes),
                        u64::from_le_bytes(rec_lsn_bytes),
                    ));
                }

                LogRecord::CheckpointEnd {
                    lsn,
                    active_txns,
                    dirty_pages,
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

        Ok(Some((lsn, record)))
    }

    pub fn truncate(&mut self) -> Result<()> {
        self.log_file.set_len(0)?;
        self.log_file.seek(SeekFrom::Start(0))?;
        self.sequence = 0;
        self.flushed_lsn = 0;
        self.committed_txns.lock().unwrap().clear();
        self.active_txns.clear();
        self.dirty_pages.clear();
        self.last_checkpoint_lsn = 0;
        Ok(())
    }

    pub fn checkpoint(&mut self) -> Result<u64> {
        // Log checkpoint begin
        let begin_lsn = self.log(LogRecord::CheckpointBegin { lsn: 0 })?;

        // Collect active transactions
        let active_txns: Vec<(u64, u64)> = self
            .active_txns
            .iter()
            .filter(|(_, state)| state.status == TxnStatus::Active)
            .map(|(txn_id, state)| (*txn_id, state.last_lsn))
            .collect();

        // Collect dirty pages
        let dirty_pages: Vec<(u32, u64)> = self
            .dirty_pages
            .iter()
            .map(|(page_id, entry)| (*page_id, entry.rec_lsn))
            .collect();

        // Log checkpoint end with tables
        let end_lsn = self.log(LogRecord::CheckpointEnd {
            lsn: 0,
            active_txns,
            dirty_pages,
        })?;

        self.flush()?;
        self.last_checkpoint_lsn = end_lsn;

        Ok(begin_lsn)
    }

    pub fn get_last_checkpoint_lsn(&self) -> u64 {
        self.last_checkpoint_lsn
    }

    pub fn mark_page_clean(&mut self, page_id: u32) {
        self.dirty_pages.remove(&page_id);
    }

    pub fn get_active_txns(&self) -> &HashMap<u64, TransactionState> {
        &self.active_txns
    }

    pub fn get_dirty_pages(&self) -> &HashMap<u32, DirtyPageEntry> {
        &self.dirty_pages
    }
}
