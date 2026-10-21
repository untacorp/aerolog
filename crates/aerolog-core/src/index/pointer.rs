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

/// Disk pointer referencing the location and length of a specific record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordPointer {
    /// Identifier of the segment file containing this record.
    pub file_id: u32,
    /// Byte offset within the segment file.
    pub offset: u64,
    /// Total byte length of the record (header + key + value).
    pub length: u32,
    /// Hybrid Logical Clock timestamp when this record was authored.
    pub hlc: HlcTimestamp,
    /// Whether this pointer represents a soft-deleted tombstone.
    pub is_tombstone: bool,
}

impl RecordPointer {
    /// Creates a new active record pointer.
    pub const fn new(
        file_id: u32,
        offset: u64,
        length: u32,
        hlc: HlcTimestamp,
        is_tombstone: bool,
    ) -> Self {
        Self {
            file_id,
            offset,
            length,
            hlc,
            is_tombstone,
        }
    }
}

/// Sparse checkpoint pointer into a stream partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamCheckpoint {
    /// Monotonically increasing sequence number within the stream.
    pub sequence: u64,
    /// HLC timestamp of the entry.
    pub hlc: HlcTimestamp,
    /// Segment file ID.
    pub file_id: u32,
    /// Byte offset in segment file.
    pub offset: u64,
}
