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
use crate::index::keydir::KeyDir;
use crate::index::pointer::RecordPointer;
use crate::index::sparse_stream::SparseStreamIndex;
use crate::log::reader::LogReader;
use crate::log::segment::SegmentFile;
use crate::record::RecordType;
use std::path::Path;

/// Result summary of the startup recovery process.
#[derive(Debug, Clone)]
pub struct RecoveryReport {
    /// Highest segment file ID found on disk.
    pub active_file_id: u32,
    /// Highest HLC timestamp encountered across all valid records.
    pub latest_hlc: HlcTimestamp,
    /// Total number of valid records re-indexed into memory.
    pub total_records: u64,
}

/// Discovers, validates, and rebuilds memory indexes from `.aerolog` segment files.
pub struct RecoveryManager;

impl RecoveryManager {
    /// Scans the database directory and rebuilds the [`KeyDir`] and [`SparseStreamIndex`].
    pub fn recover(
        base_dir: &Path,
        keydir: &KeyDir,
        sparse_index: &SparseStreamIndex,
    ) -> Result<RecoveryReport> {
        let mut segment_ids = Vec::new();

        if base_dir.exists() {
            for entry in std::fs::read_dir(base_dir)? {
                let entry = entry?;
                let path = entry.path();
                if let Some(file_id) = SegmentFile::parse_file_id(&path) {
                    segment_ids.push(file_id);
                }
            }
        }

        segment_ids.sort_unstable();

        let reader = LogReader::new(base_dir);
        let mut latest_hlc = HlcTimestamp::ZERO;
        let mut total_records = 0u64;

        for &file_id in &segment_ids {
            let scanned_records = reader.scan_segment(file_id)?;

            for (offset, record) in scanned_records {
                let total_len = record.header.total_record_size() as u32;
                let hlc = record.header.hlc;
                if hlc > latest_hlc {
                    latest_hlc = hlc;
                }

                match record.header.record_type {
                    RecordType::Put => {
                        let key_str = String::from_utf8_lossy(&record.key).to_string();
                        let (collection, key) = Self::split_composite_key(&key_str);
                        let ptr = RecordPointer::new(file_id, offset, total_len, hlc, false);
                        keydir.put(&collection, &key, ptr);
                    }
                    RecordType::Delete => {
                        let key_str = String::from_utf8_lossy(&record.key).to_string();
                        let (collection, key) = Self::split_composite_key(&key_str);
                        let ptr = RecordPointer::new(file_id, offset, total_len, hlc, true);
                        keydir.put(&collection, &key, ptr);
                    }
                    RecordType::StreamEntry => {
                        let stream_id = String::from_utf8_lossy(&record.key).to_string();
                        let ptr = RecordPointer::new(file_id, offset, total_len, hlc, false);
                        sparse_index.record_entry(&stream_id, hlc, ptr);
                    }
                }

                total_records += 1;
            }
        }

        let active_file_id = segment_ids.last().copied().unwrap_or(1);

        Ok(RecoveryReport {
            active_file_id,
            latest_hlc,
            total_records,
        })
    }

    /// Splits a stored key formatted as `collection:key` into separate collection and key strings.
    pub fn split_composite_key(raw: &str) -> (String, String) {
        if let Some(pos) = raw.find(':') {
            (raw[..pos].to_string(), raw[pos + 1..].to_string())
        } else {
            ("default".to_string(), raw.to_string())
        }
    }

    /// Formats collection and key into a single composite string: `collection:key`.
    pub fn make_composite_key(collection: &str, key: &str) -> String {
        format!("{collection}:{key}")
    }
}
