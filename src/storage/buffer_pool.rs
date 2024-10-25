use super::disk_manager::DiskManager;
use super::error::{DatabaseError, Result};
use super::page::Page;
use std::collections::HashMap;

pub struct BufferPool {
    pages: HashMap<u32, Page>,
    capacity: usize,
    disk_manager: DiskManager,
}

impl BufferPool {
    pub fn new(capacity: usize, disk_manager: DiskManager) -> Self {
        Self {
            pages: HashMap::new(),
            capacity,
            disk_manager,
        }
    }

    pub fn get_page(&mut self, page_id: u32) -> Result<&mut Page> {
        if self.pages.contains_key(&page_id) {
            return Ok(self.pages.get_mut(&page_id).unwrap());
        }

        if self.pages.len() >= self.capacity {
            return Err(DatabaseError::BufferPoolFull);
        }

        let page = self.disk_manager.read_page(page_id)?;
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
        self.disk_manager.write_page(&page)?;
        Ok(())
    }

    pub fn free_page(&mut self, page_id: u32) -> Result<()> {
        self.pages.remove(&page_id);
        self.disk_manager.free_page(page_id)?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        for page in self.pages.values() {
            if page.is_dirty() {
                self.disk_manager.write_page(page)?;
            }
        }
        Ok(())
    }
}
