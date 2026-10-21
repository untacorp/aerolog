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

use crate::config::{AeroLogConfig, RetentionPolicy, SyncMode};
use crate::error::Result;
use crate::index::keydir::KeyDir;
use crate::index::pointer::RecordPointer;
use crate::log::reader::LogReader;
use crate::log::segment::SegmentFile;
use crate::log::writer::LogWriter;
use crate::record::RecordType;
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Metrics generated upon completion of a log compaction cycle.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompactionStats {
    /// Total disk size in bytes before compaction.
    pub bytes_before: u64,
    /// Total disk size in bytes after compaction.
    pub bytes_after: u64,
    /// Number of stale / overwritten / deleted records evicted.
    pub records_evicted: u64,
}

/// Performs offline or background merging of sealed segment files to reclaim disk space.
pub struct LogCompactor;

impl LogCompactor {
    /// Merges older sealed segments up to `active_file_id - 1`, evicting dead records
    /// and applying stream retention policies.
    pub fn compact(
        base_dir: &Path,
        active_file_id: u32,
        keydir: &KeyDir,
        config: &AeroLogConfig,
    ) -> Result<CompactionStats> {
        if active_file_id <= 1 {
            // Nothing to compact if only 1 active segment exists
            return Ok(CompactionStats::default());
        }

        let mut sealed_files = Vec::new();
        let mut bytes_before = 0u64;

        for entry in std::fs::read_dir(base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(id) = SegmentFile::parse_file_id(&path) {
                if id < active_file_id {
                    sealed_files.push(id);
                    bytes_before += entry.metadata()?.len();
                }
            }
        }

        if sealed_files.is_empty() {
            return Ok(CompactionStats::default());
        }

        sealed_files.sort_unstable();

        // Collect all currently live pointers from KeyDir that reside in sealed files
        let live_pointers: HashMap<(u32, u64), RecordPointer> = keydir
            .snapshot()
            .into_iter()
            .filter(|(_, ptr)| ptr.file_id < active_file_id)
            .map(|(_, ptr)| ((ptr.file_id, ptr.offset), ptr))
            .collect();

        // Pre-count total stream entries across all sealed files to correctly retain the N newest entries
        let reader = LogReader::new(base_dir);
        let mut total_stream_entries: HashMap<String, u64> = HashMap::new();
        for &file_id in &sealed_files {
            if let Ok(records) = reader.scan_segment(file_id) {
                for (_, record) in records {
                    if record.header.record_type == RecordType::StreamEntry {
                        let stream_id = String::from_utf8_lossy(&record.key).to_string();
                        *total_stream_entries.entry(stream_id).or_insert(0) += 1;
                    }
                }
            }
        }

        // Write compacted data into a staging directory
        let staging_dir = base_dir.join(".compaction_staging");
        if staging_dir.exists() {
            std::fs::remove_dir_all(&staging_dir)?;
        }
        std::fs::create_dir_all(&staging_dir)?;

        let mut compactor_writer = LogWriter::open(
            &staging_dir,
            1,
            config.max_segment_size,
            SyncMode::SyncOnWrite,
        )?;

        let mut records_evicted = 0u64;
        let mut updated_pointers: Vec<((String, String), RecordPointer)> = Vec::new();

        let current_time_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Track stream entry counts for MaxEntries retention
        let mut stream_counts: HashMap<String, u64> = HashMap::new();

        for &file_id in &sealed_files {
            let records = reader.scan_segment(file_id)?;

            for (offset, record) in records {
                match record.header.record_type {
                    RecordType::Put => {
                        if let Some(live_ptr) = live_pointers.get(&(file_id, offset)) {
                            if !live_ptr.is_tombstone {
                                // Record is live: copy to compacted segment
                                let key_str = String::from_utf8_lossy(&record.key).to_string();
                                let (collection, key) =
                                    crate::recovery::RecoveryManager::split_composite_key(&key_str);

                                let (new_file_id, new_offset, new_len) =
                                    compactor_writer.append(&record)?;
                                let new_ptr = RecordPointer::new(
                                    new_file_id,
                                    new_offset,
                                    new_len,
                                    record.header.hlc,
                                    false,
                                );
                                updated_pointers.push(((collection, key), new_ptr));
                            } else {
                                records_evicted += 1;
                            }
                        } else {
                            records_evicted += 1;
                        }
                    }
                    RecordType::Delete => {
                        if let Some(live_ptr) = live_pointers.get(&(file_id, offset)) {
                            if live_ptr.is_tombstone {
                                // Tombstone: check if tombstone TTL is configured
                                if let Some(ttl) = config.tombstone_ttl_millis {
                                    if current_time_ms > record.header.hlc.physical_time + ttl {
                                        records_evicted += 1;
                                        continue;
                                    }
                                }
                                // If no TTL or not expired yet, preserve tombstone in KeyDir and segment
                                let (new_file_id, new_offset, new_len) =
                                    compactor_writer.append(&record)?;
                                let key_str = String::from_utf8_lossy(&record.key).to_string();
                                let (collection, key) =
                                    crate::recovery::RecoveryManager::split_composite_key(&key_str);
                                let new_ptr = RecordPointer::new(
                                    new_file_id,
                                    new_offset,
                                    new_len,
                                    record.header.hlc,
                                    true,
                                );
                                updated_pointers.push(((collection, key), new_ptr));
                            } else {
                                records_evicted += 1;
                            }
                        } else {
                            records_evicted += 1;
                        }
                    }
                    RecordType::StreamEntry => {
                        let stream_id = String::from_utf8_lossy(&record.key).to_string();
                        let policy = config.retention_for_stream(&stream_id);

                        let should_retain = match policy {
                            RetentionPolicy::KeepAll => true,
                            RetentionPolicy::TimeToLive(ttl) => {
                                current_time_ms <= record.header.hlc.physical_time + ttl
                            }
                            RetentionPolicy::MaxEntries(max) => {
                                let total =
                                    total_stream_entries.get(&stream_id).copied().unwrap_or(0);
                                let to_skip = total.saturating_sub(*max);
                                let seen = stream_counts.entry(stream_id.clone()).or_insert(0);
                                *seen += 1;
                                *seen > to_skip
                            }
                        };

                        if should_retain {
                            compactor_writer.append(&record)?;
                        } else {
                            records_evicted += 1;
                        }
                    }
                }
            }
        }

        compactor_writer.sync_all()?;

        // Calculate size of compacted staging files
        let mut bytes_after = 0u64;
        for entry in std::fs::read_dir(&staging_dir)? {
            let entry = entry?;
            bytes_after += entry.metadata()?.len();
        }

        // Remove old sealed files
        for &file_id in &sealed_files {
            let old_path = base_dir.join(SegmentFile::filename_for_id(file_id));
            if old_path.exists() {
                let _ = std::fs::remove_file(old_path);
            }
        }

        // Ensure compacted files do not collide with or overwrite the active segment
        for entry in std::fs::read_dir(&staging_dir)? {
            let entry = entry?;
            if let Some(fid) = SegmentFile::parse_file_id(&entry.path()) {
                if fid >= active_file_id {
                    return Err(crate::error::AeroLogError::CompactionError(format!(
                        "Compaction produced segment ID {fid} which collides with active segment {active_file_id}"
                    )));
                }
            }
        }

        // Move compacted files from staging to base directory
        for entry in std::fs::read_dir(&staging_dir)? {
            let entry = entry?;
            let dest = base_dir.join(entry.file_name());
            std::fs::rename(entry.path(), dest)?;
        }

        let _ = std::fs::remove_dir_all(&staging_dir);

        // Update KeyDir with relocated pointers
        for ((collection, key), new_ptr) in updated_pointers {
            keydir.put(&collection, &key, new_ptr);
        }

        Ok(CompactionStats {
            bytes_before,
            bytes_after,
            records_evicted,
        })
    }
}
