use super::disk_manager::DiskManager;
use super::error::{DatabaseError, Result};
use super::page::Page;
use super::wal::WriteAheadLog;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct BufferPool {
    pages: HashMap<u32, Page>,
    capacity: usize,
    disk_manager: DiskManager,
    wal: Arc<Mutex<WriteAheadLog>>,
    flushed_lsn: u64,
}

impl BufferPool {
    pub fn new(capacity: usize, disk_manager: DiskManager, wal: Arc<Mutex<WriteAheadLog>>) -> Self {
        Self {
            pages: HashMap::new(),
            capacity,
            disk_manager,
            wal,
            flushed_lsn: 0,
        }
    }

    pub fn get_page(&mut self, page_id: u32) -> Result<&mut Page> {
        if self.pages.contains_key(&page_id) {
            return Ok(self.pages.get_mut(&page_id).unwrap());
        }

        if self.pages.len() >= self.capacity {
            return Err(DatabaseError::BufferPoolFull);
        }

        let page = self.disk_manager.read_page(page_id).map_err(|e| {
            DatabaseError::InvalidOperation(format!("Failed to read page {}: {}", page_id, e))
        })?;
        self.pages.insert(page_id, page);
        Ok(self.pages.get_mut(&page_id).unwrap())
    }

    pub fn new_page(&mut self) -> Result<&mut Page> {
        if self.pages.len() >= self.capacity {
            return Err(DatabaseError::BufferPoolFull);
        }

        let page_id = self.disk_manager.allocate_page()?;
        let page = Page::new(page_id);
        self.pages.insert(page_id, page);
        Ok(self.pages.get_mut(&page_id).unwrap())
    }

    pub fn write_page(&mut self, page_id: u32, page: Page) -> Result<()> {
        self.pages.insert(page_id, page.clone());

        // Enforce WAL protocol: flush WAL before writing page if needed
        let page_lsn = page.get_lsn();
        if page_lsn > self.flushed_lsn {
            self.ensure_wal_flushed(page_lsn)?;
        }

        self.disk_manager.write_page(&page).map_err(|e| {
            DatabaseError::InvalidOperation(format!("Failed to write page {}: {}", page_id, e))
        })?;
        Ok(())
    }

    pub fn flush_page(&mut self, page_id: u32) -> Result<()> {
        // Extract page info without holding borrow
        let page_info = self.pages.get(&page_id).and_then(|page| {
            if page.is_dirty() {
                Some(page.get_lsn())
            } else {
                None
            }
        });

        if let Some(page_lsn) = page_info {
            // Enforce WAL protocol: WAL must be flushed up to page's LSN
            if page_lsn > self.flushed_lsn {
                self.ensure_wal_flushed(page_lsn)?;
            }

            // Now we can borrow the page again for writing
            if let Some(page) = self.pages.get(&page_id) {
                self.disk_manager.write_page(page).map_err(|e| {
                    DatabaseError::InvalidOperation(format!(
                        "Failed to flush page {}: {}",
                        page_id, e
                    ))
                })?;
            }
        }
        Ok(())
    }

    fn ensure_wal_flushed(&mut self, required_lsn: u64) -> Result<()> {
        let mut wal = self.wal.lock().unwrap();
        wal.flush()?;
        self.flushed_lsn = wal.get_flushed_lsn();

        if required_lsn > self.flushed_lsn {
            return Err(DatabaseError::PageLsnAhead {
                page_lsn: required_lsn,
                flushed_lsn: self.flushed_lsn,
            });
        }
        Ok(())
    }

    pub fn free_page(&mut self, page_id: u32) -> Result<()> {
        self.pages.remove(&page_id);
        self.disk_manager.free_page(page_id)?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        // First ensure WAL is flushed
        {
            let mut wal = self.wal.lock().unwrap();
            wal.flush()?;
            self.flushed_lsn = wal.get_flushed_lsn();
        }

        // Then write all dirty pages
        for page in self.pages.values() {
            if page.is_dirty() {
                self.disk_manager.write_page(page).map_err(|e| {
                    DatabaseError::InvalidOperation(format!(
                        "Failed to write page {}: {}",
                        page.header.page_id, e
                    ))
                })?;
            }
        }
        Ok(())
    }

    pub fn get_dirty_pages(&self) -> Vec<(u32, u64)> {
        self.pages
            .iter()
            .filter(|(_, page)| page.is_dirty())
            .map(|(page_id, page)| (*page_id, page.get_lsn()))
            .collect()
    }

    pub fn get_flushed_lsn(&self) -> u64 {
        self.flushed_lsn
    }

    pub fn update_flushed_lsn(&mut self, lsn: u64) {
        if lsn > self.flushed_lsn {
            self.flushed_lsn = lsn;
        }
    }
}
