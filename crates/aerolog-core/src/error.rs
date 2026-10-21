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

use std::io;
use thiserror::Error;

/// Result alias for AeroLog operations.
pub type Result<T> = std::result::Result<T, AeroLogError>;

/// Errors that can occur during AeroLog engine operations.
#[derive(Error, Debug)]
pub enum AeroLogError {
    /// Underlying filesystem I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// CRC32 checksum mismatch indicating segment corruption.
    #[error("CRC32 mismatch: expected 0x{expected:08X}, calculated 0x{actual:08X}")]
    CrcMismatch {
        /// Expected checksum recorded in header.
        expected: u32,
        /// Actual checksum computed over record contents.
        actual: u32,
    },

    /// Magic bytes did not match AeroLog standard (0x41 0x4C / "AL").
    #[error("Invalid magic bytes: expected 0x{expected:04X}, found 0x{actual:04X}")]
    InvalidMagic {
        /// Expected magic number.
        expected: u16,
        /// Actual magic bytes found on disk.
        actual: u16,
    },

    /// Unsupported format version.
    #[error("Unsupported record version: {0}")]
    UnsupportedVersion(u8),

    /// Corrupted record encountered during parsing or recovery.
    #[error("Corrupt record at offset {offset}: {reason}")]
    CorruptRecord {
        /// Byte offset in segment file.
        offset: u64,
        /// Description of the corruption.
        reason: String,
    },

    /// Active segment has reached its configured capacity limit.
    #[error("Active segment file {0} is full")]
    SegmentFull(u32),

    /// Referenced segment file could not be found.
    #[error("Segment file {0} not found")]
    SegmentNotFound(u32),

    /// Key not found in KeyDir index.
    #[error("Key '{key}' not found in collection '{collection}'")]
    KeyNotFound {
        /// Collection name.
        collection: String,
        /// Key name.
        key: String,
    },

    /// Stream not found in sparse stream index.
    #[error("Stream '{0}' not found")]
    StreamNotFound(String),

    /// Payload or key exceeds maximum allowable byte size.
    #[error("Size limit exceeded: {0}")]
    SizeLimitExceeded(String),

    /// An operation was attempted on a closed database instance.
    #[error("Database has been closed")]
    DatabaseClosed,

    /// Error occurred during background compaction.
    #[error("Compaction error: {0}")]
    CompactionError(String),
}
