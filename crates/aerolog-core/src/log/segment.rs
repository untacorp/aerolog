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

use crate::error::{AeroLogError, Result};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

/// Extension for AeroLog segment data files.
pub const SEGMENT_FILE_EXTENSION: &str = "aerolog";

/// Standard prefix for segment data files.
pub const SEGMENT_FILE_PREFIX: &str = "data-";

/// Manages metadata and file handles for an individual `.aerolog` segment file.
#[derive(Debug)]
pub struct SegmentFile {
    /// Monotonically increasing identifier for this segment file.
    pub file_id: u32,
    /// Absolute path to the segment file on disk.
    pub path: PathBuf,
    /// Current on-disk byte size of this segment.
    pub size: u64,
    /// Whether this segment is the currently active (writable) segment.
    pub is_active: bool,
}

impl SegmentFile {
    /// Constructs the standard filename for a given file ID.
    pub fn filename_for_id(file_id: u32) -> String {
        format!("{SEGMENT_FILE_PREFIX}{file_id:06}.{SEGMENT_FILE_EXTENSION}")
    }

    /// Attempts to parse a segment file ID from a filename or path.
    pub fn parse_file_id(path: &Path) -> Option<u32> {
        let filename = path.file_name()?.to_str()?;
        if !filename.starts_with(SEGMENT_FILE_PREFIX) || !filename.ends_with(SEGMENT_FILE_EXTENSION)
        {
            return None;
        }

        let prefix_len = SEGMENT_FILE_PREFIX.len();
        let suffix_len = SEGMENT_FILE_EXTENSION.len() + 1; // including dot
        let num_str = &filename[prefix_len..filename.len() - suffix_len];
        num_str.parse::<u32>().ok()
    }

    /// Opens or creates a segment file on disk.
    pub fn open(directory: &Path, file_id: u32, is_active: bool) -> Result<Self> {
        let path = directory.join(Self::filename_for_id(file_id));
        let size = if path.exists() {
            let metadata = std::fs::metadata(&path)?;
            metadata.len()
        } else {
            // Create file if it does not exist
            let _ = OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .read(true)
                .open(&path)?;
            0
        };

        Ok(Self {
            file_id,
            path,
            size,
            is_active,
        })
    }

    /// Opens an append-mode file handle for writing to this segment.
    pub fn open_for_append(&self) -> Result<File> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&self.path)
            .map_err(AeroLogError::Io)
    }

    /// Opens a read-only file handle for random-access reads from this segment.
    pub fn open_for_read(&self) -> Result<File> {
        OpenOptions::new()
            .read(true)
            .open(&self.path)
            .map_err(AeroLogError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filename_generation_and_parsing() {
        let filename = SegmentFile::filename_for_id(42);
        assert_eq!(filename, "data-000042.aerolog");

        let parsed_id = SegmentFile::parse_file_id(Path::new(&filename)).unwrap();
        assert_eq!(parsed_id, 42);

        let invalid = SegmentFile::parse_file_id(Path::new("not-a-segment.txt"));
        assert!(invalid.is_none());
    }
}
