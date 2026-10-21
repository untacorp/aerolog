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

use crate::config::SyncMode;
use crate::error::{AeroLogError, Result};
use crate::log::segment::SegmentFile;
use crate::record::Record;
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Append-only sequential writer for the active `.aerolog` segment file.
#[derive(Debug)]
pub struct LogWriter {
    base_dir: PathBuf,
    active_segment: SegmentFile,
    file_writer: BufWriter<File>,
    current_offset: u64,
    max_segment_size: u64,
    sync_mode: SyncMode,
}

impl LogWriter {
    /// Opens or resumes the log writer in the specified directory.
    pub fn open(
        base_dir: &Path,
        file_id: u32,
        max_segment_size: u64,
        sync_mode: SyncMode,
    ) -> Result<Self> {
        let segment = SegmentFile::open(base_dir, file_id, true)?;
        let mut file = segment.open_for_append()?;
        let current_offset = file.seek(SeekFrom::End(0))?;

        let file_writer = BufWriter::with_capacity(64 * 1024, file);

        Ok(Self {
            base_dir: base_dir.to_path_buf(),
            active_segment: segment,
            file_writer,
            current_offset,
            max_segment_size,
            sync_mode,
        })
    }

    /// Appends a serialized record to the active segment.
    ///
    /// Automatically handles segment rolling if the new record exceeds capacity.
    /// Returns `(file_id, offset, total_record_length)`.
    pub fn append(&mut self, record: &Record) -> Result<(u32, u64, u32)> {
        let record_len = record.header.total_record_size() as u32;

        // Check if we need to roll the segment before writing
        if self.current_offset > 0
            && (self.current_offset + record_len as u64) > self.max_segment_size
        {
            self.roll_segment()?;
        }

        let write_offset = self.current_offset;
        let header_bytes = record.header.encode();
        self.file_writer.write_all(&header_bytes)?;
        self.file_writer.write_all(&record.key)?;
        self.file_writer.write_all(&record.value)?;
        self.file_writer.flush()?;

        if self.sync_mode == SyncMode::SyncOnWrite {
            self.file_writer.get_ref().sync_data()?;
        }

        self.current_offset += record_len as u64;
        self.active_segment.size = self.current_offset;

        Ok((self.active_segment.file_id, write_offset, record_len))
    }

    /// Rolls the active segment: flushes current file, seals it, and creates a new one with file_id + 1.
    pub fn roll_segment(&mut self) -> Result<u32> {
        self.flush()?;
        self.file_writer.get_ref().sync_all()?;

        let next_file_id = self.active_segment.file_id + 1;
        let new_segment = SegmentFile::open(&self.base_dir, next_file_id, true)?;
        let new_file = new_segment.open_for_append()?;

        self.active_segment.is_active = false;
        self.active_segment = new_segment;
        self.file_writer = BufWriter::with_capacity(64 * 1024, new_file);
        self.current_offset = 0;

        Ok(next_file_id)
    }

    /// Flushes buffered data to the operating system file descriptor.
    pub fn flush(&mut self) -> Result<()> {
        self.file_writer.flush().map_err(AeroLogError::Io)
    }

    /// Flushes and issues an explicit filesystem `sync_all()`.
    pub fn sync_all(&mut self) -> Result<()> {
        self.flush()?;
        self.file_writer
            .get_ref()
            .sync_all()
            .map_err(AeroLogError::Io)
    }

    /// Returns the active segment file ID.
    pub fn active_file_id(&self) -> u32 {
        self.active_segment.file_id
    }

    /// Returns current write byte offset in the active segment.
    pub fn current_offset(&self) -> u64 {
        self.current_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hlc::HlcTimestamp;
    use crate::record::RecordType;
    use tempfile::tempdir;

    #[test]
    fn test_log_writer_append_and_roll() {
        let dir = tempdir().unwrap();
        // Record size is 36 bytes (32 header + 2 key + 2 val).
        // Set segment size to 50 bytes so the second append triggers a roll.
        let mut writer = LogWriter::open(dir.path(), 1, 50, SyncMode::SyncOnWrite).unwrap();

        let hlc = HlcTimestamp::new(1000, 0, 1);
        let record = Record::new(RecordType::Put, hlc, b"k1".to_vec(), b"v1".to_vec()).unwrap();

        let (file_id_1, offset_1, _len_1) = writer.append(&record).unwrap();
        assert_eq!(file_id_1, 1);
        assert_eq!(offset_1, 0);

        // Next write should cause a roll because 36 + 36 > 50
        let (file_id_2, offset_2, _len_2) = writer.append(&record).unwrap();
        assert_eq!(file_id_2, 2);
        assert_eq!(offset_2, 0);
    }
}
