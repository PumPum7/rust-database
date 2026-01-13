use std::sync::Arc;
use std::sync::RwLock;

use crate::storage::buffer_pool::BufferPool;
use crate::storage::error::{DatabaseError, Result};
use crate::storage::mvcc::VersionedValue;
use crate::storage::page::Page;
use crate::storage::TransactionId;

const ORDER: usize = 4; // Maximum number of children per node
const MAX_KEYS: usize = ORDER - 1;
const MIN_KEYS: usize = (ORDER / 2) - 1;

#[derive(Debug)]
pub struct KeyValue {
    key: i32,
    versions: Vec<VersionedValue>,
}

impl Clone for KeyValue {
    fn clone(&self) -> Self {
        Self {
            key: self.key,
            versions: self.versions.clone(),
        }
    }
}

#[derive(Debug)]
pub struct BTreeNode {
    pub page_id: u32,
    pub is_leaf: bool,
    pub entries: Vec<KeyValue>, // Key-value pairs
    pub children: Vec<u32>,     // Page IDs of children
}

impl BTreeNode {
    pub fn new(page_id: u32, is_leaf: bool) -> Self {
        Self {
            page_id,
            is_leaf,
            entries: Vec::with_capacity(MAX_KEYS),
            children: Vec::with_capacity(ORDER),
        }
    }

    pub fn search(&self, key: i32) -> Result<Option<Vec<VersionedValue>>> {
        let mut idx = 0;

        while idx < self.entries.len() && key > self.entries[idx].key {
            idx += 1;
        }

        if idx < self.entries.len() && key == self.entries[idx].key {
            Ok(Some(self.entries[idx].versions.clone()))
        } else if self.is_leaf {
            Ok(None)
        } else if idx < self.children.len() {
            Ok(None) // Return None for internal nodes, we'll handle navigation in the BTree search
        } else {
            Ok(None)
        }
    }

    pub fn insert_non_full(&mut self, key: i32, versions: Vec<VersionedValue>) -> Result<()> {
        let mut idx = self.entries.len();

        if self.is_leaf {
            while idx > 0 && key < self.entries[idx - 1].key {
                idx -= 1;
            }

            self.entries.insert(idx, KeyValue { key, versions });
        } else {
            while idx > 0 && key < self.entries[idx - 1].key {
                idx -= 1;
            }
        }

        Ok(())
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= MAX_KEYS
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        // Header
        buffer.extend_from_slice(&self.page_id.to_le_bytes());
        buffer.push(if self.is_leaf { 1 } else { 0 });
        buffer.extend_from_slice(&(self.entries.len() as u16).to_le_bytes());

        // Entries
        for entry in &self.entries {
            buffer.extend_from_slice(&entry.key.to_le_bytes());

            // Serialize versions count
            buffer.extend_from_slice(&(entry.versions.len() as u16).to_le_bytes());

            // Serialize versions
            for version in &entry.versions {
                buffer.extend(version.serialize());
            }
        }

        // Children section: each child is 4 bytes
        for child in &self.children {
            buffer.extend_from_slice(&child.to_le_bytes());
        }

        buffer
    }

    pub fn deserialize(buffer: &[u8]) -> Result<Self> {
        if buffer.len() < 7 {
            return Err(DatabaseError::InvalidData(
                "Buffer too short for BTreeNode header".to_string(),
            )
            .into());
        }

        let page_id = u32::from_le_bytes(buffer[0..4].try_into().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid page_id")
        })?);
        let is_leaf = buffer[4] == 1;
        let entry_count = u16::from_le_bytes(buffer[5..7].try_into().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid entry_count")
        })?) as usize;

        let entries_start = 7;

        let mut current_pos = entries_start;
        let mut entries = Vec::new();
        for _ in 0..entry_count {
            if current_pos + 4 > buffer.len() {
                return Err(
                    DatabaseError::InvalidData("Buffer too short for key".to_string()).into(),
                );
            }
            let key =
                i32::from_le_bytes(buffer[current_pos..current_pos + 4].try_into().map_err(
                    |e| DatabaseError::InvalidData(format!("Invalid key bytes: {}", e)),
                )?);
            current_pos += 4;

            if current_pos + 2 > buffer.len() {
                return Err(DatabaseError::InvalidData(
                    "Buffer too short for version count".to_string(),
                )
                .into());
            }
            let version_count =
                u16::from_le_bytes(buffer[current_pos..current_pos + 2].try_into().map_err(
                    |e| DatabaseError::InvalidData(format!("Invalid version count bytes: {}", e)),
                )?) as usize;
            current_pos += 2;

            let mut versions = Vec::new();
            for _ in 0..version_count {
                let (version, version_size) =
                    VersionedValue::deserialize(&buffer[current_pos..])
                        .map_err(|e| DatabaseError::InvalidData(e.to_string()))?;
                versions.push(version);
                current_pos += version_size;
            }

            entries.push(KeyValue { key, versions });
        }

        let children: Vec<u32> = buffer[current_pos..]
            .chunks_exact(4)
            .map(|chunk| {
                chunk.try_into().map(u32::from_le_bytes).map_err(|e| {
                    DatabaseError::InvalidData(format!("Invalid child page ID: {}", e))
                })
            })
            .collect::<Result<_>>()?;

        Ok(Self {
            page_id,
            is_leaf,
            entries,
            children,
        })
    }
}

