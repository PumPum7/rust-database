use super::error::{DatabaseError, Result};
use super::page::{Page, PAGE_SIZE};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

pub struct DiskManager {
    heap_file: File,
    next_page_id: u32,
}

impl DiskManager {
    pub fn new(file_path: &str) -> Result<Self> {
        let heap_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(file_path)?;

        Ok(Self {
            heap_file,
            next_page_id: 0,
        })
    }

    pub fn allocate_page(&mut self) -> Result<u32> {
        let page_id = self.next_page_id;
        self.next_page_id += 1;

        let zeros = vec![0u8; PAGE_SIZE];
        self.heap_file
            .seek(SeekFrom::Start(page_id as u64 * PAGE_SIZE as u64))?;
        self.heap_file.write_all(&zeros)?;

        Ok(page_id)
    }

    pub fn read_page(&mut self, page_id: u32) -> Result<Page> {
        let mut buffer = vec![0; PAGE_SIZE];
        self.heap_file
            .seek(SeekFrom::Start(page_id as u64 * PAGE_SIZE as u64))?;

        self.heap_file
            .read_exact(&mut buffer)
            .map_err(|_| DatabaseError::PageNotFound(page_id))?;
        Page::deserialize(&buffer)
    }

    pub fn write_page(&mut self, page: &Page) -> Result<()> {
        let buffer = page.serialize();
        self.heap_file.seek(SeekFrom::Start(
            page.header.page_id as u64 * PAGE_SIZE as u64,
        ))?;
        self.heap_file.write_all(&buffer)?;
        self.heap_file.flush()?;
        Ok(())
    }

    pub fn free_page(&mut self, page_id: u32) -> Result<()> {
        // Check if the page_id is valid
        if page_id >= self.next_page_id {
            return Err(DatabaseError::InvalidPage);
        }

        // Seek to the page location
        self.heap_file
            .seek(SeekFrom::Start(page_id as u64 * PAGE_SIZE as u64))?;

        // Overwrite the page with zeros
        let zeros = vec![0u8; PAGE_SIZE];
        self.heap_file.write_all(&zeros)?;

        // Flush changes to disk
        self.heap_file.flush()?;

        // Note: We don't decrement next_page_id to avoid reusing page IDs,
        Ok(())
    }
}
