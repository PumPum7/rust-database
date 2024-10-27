use super::error::{DatabaseError, Result};
use super::page::Page;

// Record slot entry: 2 bytes offset + 2 bytes length
const SLOT_SIZE: usize = 4;

#[derive(Debug, Clone, Copy)]
struct Slot {
    offset: u16, // Offset from start of data section
    length: u16, // Length of the record
}

impl Slot {
    fn new(offset: u16, length: u16) -> Self {
        Self { offset, length }
    }

    fn serialize(&self) -> [u8; SLOT_SIZE] {
        let mut bytes = [0; SLOT_SIZE];
        bytes[0..2].copy_from_slice(&self.offset.to_le_bytes());
        bytes[2..4].copy_from_slice(&self.length.to_le_bytes());
        bytes
    }

    fn deserialize(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < SLOT_SIZE {
            return Err(DatabaseError::InvalidRecord);
        }

        Ok(Self {
            offset: u16::from_le_bytes(bytes[0..2].try_into().unwrap()),
            length: u16::from_le_bytes(bytes[2..4].try_into().unwrap()),
        })
    }
}

pub struct SlottedPage {
    page: Page,
    free_space_pointer: u16,
}

impl SlottedPage {
    pub fn new(page: Page) -> Self {
        let data_len = page.data.len();
        Self {
            page,
            free_space_pointer: (data_len - 1) as u16,
        }
    }

    pub fn insert_record(&mut self, record_data: &[u8]) -> Result<u16> {
        let record_len = record_data.len();
        if record_len > u16::MAX as usize {
            return Err(DatabaseError::InvalidRecord);
        }

        let slot_count = self.page.header.record_count as usize;
        let required_space = SLOT_SIZE + record_len;
        let available_space = self.free_space_pointer as usize - (slot_count * SLOT_SIZE);

        if required_space > available_space {
            return Err(DatabaseError::PageFull);
        }

        // Insert the record data
        let start_pos = self.free_space_pointer as usize - record_len;
        let end_pos = self.free_space_pointer as usize;
        self.page.data[start_pos..end_pos].copy_from_slice(record_data);

        // Create and insert the slot entry
        let slot = Slot::new(start_pos as u16, record_len as u16);
        let slot_bytes = slot.serialize();
        let slot_pos = slot_count * SLOT_SIZE;
        self.page.data[slot_pos..slot_pos + SLOT_SIZE].copy_from_slice(&slot_bytes);

        self.free_space_pointer -= record_len as u16;
        self.page.header.record_count += 1;
        self.page.mark_dirty();

        Ok(slot_count as u16)
    }

    pub fn get_record(&self, slot_number: u16) -> Result<Vec<u8>> {
        if slot_number >= self.page.header.record_count as u16 {
            return Err(DatabaseError::InvalidSlot);
        }

        let slot_pos = slot_number as usize * SLOT_SIZE;
        let slot = Slot::deserialize(&self.page.data[slot_pos..slot_pos + SLOT_SIZE])?;

        if slot.length == 0 {
            return Err(DatabaseError::DeletedRecord);
        }

        Ok(self.page.data[slot.offset as usize..(slot.offset + slot.length) as usize].to_vec())
    }

    pub fn delete_record(&mut self, slot_number: u16) -> Result<()> {
        if slot_number >= self.page.header.record_count as u16 {
            return Err(DatabaseError::InvalidSlot);
        }

        let slot_pos = slot_number as usize * SLOT_SIZE;
        let deleted_slot = Slot::new(0, 0); // Mark as deleted with zero length
        let slot_bytes = deleted_slot.serialize();
        self.page.data[slot_pos..slot_pos + SLOT_SIZE].copy_from_slice(&slot_bytes);
        self.page.mark_dirty();

        Ok(())
    }
}