pub struct BTree {
    root_page_id: Arc<RwLock<u32>>,
}

impl BTree {
    pub fn new(root_page_id: u32) -> Self {
        Self {
            root_page_id: Arc::new(RwLock::new(root_page_id)),
        }
    }

    pub fn init(&self, buffer_pool: &mut BufferPool) -> Result<()> {
        let root_page_id = *self.root_page_id.read().unwrap();
        let root = BTreeNode::new(root_page_id, true);
        let mut page = Page::new(root_page_id);
        page.data = root.serialize();
        buffer_pool.write_page(root_page_id, page)?;
        Ok(())
    }

    pub fn root_page_id(&self) -> u32 {
        *self.root_page_id.read().unwrap()
    }

    pub fn search(
        &self,
        key: i32,
        buffer_pool: &mut BufferPool,
    ) -> Result<Option<Vec<VersionedValue>>> {
        let mut current_page_id = *self.root_page_id.read().unwrap();

        loop {
            let page = buffer_pool.get_page(current_page_id)?;
            let node = BTreeNode::deserialize(&page.data)?;

            // First check if key exists in current node
            let idx = node.entries.binary_search_by_key(&key, |entry| entry.key);

            match idx {
                Ok(i) => return Ok(Some(node.entries[i].versions.clone())),
                Err(i) if !node.is_leaf => {
                    // Key not found in current node, traverse to appropriate child
                    if i < node.children.len() {
                        current_page_id = node.children[i];
                    } else {
                        return Ok(None);
                    }
                }
                _ => return Ok(None),
            }
        }
    }

    pub fn insert(
        &mut self,
        key: i32,
        versions: Vec<VersionedValue>,
        buffer_pool: &mut BufferPool,
    ) -> Result<()> {
        let root_page_id = *self.root_page_id.read().unwrap();
        // Get root node
        let root_page = buffer_pool.get_page(root_page_id)?;
        let root_node = BTreeNode::deserialize(&root_page.data)?;

        if root_node.is_full() {
            // Create new root
            let new_root_page = buffer_pool.new_page()?;
            let mut new_root = BTreeNode::new(new_root_page.header.page_id, false);
            new_root.children.push(root_page_id);

            // Write new empty root
            let mut page = Page::new(new_root.page_id);
            page.data = new_root.serialize();
            buffer_pool.write_page(new_root.page_id, page)?;

            // Split old root
            self.split_child(new_root.page_id, 0, buffer_pool)?;

            // Update root page id
            let mut root_page_id_write = self.root_page_id.write().unwrap();
            *root_page_id_write = new_root.page_id;
            // Insert into new root

            drop(root_page_id_write);
            self.insert_non_full(new_root.page_id, key, versions, buffer_pool)?;
        } else {
            self.insert_non_full(root_page_id, key, versions, buffer_pool)?;
        }
        Ok(())
    }

