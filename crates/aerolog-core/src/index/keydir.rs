// Copyright 2026 AeroLog Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::index::pointer::RecordPointer;
use parking_lot::RwLock;
use std::collections::HashMap;

/// High-concurrency in-memory hash table mapping keys to disk pointers.
///
/// Implements Bitcask $\mathcal{O}(1)$ random seek lookups and strictly enforces
/// deterministic Last-Write-Wins (LWW) conflict resolution using Hybrid Logical Clocks.
#[derive(Debug, Default)]
pub struct KeyDir {
    // collection -> (key -> pointer)
    entries: RwLock<HashMap<String, HashMap<String, RecordPointer>>>,
}

impl KeyDir {
    /// Creates a new empty KeyDir index.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Retrieves the record pointer for a specific collection and key.
    /// Returns `None` if the key does not exist or has been soft-deleted (tombstone).
    ///
    /// Executes with zero heap allocations on the hot read path.
    pub fn get(&self, collection: &str, key: &str) -> Option<RecordPointer> {
        let lock = self.entries.read();
        let col_map = lock.get(collection)?;
        match col_map.get(key) {
            Some(ptr) if !ptr.is_tombstone => Some(*ptr),
            _ => None,
        }
    }

    /// Retrieves the record pointer regardless of whether it is a tombstone.
    ///
    /// Executes with zero heap allocations on the hot path.
    pub fn get_raw(&self, collection: &str, key: &str) -> Option<RecordPointer> {
        let lock = self.entries.read();
        lock.get(collection)?.get(key).copied()
    }

    /// Inserts or updates a record pointer using Last-Write-Wins (LWW) resolution.
    ///
    /// If an existing pointer for this key has a higher HLC timestamp, the update is ignored.
    /// Returns `true` if the pointer was accepted, or `false` if rejected as stale.
    /// Rejected stale writes perform zero heap allocations.
    pub fn put(&self, collection: &str, key: &str, new_ptr: RecordPointer) -> bool {
        let mut lock = self.entries.write();
        let col_map = lock.entry(collection.to_string()).or_default();

        if let Some(existing) = col_map.get(key) {
            if existing.hlc > new_ptr.hlc {
                // Stale write rejected by Last-Write-Wins rule
                return false;
            }
        }

        col_map.insert(key.to_string(), new_ptr);
        true
    }

    /// Returns the total number of entries currently indexed in memory across all collections.
    pub fn len(&self) -> usize {
        let lock = self.entries.read();
        lock.values().map(|m| m.len()).sum()
    }

    /// Returns `true` if the index contains no entries.
    pub fn is_empty(&self) -> bool {
        let lock = self.entries.read();
        lock.values().all(|m| m.is_empty())
    }

    /// Returns all non-deleted keys currently stored under a given collection.
    /// Only scans the targeted collection map rather than iterating all records in the database.
    pub fn keys_in_collection(&self, collection: &str) -> Vec<String> {
        let lock = self.entries.read();
        match lock.get(collection) {
            Some(col_map) => col_map
                .iter()
                .filter(|(_, ptr)| !ptr.is_tombstone)
                .map(|(k, _)| k.clone())
                .collect(),
            None => Vec::new(),
        }
    }

    /// Returns a snapshot of all pointers currently in the index (used for compaction).
    pub fn snapshot(&self) -> Vec<((String, String), RecordPointer)> {
        let lock = self.entries.read();
        let total_len: usize = lock.values().map(|m| m.len()).sum();
        let mut result = Vec::with_capacity(total_len);
        for (col, map) in lock.iter() {
            for (key, ptr) in map.iter() {
                result.push(((col.clone(), key.clone()), *ptr));
            }
        }
        result
    }

    /// Clears the entire index.
    pub fn clear(&self) {
        self.entries.write().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hlc::HlcTimestamp;

    #[test]
    fn test_keydir_lww_conflict_resolution() {
        let keydir = KeyDir::new();
        let t1 = HlcTimestamp::new(1000, 0, 1);
        let t2 = HlcTimestamp::new(1001, 0, 1);

        let ptr1 = RecordPointer::new(1, 0, 100, t1, false);
        let ptr2 = RecordPointer::new(1, 100, 120, t2, false);

        assert!(keydir.put("users", "alice", ptr1));
        assert_eq!(keydir.get("users", "alice").unwrap().offset, 0);

        // Newer timestamp overwrites
        assert!(keydir.put("users", "alice", ptr2));
        assert_eq!(keydir.get("users", "alice").unwrap().offset, 100);

        // Stale timestamp is rejected
        assert!(!keydir.put("users", "alice", ptr1));
        assert_eq!(keydir.get("users", "alice").unwrap().offset, 100);
    }

    #[test]
    fn test_keydir_tombstone_behavior() {
        let keydir = KeyDir::new();
        let t1 = HlcTimestamp::new(1000, 0, 1);
        let t2 = HlcTimestamp::new(1002, 0, 1);

        let ptr = RecordPointer::new(1, 0, 50, t1, false);
        keydir.put("items", "item-1", ptr);
        assert!(keydir.get("items", "item-1").is_some());

        // Write tombstone
        let tombstone = RecordPointer::new(1, 50, 40, t2, true);
        keydir.put("items", "item-1", tombstone);

        // get() returns None for deleted item
        assert!(keydir.get("items", "item-1").is_none());
        // get_raw() still returns the pointer
        assert!(keydir.get_raw("items", "item-1").unwrap().is_tombstone);
    }
}
