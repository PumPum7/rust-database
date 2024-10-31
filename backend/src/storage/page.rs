use super::error::{DatabaseError, Result};

pub const PAGE_SIZE: usize = 4096; // Standard page size (4KB)
const PAGE_HEADER_SIZE: usize = 8; // 4 bytes for page_id, 4 bytes for record count

#[derive(Debug)]
pub struct PageHeader {
    pub page_id: u32,
    pub record_count: u32,
}

impl PageHeader {
    pub fn new(page_id: u32) -> Self {
        Self {
            page_id,
            record_count: 0,
        }
    }

    pub fn deserialize(buffer: &[u8]) -> Result<Self> {
        if buffer.len() < PAGE_HEADER_SIZE {
            return Err(DatabaseError::InvalidPage);
        }
        Ok(Self {
            page_id: u32::from_le_bytes(buffer[0..4].try_into().unwrap()),
            record_count: u32::from_le_bytes(buffer[4..8].try_into().unwrap()),
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(PAGE_HEADER_SIZE);
        buffer.extend_from_slice(&self.page_id.to_le_bytes());
        buffer.extend_from_slice(&self.record_count.to_le_bytes());
        buffer
    }

    pub fn clone(&self) -> Self {
        Self {
            page_id: self.page_id,
            record_count: self.record_count,
        }
    }
}

#[derive(Debug)]
pub struct Page {
    pub header: PageHeader,
    pub data: Vec<u8>,
    is_dirty: bool,
    pin_count: u32,
}

impl Page {
    pub fn new(page_id: u32) -> Self {
        Self {
            header: PageHeader::new(page_id),
            data: vec![0; PAGE_SIZE - PAGE_HEADER_SIZE],
            is_dirty: false,
            pin_count: 0,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub fn pin(&mut self) {
        self.pin_count += 1;
    }

    pub fn unpin(&mut self) -> Result<()> {
        if self.pin_count == 0 {
            return Err(DatabaseError::InvalidOperation(
                "Cannot unpin an unpinned page".to_string(),
            ));
        }
        self.pin_count -= 1;
        Ok(())
    }

    pub fn clone(&self) -> Self {
        Self {
            header: self.header.clone(),
            data: self.data.clone(),
            is_dirty: self.is_dirty,
            pin_count: self.pin_count,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = self.header.serialize();
        buffer.extend_from_slice(&self.data);
        buffer
    }

    pub fn deserialize(buffer: &[u8]) -> Result<Self> {
        let header = PageHeader::deserialize(&buffer[0..PAGE_HEADER_SIZE])?;
        let data = buffer[PAGE_HEADER_SIZE..].to_vec();
        Ok(Self {
            header,
            data,
            is_dirty: false,
            pin_count: 0,
        })
    }

    pub fn write(&mut self) -> Result<()> {
        self.is_dirty = true;
        Ok(())
    }
}