    fn insert_non_full(
        &mut self,
        page_id: u32,
        key: i32,
        versions: Vec<VersionedValue>,
        buffer_pool: &mut BufferPool,
    ) -> Result<()> {
        let page = buffer_pool.get_page(page_id)?;
        let mut node = BTreeNode::deserialize(&page.data)?;

        if node.is_leaf {
            // Insert into leaf node
            let pos = node
                .entries
                .iter()
                .position(|entry| entry.key > key)
                .unwrap_or(node.entries.len());

            node.entries.insert(pos, KeyValue { key, versions });

            // Write updated node
            let mut page = Page::new(node.page_id);
            page.data = node.serialize();
            buffer_pool.write_page(node.page_id, page)?;
        } else {
            // Find child to insert into
            let child_idx = node.entries.partition_point(|entry| entry.key <= key);

            // Ensure child_idx is valid
            if child_idx >= node.children.len() {
                return Err(DatabaseError::InvalidData("Invalid child index".to_string()).into());
            }

            let child_page_id = node.children[child_idx];
            let child_page = buffer_pool.get_page(child_page_id)?;
            let child_node = BTreeNode::deserialize(&child_page.data)?;

            if child_node.is_full() {
                // Split child if full
                self.split_child(page_id, child_idx, buffer_pool)?;

                // Get updated node after split
                let page = buffer_pool.get_page(page_id)?;
                let node = BTreeNode::deserialize(&page.data)?;

                // Determine which child to follow
                let new_child_idx = if key > node.entries[child_idx].key {
                    child_idx + 1
                } else {
                    child_idx
                };

                // Recursively insert into child
                self.insert_non_full(node.children[new_child_idx], key, versions, buffer_pool)?;
            } else {
                // Recursively insert into child
                self.insert_non_full(child_page_id, key, versions, buffer_pool)?;
            }
        }
        Ok(())
    }

    fn split_child(
        &mut self,
        parent_page_id: u32,
        child_idx: usize,
        buffer_pool: &mut BufferPool,
    ) -> Result<()> {
        // Get parent and child nodes
        let parent_page = buffer_pool.get_page(parent_page_id)?;
        let mut parent = BTreeNode::deserialize(&parent_page.data)?;

        let child_page_id = parent.children[child_idx];
        let child_page = buffer_pool.get_page(child_page_id)?;
        let mut child = BTreeNode::deserialize(&child_page.data)?;

        // Create new sibling
        let new_sibling_page = buffer_pool.new_page()?;
        let mut new_sibling = BTreeNode::new(new_sibling_page.header.page_id, child.is_leaf);

        // Move half of child's entries to new sibling
        let mid_idx = (ORDER - 1) / 2;
        let mid_key = child.entries[mid_idx].clone();

        new_sibling.entries = child.entries.split_off(mid_idx + 1);
        child.entries.pop(); // Remove middle key

        if !child.is_leaf {
            new_sibling.children = child.children.split_off(mid_idx + 1);
        }

        // Insert new key into parent
        parent.entries.insert(child_idx, mid_key);
        parent.children.insert(child_idx + 1, new_sibling.page_id);

        // Write all changes back to disk
        let mut parent_page = Page::new(parent.page_id);
        parent_page.data = parent.serialize();
        buffer_pool.write_page(parent.page_id, parent_page)?;

        let mut child_page = Page::new(child.page_id);
        child_page.data = child.serialize();
        buffer_pool.write_page(child.page_id, child_page)?;

        let mut sibling_page = Page::new(new_sibling.page_id);
        sibling_page.data = new_sibling.serialize();
        buffer_pool.write_page(new_sibling.page_id, sibling_page)?;

        Ok(())
    }

    pub fn delete(&mut self, key: i32, buffer_pool: &mut BufferPool) -> Result<()> {
        let root_page_id = *self.root_page_id.read().unwrap();
        self.delete_key(root_page_id, key, buffer_pool)
            .map_err(|_| DatabaseError::KeyNotFound(key))
    }

    fn delete_key(&mut self, page_id: u32, key: i32, buffer_pool: &mut BufferPool) -> Result<()> {
        let page = buffer_pool.get_page(page_id)?;
        let mut node = BTreeNode::deserialize(&page.data)?;

        if node.is_leaf {
            // Case 1: If the key is in this leaf node, remove it
            return if let Some(idx) = node.entries.iter().position(|entry| entry.key == key) {
                node.entries.remove(idx);
                let mut page = Page::new(page_id);
                page.data = node.serialize();
                buffer_pool.write_page(page_id, page)?;
                Ok(())
            } else {
                Ok(())
            };
        } else {
            // Find the child which might contain the key
            let idx = match node.entries.binary_search_by_key(&key, |entry| entry.key) {
                Ok(i) => i + 1,
                Err(i) => i,
            };

            let child_page_id = node.children[idx];

            // Ensure the child has at least MIN_KEYS + 1 keys
            self.ensure_min_keys(page_id, idx, buffer_pool)?;

            // Recursively delete from the child
            self.delete_key(child_page_id, key, buffer_pool)?;
        }
        Ok(())
    }

