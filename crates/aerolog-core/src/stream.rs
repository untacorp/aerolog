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

use crate::error::Result;
use crate::hlc::HlcTimestamp;

/// Model representing a single entry retrieved from an append-only stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamEntry {
    /// Monotonically increasing sequence number within the stream.
    pub sequence: u64,
    /// Hybrid Logical Clock timestamp of the entry.
    pub hlc: HlcTimestamp,
    /// Raw payload bytes of the message or event.
    pub payload: Vec<u8>,
}

/// High-level interface for reading and writing to an append-only stream partition.
pub trait AeroLogStream: Send + Sync {
    /// Appends a new payload to the stream.
    /// Returns the assigned sequence number and HLC timestamp.
    fn append(&self, payload: Vec<u8>) -> Result<(u64, HlcTimestamp)>;

    /// Reads up to `limit` of the most recent entries in the stream, ordered chronologically.
    fn read_recent(&self, limit: usize) -> Result<Vec<StreamEntry>>;

    /// Returns the total number of entries recorded in this stream.
    fn total_entries(&self) -> u64;

    /// Returns the unique identifier of this stream.
    fn stream_id(&self) -> &str;
}
