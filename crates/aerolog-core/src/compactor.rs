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
#[derive(Debug, Clone, Default)]
pub struct CompactionStats {
    /// Total disk size in bytes before compaction.
    pub bytes_before: u64,
    /// Total disk size in bytes after compaction.
    pub bytes_after: u64,
    /// Number of stale / overwritten / deleted records evicted.
    pub records_evicted: u64,
    /// Evicted record counts per stream partition (for sequence offset preservation).
    pub stream_evictions: HashMap<String, u64>,
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
        Self::compact_with_retentions(base_dir, active_file_id, keydir, config, None)
    }

    /// Merges older sealed segments with optional dynamic per-stream retention policy overrides.
    pub fn compact_with_retentions(
        base_dir: &Path,
        active_file_id: u32,
        keydir: &KeyDir,
        config: &AeroLogConfig,
        custom_retentions: Option<&HashMap<String, RetentionPolicy>>,
    ) -> Result<CompactionStats> {
        if active_file_id <= 1 {
            return Ok(CompactionStats::default());
        }

        let mut sealed_files = Vec::new();
        let mut bytes_before = 0u64;

        for entry in std::fs::read_dir(base_dir)? {
            let entry = entry?;
            if let Some(id) =
                SegmentFile::parse_file_id(&entry.path()).filter(|&id| id < active_file_id)
            {
                sealed_files.push(id);
                bytes_before += entry.metadata()?.len();
            }
        }

        if sealed_files.is_empty() {
            return Ok(CompactionStats::default());
        }

        sealed_files.sort_unstable();

        let live_pointers: HashMap<(u32, u64), RecordPointer> = keydir
            .snapshot()
            .into_iter()
            .filter(|(_, ptr)| ptr.file_id < active_file_id)
            .map(|(_, ptr)| ((ptr.file_id, ptr.offset), ptr))
            .collect();

        let reader = LogReader::new(base_dir);
        let mut total_stream_entries: HashMap<String, u64> = HashMap::new();
        let has_max = |p: &_| matches!(p, RetentionPolicy::MaxEntries(_));
        let has_max_entries_policy = has_max(&config.default_stream_retention)
            || config.stream_retention_overrides.values().any(has_max)
            || custom_retentions.is_some_and(|m| m.values().any(has_max));

        if has_max_entries_policy {
            for &file_id in &sealed_files {
                let _ = reader.scan_segment_streaming(file_id, |_, record| {
                    if record.header.record_type == RecordType::StreamEntry {
                        let stream_id = String::from_utf8_lossy(&record.key).to_string();
                        *total_stream_entries.entry(stream_id).or_insert(0) += 1;
                    }
                    Ok(true)
                });
            }
        }

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
        let mut purged_tombstones: Vec<((String, String), crate::hlc::HlcTimestamp)> = Vec::new();
        let mut stream_evictions: HashMap<String, u64> = HashMap::new();

        let current_time_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut stream_counts: HashMap<String, u64> = HashMap::new();

        for &file_id in &sealed_files {
            let _ = reader.scan_segment_streaming(file_id, |offset, record| {
                match record.header.record_type {
                    RecordType::Put => {
                        let Some(_) = live_pointers
                            .get(&(file_id, offset))
                            .filter(|l| !l.is_tombstone)
                        else {
                            records_evicted += 1;
                            return Ok(true);
                        };
                        let (col, key) = crate::recovery::RecoveryManager::split_composite_key(
                            &String::from_utf8_lossy(&record.key),
                        );
                        let (fid, off, len) = compactor_writer.append(&record)?;
                        updated_pointers.push((
                            (col, key),
                            RecordPointer::new(fid, off, len, record.header.hlc, false),
                        ));
                    }
                    RecordType::Delete => {
                        let Some(_) = live_pointers
                            .get(&(file_id, offset))
                            .filter(|l| l.is_tombstone)
                        else {
                            records_evicted += 1;
                            return Ok(true);
                        };
                        let (col, key) = crate::recovery::RecoveryManager::split_composite_key(
                            &String::from_utf8_lossy(&record.key),
                        );
                        if let Some(ttl) = config.tombstone_ttl_millis {
                            if current_time_ms > record.header.hlc.physical_time + ttl {
                                records_evicted += 1;
                                purged_tombstones.push(((col, key), record.header.hlc));
                                return Ok(true);
                            }
                        }
                        let (fid, off, len) = compactor_writer.append(&record)?;
                        updated_pointers.push((
                            (col, key),
                            RecordPointer::new(fid, off, len, record.header.hlc, true),
                        ));
                    }
                    RecordType::StreamEntry => {
                        let stream_id = String::from_utf8_lossy(&record.key).to_string();
                        let policy = custom_retentions
                            .and_then(|m| m.get(&stream_id))
                            .unwrap_or_else(|| config.retention_for_stream(&stream_id));

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
                            *stream_evictions.entry(stream_id).or_insert(0) += 1;
                        }
                    }
                }
                Ok(true)
            });
        }

        compactor_writer.sync_all()?;

        let staging_entries: Vec<_> =
            std::fs::read_dir(&staging_dir)?.collect::<std::result::Result<Vec<_>, _>>()?;

        let mut bytes_after = 0u64;
        let mut compacted_file_ids = std::collections::HashSet::new();
        for entry in &staging_entries {
            bytes_after += entry.metadata()?.len();
            if let Some(fid) = SegmentFile::parse_file_id(&entry.path()) {
                if fid >= active_file_id {
                    return Err(crate::error::AeroLogError::CompactionError(format!(
                        "Compaction produced segment ID {fid} which collides with active segment {active_file_id}"
                    )));
                }
                compacted_file_ids.insert(fid);
            }
        }

        for entry in staging_entries {
            std::fs::rename(entry.path(), base_dir.join(entry.file_name()))?;
        }

        let _ = std::fs::remove_dir_all(&staging_dir);

        for ((col, key), ptr) in updated_pointers {
            keydir.put(&col, &key, ptr);
        }
        for ((col, key), hlc) in purged_tombstones {
            keydir.remove_tombstone(&col, &key, hlc);
        }
        for &fid in &sealed_files {
            if !compacted_file_ids.contains(&fid) {
                let _ = std::fs::remove_file(base_dir.join(SegmentFile::filename_for_id(fid)));
            }
        }

        Ok(CompactionStats {
            bytes_before,
            bytes_after,
            records_evicted,
            stream_evictions,
        })
    }
}