    fn ensure_min_keys(
        &mut self,
        parent_page_id: u32,
        child_idx: usize,
        buffer_pool: &mut BufferPool,
    ) -> Result<()> {
        // Get parent node
        let parent_page = buffer_pool.get_page(parent_page_id)?;
        let parent = BTreeNode::deserialize(&parent_page.data)?;

        let child_page_id = parent.children[child_idx];
        let child_page = buffer_pool.get_page(child_page_id)?;
        let child = BTreeNode::deserialize(&child_page.data)?;

        if child.entries.len() >= MIN_KEYS {
            return Ok(());
        }

        // try to borrow from left sibling
        if child_idx > 0 {
            let left_sibling_page_id = parent.children[child_idx - 1];
            let left_sibling_page = buffer_pool.get_page(left_sibling_page_id)?;
            let mut left_sibling = BTreeNode::deserialize(&left_sibling_page.data)?;

            if left_sibling.entries.len() > MIN_KEYS {
                let mut child = child;
                let mut parent = parent;

                let separator = parent.entries.remove(child_idx - 1);
                child.entries.insert(0, separator);

                let new_separator = left_sibling.entries.pop().unwrap();
                parent.entries.insert(child_idx - 1, new_separator);

                if !child.is_leaf {
                    let moved_child = left_sibling.children.pop().unwrap();
                    child.children.insert(0, moved_child);
                }

                // Write changes back to pages
                let mut parent_page = Page::new(parent_page_id);
                let mut child_page = Page::new(child_page_id);
                let mut left_sibling_page = Page::new(left_sibling_page_id);

                parent_page.data = parent.serialize();
                child_page.data = child.serialize();
                left_sibling_page.data = left_sibling.serialize();

                buffer_pool.write_page(parent_page_id, parent_page)?;
                buffer_pool.write_page(child_page_id, child_page)?;
                buffer_pool.write_page(left_sibling_page_id, left_sibling_page)?;

                return Ok(());
            }
        }

        // try to borrow from right sibling
        if child_idx < parent.children.len() - 1 {
            let right_sibling_page_id = parent.children[child_idx + 1];
            let right_sibling_page = buffer_pool.get_page(right_sibling_page_id)?;
            let mut right_sibling = BTreeNode::deserialize(&right_sibling_page.data)?;

            if right_sibling.entries.len() > MIN_KEYS {
                let mut child = child;
                let mut parent = parent;

                let separator = parent.entries.remove(child_idx);
                child.entries.push(separator);

                let new_separator = right_sibling.entries.remove(0);
                parent.entries.insert(child_idx, new_separator);

                if !child.is_leaf {
                    let moved_child = right_sibling.children.remove(0);
                    child.children.push(moved_child);
                }

                // Write changes back to pages
                let mut parent_page = Page::new(parent_page_id);
                let mut child_page = Page::new(child_page_id);
                let mut right_sibling_page = Page::new(right_sibling_page_id);

                parent_page.data = parent.serialize();
                child_page.data = child.serialize();
                right_sibling_page.data = right_sibling.serialize();

                buffer_pool.write_page(parent_page_id, parent_page)?;
                buffer_pool.write_page(child_page_id, child_page)?;
                buffer_pool.write_page(right_sibling_page_id, right_sibling_page)?;

                return Ok(());
            }
        }

        // If we can't borrow, we need to merge
        self.merge_children(parent_page_id, child_idx, buffer_pool)
    }

    fn merge_children(
        &mut self,
        parent_page_id: u32,
        child_idx: usize,
        buffer_pool: &mut BufferPool,
    ) -> Result<()> {
        let parent_page = buffer_pool.get_page(parent_page_id)?;
        let mut parent = BTreeNode::deserialize(&parent_page.data)?;

        let left_child_page_id = parent.children[child_idx];
        let right_child_page_id = parent.children[child_idx + 1];

        let left_child_page = buffer_pool.get_page(left_child_page_id)?;
        let mut left_child = BTreeNode::deserialize(&left_child_page.data)?;

        let right_child_page = buffer_pool.get_page(right_child_page_id)?;
        let mut right_child = BTreeNode::deserialize(&right_child_page.data)?;

        // Move seperator to left child
        let seperator = parent.entries.remove(child_idx);
        left_child.entries.push(seperator);

        // Move all keys and children from right child to left child
        left_child.entries.extend(right_child.entries.drain(..));
        if !left_child.is_leaf {
            left_child.children.extend(right_child.children.drain(..));
        }

        // Remove right child from parent
        parent.children.remove(child_idx + 1);

        // Write changes back to pages
        let mut parent_page = Page::new(parent_page_id);
        let mut left_child_page = Page::new(left_child_page_id);
        let mut right_child_page = Page::new(right_child_page_id);

        parent_page.data = parent.serialize();
        left_child_page.data = left_child.serialize();
        right_child_page.data = right_child.serialize();

        buffer_pool.write_page(parent_page_id, parent_page)?;
        buffer_pool.write_page(left_child_page_id, left_child_page)?;

        buffer_pool.free_page(right_child_page_id)?;

        Ok(())
    }

