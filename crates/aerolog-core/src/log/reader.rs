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
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Reader for random-access and sequential reading of `.aerolog` segment files.
#[derive(Debug, Clone)]
pub struct LogReader {
    base_dir: PathBuf,
    file_cache: Arc<RwLock<HashMap<u32, Arc<File>>>>,
}

impl LogReader {
    /// Creates a new reader rooted at the given database directory.
    pub fn new(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
            file_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn get_file(&self, file_id: u32) -> Result<Arc<File>> {
        {
            let cache = self.file_cache.read();
            if let Some(f) = cache.get(&file_id) {
                return Ok(Arc::clone(f));
            }
        }

        let segment_path = self.base_dir.join(SegmentFile::filename_for_id(file_id));
        if !segment_path.exists() {
            return Err(AeroLogError::SegmentNotFound(file_id));
        }

        let file = Arc::new(File::open(&segment_path)?);
        let mut cache = self.file_cache.write();
        // Prevent descriptor exhaustion (EMFILE) by bounding the open handle cache
        if cache.len() >= 64 {
            if let Some(&victim) = cache.keys().next() {
                cache.remove(&victim);
            }
        }
        cache.insert(file_id, Arc::clone(&file));
        Ok(file)
    }

    fn read_bytes_at(&self, file: &File, offset: u64, buf: &mut [u8]) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileExt;
            file.read_exact_at(buf, offset).map_err(AeroLogError::Io)
        }
        #[cfg(not(unix))]
        {
            let mut cloned = file.try_clone().map_err(AeroLogError::Io)?;
            cloned
                .seek(SeekFrom::Start(offset))
                .map_err(AeroLogError::Io)?;
            cloned.read_exact(buf).map_err(AeroLogError::Io)
        }
    }

    /// Evicts a specific segment file from the descriptor cache.
    pub fn invalidate_cache(&self, file_id: u32) {
        let mut cache = self.file_cache.write();
        cache.remove(&file_id);
    }

    /// Clears the entire file descriptor cache.
    pub fn clear_cache(&self) {
        let mut cache = self.file_cache.write();
        cache.clear();
    }

    fn read_payload_buf(
        &self,
        file_id: u32,
        offset: u64,
        length: u32,
    ) -> Result<(RecordHeader, Vec<u8>)> {
        if (length as usize) < HEADER_SIZE {
            return Err(AeroLogError::CorruptRecord {
                offset,
                reason: "Byte length is less than fixed 32-byte header".to_string(),
            });
        }

        let file = self.get_file(file_id)?;
        let mut buf = vec![0u8; length as usize];
        self.read_bytes_at(&file, offset, &mut buf)?;

        let mut header_bytes = [0u8; HEADER_SIZE];
        header_bytes.copy_from_slice(&buf[0..HEADER_SIZE]);
        let header = RecordHeader::decode(&header_bytes)?;

        let val_end = HEADER_SIZE + (header.key_len as usize) + (header.value_len as usize);
        if val_end != buf.len() {
            return Err(AeroLogError::CorruptRecord {
                offset,
                reason: format!(
                    "Buffer length {} does not match expected bounds {}",
                    buf.len(),
                    val_end
                ),
            });
        }

        Ok((header, buf))
    }

    /// Reads and verifies a record from a specific segment, offset, and length.
    pub fn read_record(&self, file_id: u32, offset: u64, length: u32) -> Result<Record> {
        let (header, buf) = self.read_payload_buf(file_id, offset, length)?;
        let key_end = HEADER_SIZE + header.key_len as usize;
        let record = Record {
            header,
            key: buf[HEADER_SIZE..key_end].to_vec(),
            value: buf[key_end..].to_vec(),
        };
        record.verify_checksum()?;
        Ok(record)
    }

    /// Reads and verifies only the payload value, avoiding unnecessary key allocations.
    pub fn read_value(&self, file_id: u32, offset: u64, length: u32) -> Result<Vec<u8>> {
        let (header, mut buf) = self.read_payload_buf(file_id, offset, length)?;
        let key_end = HEADER_SIZE + header.key_len as usize;
        let expected = RecordHeader::compute_crc(
            header.record_type,
            &header.hlc,
            &buf[HEADER_SIZE..key_end],
            &buf[key_end..],
        );
        if expected != header.crc32 {
            return Err(AeroLogError::CrcMismatch {
                expected: header.crc32,
                actual: expected,
            });
        }
        buf.drain(..key_end);
        Ok(buf)
    }

    /// Streams through a segment file starting at `start_offset`, invoking `visitor`
    /// for each valid record. Returning `Ok(false)` from visitor terminates the scan early.
    pub fn scan_segment_from_offset<F>(
        &self,
        file_id: u32,
        start_offset: u64,
        mut visitor: F,
    ) -> Result<()>
    where
        F: FnMut(u64, Record) -> Result<bool>,
    {
        let segment_path = self.base_dir.join(SegmentFile::filename_for_id(file_id));
        if !segment_path.exists() {
            return Err(AeroLogError::SegmentNotFound(file_id));
        }

        let mut file = File::open(&segment_path)?;
        if start_offset > 0 {
            file.seek(SeekFrom::Start(start_offset))?;
        }

        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut offset = start_offset;
        let mut header_buf = [0u8; HEADER_SIZE];

        loop {
            match reader.read_exact(&mut header_buf) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    break;
                }
                Err(e) => return Err(AeroLogError::Io(e)),
            }

            let header = match RecordHeader::decode(&header_buf) {
                Ok(h) => h,
                Err(AeroLogError::InvalidMagic { .. }) => {
                    break;
                }
                Err(e) => return Err(e),
            };

            let payload_size = (header.key_len as usize) + (header.value_len as usize);
            let mut payload = vec![0u8; payload_size];
            if let Err(e) = reader.read_exact(&mut payload) {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    break;
                }
                return Err(AeroLogError::Io(e));
            }

            let key = payload[0..header.key_len as usize].to_vec();
            let value = payload[header.key_len as usize..].to_vec();

            let record = Record { header, key, value };
            if record.verify_checksum().is_err() {
                break;
            }

            let total_size = header.total_record_size();
            let should_continue = visitor(offset, record)?;
            offset += total_size;

            if !should_continue {
                break;
            }
        }

        Ok(())
    }

    /// Streams through a segment file sequentially using buffered I/O, invoking `visitor`
    /// for each valid record. Returning `Ok(false)` from visitor terminates the scan early.
    pub fn scan_segment_streaming<F>(&self, file_id: u32, visitor: F) -> Result<()>
    where
        F: FnMut(u64, Record) -> Result<bool>,
    {
        self.scan_segment_from_offset(file_id, 0, visitor)
    }

    /// Scans an entire segment file sequentially from the beginning,
    /// yielding `(offset, record)` pairs. Stops cleanly upon encountering EOF.
    pub fn scan_segment(&self, file_id: u32) -> Result<Vec<(u64, Record)>> {
        let mut records = Vec::new();
        self.scan_segment_streaming(file_id, |offset, record| {
            records.push((offset, record));
            Ok(true)
        })?;
        Ok(records)
    }
}
