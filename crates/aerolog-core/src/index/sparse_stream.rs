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

use crate::hlc::HlcTimestamp;
use crate::index::pointer::{RecordPointer, StreamCheckpoint};
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap};

/// In-memory metadata and sparse checkpoints for an individual append-only stream.
#[derive(Debug, Clone, Default)]
pub struct StreamMetadata {
    /// Earliest surviving sequence number for this stream.
    pub base_sequence: u64,
    /// Total number of messages/events appended to this stream.
    pub total_entries: u64,
    /// Latest observed HLC timestamp in this stream.
    pub latest_hlc: Option<HlcTimestamp>,
    /// Sparse checkpoint pointers indexed by sequence number.
    pub checkpoints: BTreeMap<u64, StreamCheckpoint>,
    /// Pointer to the most recent record appended to the stream.
    pub latest_pointer: Option<RecordPointer>,
}

/// Sparse in-memory index for append-only streams (chat rooms, event logs).
///
/// Keeps RAM footprint tiny (<15 MB) by recording only 1 checkpoint every $N$ entries,
/// preventing the in-memory index from exploding during massive message volume.
#[derive(Debug)]
pub struct SparseStreamIndex {
    checkpoint_interval: u32,
    streams: RwLock<HashMap<String, StreamMetadata>>,
}

impl SparseStreamIndex {
    /// Creates a new sparse stream index with the specified checkpoint interval.
    pub fn new(checkpoint_interval: u32) -> Self {
        Self {
            checkpoint_interval: checkpoint_interval.max(1),
            streams: RwLock::new(HashMap::new()),
        }
    }

    /// Records an appended stream entry into the sparse index.
    ///
    /// Avoids heap allocations when appending to an already active stream.
    pub fn record_entry(&self, stream_id: &str, hlc: HlcTimestamp, pointer: RecordPointer) -> u64 {
        let mut lock = self.streams.write();
        let meta = if let Some(meta) = lock.get_mut(stream_id) {
            meta
        } else {
            lock.entry(stream_id.to_string()).or_default()
        };

        let seq = meta.total_entries;
        meta.total_entries += 1;
        meta.latest_hlc = Some(hlc);
        meta.latest_pointer = Some(pointer);

        if seq.is_multiple_of(self.checkpoint_interval as u64) {
            let cp = StreamCheckpoint {
                sequence: seq,
                hlc,
                file_id: pointer.file_id,
                offset: pointer.offset,
            };
            meta.checkpoints.insert(seq, cp);
        }

        seq
    }

    /// Records a stream entry at an explicit sequence number (used during post-compaction rebuild).
    pub fn record_entry_with_seq(
        &self,
        stream_id: &str,
        seq: u64,
        hlc: HlcTimestamp,
        pointer: RecordPointer,
        force_checkpoint: bool,
    ) {
        let mut lock = self.streams.write();
        let interval = self.checkpoint_interval as u64;
        let meta = lock.entry(stream_id.to_string()).or_default();

        if seq >= meta.total_entries {
            meta.total_entries = seq + 1;
        }
        meta.latest_hlc = Some(hlc);
        meta.latest_pointer = Some(pointer);

        if force_checkpoint || seq.is_multiple_of(interval) {
            let cp = StreamCheckpoint {
                sequence: seq,
                hlc,
                file_id: pointer.file_id,
                offset: pointer.offset,
            };
            meta.checkpoints.insert(seq, cp);
        }
    }

    /// Resets checkpoint tree for a stream while preserving base sequence and total entry bounds.
    pub fn reset_stream_checkpoints(
        &self,
        stream_id: &str,
        base_sequence: u64,
        total_entries: u64,
    ) {
        let mut lock = self.streams.write();
        let meta = lock.entry(stream_id.to_string()).or_default();
        meta.base_sequence = base_sequence;
        meta.total_entries = total_entries.max(base_sequence);
        meta.checkpoints.clear();
        meta.latest_pointer = None;
        meta.latest_hlc = None;
    }

    /// Retrieves the base sequence (earliest surviving entry) for a stream.
    pub fn base_sequence(&self, stream_id: &str) -> u64 {
        self.streams
            .read()
            .get(stream_id)
            .map(|m| m.base_sequence)
            .unwrap_or(0)
    }

    /// Retrieves stream metadata if the stream exists.
    pub fn get_metadata(&self, stream_id: &str) -> Option<StreamMetadata> {
        self.streams.read().get(stream_id).cloned()
    }

    /// Returns the total entry count for a stream without cloning its checkpoint tree.
    pub fn total_entries(&self, stream_id: &str) -> u64 {
        self.streams
            .read()
            .get(stream_id)
            .map(|m| m.total_entries)
            .unwrap_or(0)
    }

    /// Finds total entry count and the nearest checkpoint before the starting sequence
    /// without cloning the checkpoint map.
    pub fn find_scan_plan(
        &self,
        stream_id: &str,
        limit: usize,
    ) -> Option<(u64, Option<StreamCheckpoint>)> {
        let lock = self.streams.read();
        let meta = lock.get(stream_id)?;
        if meta.total_entries == 0 {
            return None;
        }
        let start_seq = meta
            .total_entries
            .saturating_sub(limit as u64)
            .max(meta.base_sequence);
        let checkpoint = meta
            .checkpoints
            .range(..=start_seq)
            .next_back()
            .map(|(_, cp)| *cp);
        Some((meta.total_entries, checkpoint))
    }

    /// Finds the nearest checkpoint at or before the requested sequence number.
    pub fn find_checkpoint_before(&self, stream_id: &str, seq: u64) -> Option<StreamCheckpoint> {
        let lock = self.streams.read();
        let meta = lock.get(stream_id)?;
        meta.checkpoints
            .range(..=seq)
            .next_back()
            .map(|(_, cp)| *cp)
    }

    /// Returns list of all active stream identifiers.
    pub fn stream_ids(&self) -> Vec<String> {
        self.streams.read().keys().cloned().collect()
    }

    /// Clears all streams from memory.
    pub fn clear(&self) {
        self.streams.write().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_checkpoint_interval() {
        let index = SparseStreamIndex::new(10);
        let hlc = HlcTimestamp::new(1000, 0, 1);

        for i in 0..25 {
            let ptr = RecordPointer::new(1, i * 100, 100, hlc, false);
            index.record_entry("chat-room-1", hlc, ptr);
        }

        let meta = index.get_metadata("chat-room-1").unwrap();
        assert_eq!(meta.total_entries, 25);
        assert_eq!(meta.checkpoints.len(), 3);
        assert!(meta.checkpoints.contains_key(&0));
        assert!(meta.checkpoints.contains_key(&10));
        assert!(meta.checkpoints.contains_key(&20));

        let cp = index.find_checkpoint_before("chat-room-1", 15).unwrap();
        assert_eq!(cp.sequence, 10);
    }
}