    fn get_node(&self, page_id: u32, buffer_pool: &mut BufferPool) -> Result<BTreeNode> {
        let page = buffer_pool.get_page(page_id)?;
        BTreeNode::deserialize(&page.data)
    }

    pub fn update(
        &mut self,
        key: i32,
        versions: Vec<VersionedValue>,
        buffer_pool: &mut BufferPool,
    ) -> Result<()> {
        let mut current_page_id = *self.root_page_id.read().unwrap();

        loop {
            let page = buffer_pool.get_page(current_page_id)?;
            let mut node = BTreeNode::deserialize(&page.data)?;

            // Check if key exists in current node
            let idx = node.entries.binary_search_by_key(&key, |entry| entry.key);

            if let Ok(entry_idx) = idx {
                // Key found, update versions
                node.entries[entry_idx].versions = versions;

                // Write updated node
                let mut page = Page::new(node.page_id);
                page.data = node.serialize();
                buffer_pool.write_page(node.page_id, page)?;
                return Ok(());
            } else if !node.is_leaf {
                // Key not found, traverse to appropriate child
                let child_idx = idx.unwrap_or_else(|e| e);
                if child_idx < node.children.len() {
                    current_page_id = node.children[child_idx];
                    continue;
                } else {
                    // Key not found, insert it
                    return self.insert(key, versions, buffer_pool);
                }
            } else {
                // Leaf node and key not found, insert it
                return self.insert(key, versions, buffer_pool);
            }
        }
    }

    pub fn traverse(
        &self,
        page_id: u32,
        buffer_pool: &mut BufferPool,
        result: &mut Vec<(i32, Vec<VersionedValue>)>,
    ) -> Result<()> {
        let node = self.get_node(page_id, buffer_pool)?;

        if node.is_leaf {
            for entry in &node.entries {
                if !entry.versions.is_empty() {
                    result.push((entry.key, entry.versions.clone()));
                }
            }
        } else {
            for i in 0..=node.entries.len() {
                if i < node.children.len() {
                    self.traverse(node.children[i], buffer_pool, result)?;
                }
                if i < node.entries.len() && !node.entries[i].versions.is_empty() {
                    result.push((node.entries[i].key, node.entries[i].versions.clone()));
                }
            }
        }

        Ok(())
    }

    pub fn all(&self, buffer_pool: &mut BufferPool) -> Result<Vec<(i32, Vec<VersionedValue>)>> {
        let mut result = Vec::new();
        let root_page_id = *self.root_page_id.read().unwrap();
        self.traverse(root_page_id, buffer_pool, &mut result)?;
        Ok(result)
    }

    pub fn gc(
        &mut self,
        oldest_active_txn: TransactionId,
        buffer_pool: &mut BufferPool,
    ) -> Result<usize> {
        let root_page_id = *self.root_page_id.read().unwrap();
        self.gc_node(root_page_id, oldest_active_txn, buffer_pool)
    }

    fn gc_node(
        &mut self,
        page_id: u32,
        oldest_active_txn: TransactionId,
        buffer_pool: &mut BufferPool,
    ) -> Result<usize> {
        let page = buffer_pool.get_page(page_id)?;
        let mut node = BTreeNode::deserialize(&page.data)?;
        let mut versions_removed = 0;

        for entry in &mut node.entries {
            let original_len = entry.versions.len();

            // Keep only versions that are still visible to some transaction
            // A version can be garbage collected if:
            // 1. It was deleted (xmax != 0)
            // 2. The deleting transaction is older than the oldest active transaction
            // 3. There's a newer version available
            entry.versions.retain(|version| {
                if version.xmax == 0 {
                    // Not deleted, keep it
                    true
                } else if TransactionId(version.xmax) >= oldest_active_txn {
                    // Deleted by a transaction that might still be visible
                    true
                } else {
                    // Deleted and the delete is visible to all active transactions
                    false
                }
            });

            versions_removed += original_len - entry.versions.len();
        }

        // Write back the node if we removed any versions
        if versions_removed > 0 {
            let mut page = Page::new(node.page_id);
            page.data = node.serialize();
            buffer_pool.write_page(node.page_id, page)?;
        }

        // Recursively GC children
        if !node.is_leaf {
            for child_id in &node.children {
                versions_removed += self.gc_node(*child_id, oldest_active_txn, buffer_pool)?;
            }
        }

        Ok(versions_removed)
    }
}
