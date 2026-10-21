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
use crate::log::segment::SegmentFile;
use crate::record::{Record, RecordHeader, HEADER_SIZE};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Reader for random-access and sequential reading of `.aerolog` segment files.
#[derive(Debug, Clone)]
pub struct LogReader {
    base_dir: PathBuf,
}

impl LogReader {
    /// Creates a new reader rooted at the given database directory.
    pub fn new(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
        }
    }

    /// Reads and verifies a record from a specific segment, offset, and length.
    pub fn read_record(&self, file_id: u32, offset: u64, length: u32) -> Result<Record> {
        let segment_path = self.base_dir.join(SegmentFile::filename_for_id(file_id));
        if !segment_path.exists() {
            return Err(AeroLogError::SegmentNotFound(file_id));
        }

        let mut file = File::open(&segment_path)?;
        file.seek(SeekFrom::Start(offset))?;

        let mut buf = vec![0u8; length as usize];
        file.read_exact(&mut buf)?;

        if buf.len() < HEADER_SIZE {
            return Err(AeroLogError::CorruptRecord {
                offset,
                reason: "Byte length is less than fixed 32-byte header".to_string(),
            });
        }

        let mut header_bytes = [0u8; HEADER_SIZE];
        header_bytes.copy_from_slice(&buf[0..HEADER_SIZE]);
        let header = RecordHeader::decode(&header_bytes)?;

        let key_end = HEADER_SIZE + header.key_len as usize;
        let val_end = key_end + header.value_len as usize;

        if val_end != buf.len() {
            return Err(AeroLogError::CorruptRecord {
                offset,
                reason: format!(
                    "Buffer length {} does not match expected payload bounds {}",
                    buf.len(),
                    val_end
                ),
            });
        }

        let key = buf[HEADER_SIZE..key_end].to_vec();
        let value = buf[key_end..val_end].to_vec();

        let record = Record { header, key, value };
        record.verify_checksum()?;

        Ok(record)
    }

    /// Scans an entire segment file sequentially from the beginning,
    /// yielding `(offset, record)` pairs. Stops cleanly upon encountering EOF.
    pub fn scan_segment(&self, file_id: u32) -> Result<Vec<(u64, Record)>> {
        let segment_path = self.base_dir.join(SegmentFile::filename_for_id(file_id));
        if !segment_path.exists() {
            return Err(AeroLogError::SegmentNotFound(file_id));
        }

        let mut file = File::open(&segment_path)?;
        let mut offset = 0u64;
        let mut records = Vec::new();

        let mut header_buf = [0u8; HEADER_SIZE];

        loop {
            match file.read_exact(&mut header_buf) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // Clean EOF reached
                    break;
                }
                Err(e) => return Err(AeroLogError::Io(e)),
            }

            let header = match RecordHeader::decode(&header_buf) {
                Ok(h) => h,
                Err(AeroLogError::InvalidMagic { .. }) => {
                    // Possible zero-padding or incomplete crash fragment at the end of file
                    break;
                }
                Err(e) => return Err(e),
            };

            let payload_size = (header.key_len as usize) + (header.value_len as usize);
            let mut payload = vec![0u8; payload_size];
            if let Err(e) = file.read_exact(&mut payload) {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    // Incomplete trailing write from previous process crash: safely ignore
                    break;
                }
                return Err(AeroLogError::Io(e));
            }

            let key = payload[0..header.key_len as usize].to_vec();
            let value = payload[header.key_len as usize..].to_vec();

            let record = Record { header, key, value };
            if record.verify_checksum().is_err() {
                // Corrupted record encountered: stop scanning
                break;
            }

            let total_size = header.total_record_size();
            records.push((offset, record));
            offset += total_size;
        }

        Ok(records)
    }
}
