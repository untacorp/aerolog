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

use crate::compactor::{CompactionStats, LogCompactor};
use crate::config::{AeroLogConfig, RetentionPolicy};
use crate::error::{AeroLogError, Result};
use crate::hlc::{HlcGenerator, HlcTimestamp};
use crate::index::keydir::KeyDir;
use crate::index::pointer::RecordPointer;
use crate::index::sparse_stream::SparseStreamIndex;
use crate::log::reader::LogReader;
use crate::log::segment::SegmentFile;
use crate::log::writer::LogWriter;
use crate::record::{Record, RecordType};
use crate::recovery::RecoveryManager;
use crate::stream::{AeroLogStream, StreamEntry};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Primary thread-safe database instance for AeroLog.
#[derive(Debug, Clone)]
pub struct AeroLogDb {
    inner: Arc<InnerDb>,
}

#[derive(Debug)]
struct InnerDb {
    base_dir: PathBuf,
    config: AeroLogConfig,
    keydir: KeyDir,
    sparse_index: SparseStreamIndex,
    hlc_gen: HlcGenerator,
    writer: Mutex<LogWriter>,
    reader: LogReader,
    is_closed: AtomicBool,
    compaction_lock: Mutex<()>,
    stream_retentions: parking_lot::RwLock<HashMap<String, RetentionPolicy>>,
    background_worker_stop: Arc<AtomicBool>,
}

impl AeroLogDb {
    /// Opens or creates an AeroLog database at the specified directory path.
    pub fn open(path: impl AsRef<Path>, config: AeroLogConfig) -> Result<Self> {
        let base_dir = path.as_ref().to_path_buf();
        std::fs::create_dir_all(&base_dir)?;

        let keydir = KeyDir::new();
        let sparse_index = SparseStreamIndex::new(config.sparse_checkpoint_interval);
        let report = RecoveryManager::recover(&base_dir, &keydir, &sparse_index)?;

        let hlc_gen = HlcGenerator::new(config.node_id_hash);
        if report.latest_hlc > HlcTimestamp::ZERO {
            hlc_gen.update(report.latest_hlc);
        }

        let writer = LogWriter::open(
            &base_dir,
            report.active_file_id,
            config.max_segment_size,
            config.sync_mode,
        )?;
        let reader = LogReader::new(&base_dir);

        let stream_retentions = parking_lot::RwLock::new(config.stream_retention_overrides.clone());
        let background_worker_stop = Arc::new(AtomicBool::new(false));
        let auto_compact_interval = config.compaction_interval;

        let inner = Arc::new(InnerDb {
            base_dir,
            config,
            keydir,
            sparse_index,
            hlc_gen,
            writer: Mutex::new(writer),
            reader,
            is_closed: AtomicBool::new(false),
            compaction_lock: Mutex::new(()),
            stream_retentions,
            background_worker_stop,
        });

        let db = Self { inner };
        if let Some(interval) = auto_compact_interval {
            db.start_background_compactor(interval)?;
        }

        Ok(db)
    }

    /// Stores a Key-Value pair, generating a new monotonic HLC timestamp.
    pub fn put(&self, collection: &str, key: &str, value: Vec<u8>) -> Result<HlcTimestamp> {
        self.check_open()?;
        let hlc = self.inner.hlc_gen.now();
        self.put_internal(collection, key, value, hlc)?;
        Ok(hlc)
    }

    /// Stores a Key-Value pair with an explicit remote HLC timestamp (used during peer replication).
    ///
    /// Ratchets the local clock forward. Returns `true` if accepted under LWW rules.
    pub fn put_with_hlc(
        &self,
        collection: &str,
        key: &str,
        value: Vec<u8>,
        remote_hlc: HlcTimestamp,
    ) -> Result<bool> {
        self.check_open()?;
        self.inner.hlc_gen.update(remote_hlc);
        self.put_internal(collection, key, value, remote_hlc)
    }

    fn put_internal(
        &self,
        collection: &str,
        key: &str,
        value: Vec<u8>,
        hlc: HlcTimestamp,
    ) -> Result<bool> {
        let composite_key = RecoveryManager::make_composite_key(collection, key);
        let record = Record::new(
            RecordType::Put,
            hlc,
            composite_key.as_bytes().to_vec(),
            value,
        )?;

        let mut writer = self.inner.writer.lock();
        let (file_id, offset, len) = writer.append(&record)?;
        let ptr = RecordPointer::new(file_id, offset, len, hlc, false);

        let accepted = self.inner.keydir.put(collection, key, ptr);
        Ok(accepted)
    }

    /// Retrieves the payload value for a specific collection and key.
    pub fn get(&self, collection: &str, key: &str) -> Result<Option<Vec<u8>>> {
        self.check_open()?;
        let ptr = match self.inner.keydir.get(collection, key) {
            Some(p) => p,
            None => return Ok(None),
        };

        {
            let mut writer = self.inner.writer.lock();
            if ptr.file_id == writer.active_file_id() {
                writer.flush()?;
            }
        }

        let value = self
            .inner
            .reader
            .read_value(ptr.file_id, ptr.offset, ptr.length)?;
        Ok(Some(value))
    }

    /// Retrieves the complete [`Record`] including header metadata and HLC timestamp.
    pub fn get_record(&self, collection: &str, key: &str) -> Result<Option<Record>> {
        self.check_open()?;
        let ptr = match self.inner.keydir.get(collection, key) {
            Some(p) => p,
            None => return Ok(None),
        };

        {
            let mut writer = self.inner.writer.lock();
            if ptr.file_id == writer.active_file_id() {
                writer.flush()?;
            }
        }

        let record = self
            .inner
            .reader
            .read_record(ptr.file_id, ptr.offset, ptr.length)?;
        Ok(Some(record))
    }

    /// Soft-deletes a key by appending a tombstone record with a new HLC timestamp.
    pub fn delete(&self, collection: &str, key: &str) -> Result<HlcTimestamp> {
        self.check_open()?;
        let hlc = self.inner.hlc_gen.now();
        self.delete_internal(collection, key, hlc)?;
        Ok(hlc)
    }

    /// Soft-deletes a key with an explicit remote HLC timestamp.
    pub fn delete_with_hlc(
        &self,
        collection: &str,
        key: &str,
        remote_hlc: HlcTimestamp,
    ) -> Result<bool> {
        self.check_open()?;
        self.inner.hlc_gen.update(remote_hlc);
        self.delete_internal(collection, key, remote_hlc)
    }

    fn delete_internal(&self, collection: &str, key: &str, hlc: HlcTimestamp) -> Result<bool> {
        let composite_key = RecoveryManager::make_composite_key(collection, key);
        let record = Record::new(
            RecordType::Delete,
            hlc,
            composite_key.as_bytes().to_vec(),
            Vec::new(),
        )?;

        let mut writer = self.inner.writer.lock();
        let (file_id, offset, len) = writer.append(&record)?;
        let ptr = RecordPointer::new(file_id, offset, len, hlc, true);

        let accepted = self.inner.keydir.put(collection, key, ptr);
        Ok(accepted)
    }

    /// Retrieves an append-only stream handle for a specific stream identifier.
    pub fn get_stream(&self, stream_id: &str) -> Result<Arc<dyn AeroLogStream>> {
        self.check_open()?;
        Ok(Arc::new(StreamHandle {
            db: self.clone(),
            stream_id: stream_id.to_string(),
        }))
    }

    /// Retrieves an append-only stream handle, registering or updating its declarative retention policy.
    pub fn get_stream_with_retention(
        &self,
        stream_id: &str,
        retention: RetentionPolicy,
    ) -> Result<Arc<dyn AeroLogStream>> {
        self.check_open()?;
        self.inner
            .stream_retentions
            .write()
            .insert(stream_id.to_string(), retention);
        self.get_stream(stream_id)
    }

    /// Scans and returns all live key-value pairs within a collection.
    pub fn scan_collection(&self, collection: &str) -> Result<Vec<(String, Vec<u8>)>> {
        self.check_open()?;
        let keys = self.inner.keydir.keys_in_collection(collection);
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            if let Some(val) = self.get(collection, &key)? {
                results.push((key, val));
            }
        }

        Ok(results)
    }

    /// Scans all local mutations strictly newer than `since_hlc` (used for delta replication).
    pub fn scan_mutations_since(
        &self,
        since_hlc: Option<HlcTimestamp>,
        limit: usize,
    ) -> Result<Vec<Record>> {
        self.check_open()?;
        self.inner.writer.lock().flush()?;

        let mut candidates = Vec::with_capacity(limit.min(1024));
        let active_file_id = self.inner.writer.lock().active_file_id();

        for file_id in 1..=active_file_id {
            let seg_path = self
                .inner
                .base_dir
                .join(SegmentFile::filename_for_id(file_id));
            if !seg_path.exists() {
                continue;
            }

            let mut reached_limit = false;
            self.inner
                .reader
                .scan_segment_streaming(file_id, |_, record| {
                    let include = match since_hlc {
                        Some(since) => record.header.hlc > since,
                        None => true,
                    };

                    if include {
                        candidates.push(record);
                        if candidates.len() >= limit {
                            reached_limit = true;
                            return Ok(false);
                        }
                    }
                    Ok(true)
                })?;

            if reached_limit {
                break;
            }
        }

        Ok(candidates)
    }

    /// Performs background compaction to merge older sealed segment files.
    pub fn compact(&self) -> Result<CompactionStats> {
        self.check_open()?;
        let _guard = self.inner.compaction_lock.lock();

        let active_id = {
            let mut writer = self.inner.writer.lock();
            writer.roll_segment()?
        };

        self.inner.reader.clear_cache();

        let retentions = self.inner.stream_retentions.read();
        let stats = LogCompactor::compact_with_retentions(
            &self.inner.base_dir,
            active_id,
            &self.inner.keydir,
            &self.inner.config,
            Some(&retentions),
        )?;
        drop(retentions);

        self.inner.reader.clear_cache();
        self.rebuild_sparse_index(&stats.stream_evictions)?;

        Ok(stats)
    }

    fn rebuild_sparse_index(&self, stream_evictions: &HashMap<String, u64>) -> Result<()> {
        let active_file_id = self.inner.writer.lock().active_file_id();

        let mut stream_states: HashMap<String, (u64, u64)> = HashMap::new();
        for stream_id in self.inner.sparse_index.stream_ids() {
            let base = self.inner.sparse_index.base_sequence(&stream_id);
            let total = self.inner.sparse_index.total_entries(&stream_id);
            let evicted = stream_evictions.get(&stream_id).copied().unwrap_or(0);
            let new_base = base + evicted;
            stream_states.insert(stream_id.clone(), (new_base, total));
            self.inner
                .sparse_index
                .reset_stream_checkpoints(&stream_id, new_base, total);
        }

        let mut current_seqs: HashMap<String, u64> = HashMap::new();

        for file_id in 1..=active_file_id {
            let seg_path = self
                .inner
                .base_dir
                .join(SegmentFile::filename_for_id(file_id));
            if !seg_path.exists() {
                continue;
            }

            self.inner
                .reader
                .scan_segment_streaming(file_id, |offset, record| {
                    if record.header.record_type == RecordType::StreamEntry {
                        let total_len = record.header.total_record_size() as u32;
                        let stream_id = String::from_utf8_lossy(&record.key).to_string();
                        let ptr = RecordPointer::new(
                            file_id,
                            offset,
                            total_len,
                            record.header.hlc,
                            false,
                        );

                        let (base_seq, _) =
                            stream_states.entry(stream_id.clone()).or_insert_with(|| {
                                let b = self.inner.sparse_index.base_sequence(&stream_id);
                                (b, b)
                            });
                        let current_seq =
                            current_seqs.entry(stream_id.clone()).or_insert(*base_seq);
                        let is_first = *current_seq == *base_seq;
                        self.inner.sparse_index.record_entry_with_seq(
                            &stream_id,
                            *current_seq,
                            record.header.hlc,
                            ptr,
                            is_first,
                        );
                        *current_seq += 1;
                    }
                    Ok(true)
                })?;
        }
        Ok(())
    }

    /// Starts an optional background compaction worker running at the given interval.
    pub fn start_background_compactor(&self, interval: std::time::Duration) -> Result<()> {
        self.check_open()?;
        self.inner
            .background_worker_stop
            .store(false, Ordering::SeqCst);
        let db_clone = self.clone();
        let stop_flag = self.inner.background_worker_stop.clone();

        std::thread::Builder::new()
            .name("aerolog-compactor".to_string())
            .spawn(move || {
                let tick = std::time::Duration::from_millis(100);
                let total_ticks = (interval.as_millis() / 100).max(1) as u64;
                let mut current_tick = 0u64;

                while !stop_flag.load(Ordering::Relaxed)
                    && !db_clone.inner.is_closed.load(Ordering::Relaxed)
                {
                    std::thread::sleep(tick);
                    current_tick += 1;
                    if current_tick >= total_ticks {
                        current_tick = 0;
                        if stop_flag.load(Ordering::Relaxed)
                            || db_clone.inner.is_closed.load(Ordering::Relaxed)
                        {
                            break;
                        }
                        let _ = db_clone.compact();
                    }
                }
            })
            .map_err(AeroLogError::Io)?;

        Ok(())
    }

    /// Signals the background compaction worker to terminate gracefully.
    pub fn stop_background_compactor(&self) {
        self.inner
            .background_worker_stop
            .store(true, Ordering::SeqCst);
    }

    /// Flushes all pending in-memory buffer writes to disk.
    pub fn flush(&self) -> Result<()> {
        self.check_open()?;
        self.inner.writer.lock().flush()
    }

    /// Flushes and invokes an explicit filesystem sync on the active segment.
    pub fn sync_all(&self) -> Result<()> {
        self.check_open()?;
        self.inner.writer.lock().sync_all()
    }

    /// Closes the database instance.
    pub fn close(&self) -> Result<()> {
        self.stop_background_compactor();
        if !self.inner.is_closed.swap(true, Ordering::SeqCst) {
            let mut writer = self.inner.writer.lock();
            writer.sync_all()?;
        }
        Ok(())
    }

    fn check_open(&self) -> Result<()> {
        if self.inner.is_closed.load(Ordering::Relaxed) {
            Err(AeroLogError::DatabaseClosed)
        } else {
            Ok(())
        }
    }
}

impl Drop for AeroLogDb {
    fn drop(&mut self) {
        self.stop_background_compactor();
    }
}

/// Concrete stream handle implementation over AeroLog.
struct StreamHandle {
    db: AeroLogDb,
    stream_id: String,
}

impl AeroLogStream for StreamHandle {
    fn append(&self, payload: Vec<u8>) -> Result<(u64, HlcTimestamp)> {
        self.db.check_open()?;
        let hlc = self.db.inner.hlc_gen.now();
        let record = Record::new(
            RecordType::StreamEntry,
            hlc,
            self.stream_id.as_bytes().to_vec(),
            payload,
        )?;

        let mut writer = self.db.inner.writer.lock();
        let (file_id, offset, len) = writer.append(&record)?;
        let ptr = RecordPointer::new(file_id, offset, len, hlc, false);

        let seq = self
            .db
            .inner
            .sparse_index
            .record_entry(&self.stream_id, hlc, ptr);

        Ok((seq, hlc))
    }

    fn read_recent(&self, limit: usize) -> Result<Vec<StreamEntry>> {
        self.db.check_open()?;
        // Ensure pending buffered stream appends in active segment are pushed to OS
        self.db.inner.writer.lock().flush()?;

        let (total_entries, checkpoint) = match self
            .db
            .inner
            .sparse_index
            .find_scan_plan(&self.stream_id, limit)
        {
            Some(plan) => plan,
            None => return Ok(Vec::new()),
        };

        let base_seq = self.db.inner.sparse_index.base_sequence(&self.stream_id);
        let start_seq = total_entries.saturating_sub(limit as u64).max(base_seq);
        let (scan_file_id, scan_offset, checkpoint_seq) = match checkpoint {
            Some(cp) => (cp.file_id, cp.offset, cp.sequence),
            None => (1, 0, base_seq),
        };

        let active_file_id = self.db.inner.writer.lock().active_file_id();
        let mut entries = Vec::with_capacity(limit);
        let mut current_seq = checkpoint_seq;
        let stream_bytes = self.stream_id.as_bytes();

        for fid in scan_file_id..=active_file_id {
            let seg_path = self
                .db
                .inner
                .base_dir
                .join(SegmentFile::filename_for_id(fid));
            if !seg_path.exists() {
                continue;
            }

            let start_off = if fid == scan_file_id { scan_offset } else { 0 };
            let mut limit_reached = false;

            self.db
                .inner
                .reader
                .scan_segment_from_offset(fid, start_off, |_offset, record| {
                    if record.header.record_type == RecordType::StreamEntry
                        && record.key == stream_bytes
                    {
                        if current_seq >= start_seq {
                            entries.push(StreamEntry {
                                sequence: current_seq,
                                hlc: record.header.hlc,
                                payload: record.value,
                            });
                        }
                        current_seq += 1;
                        if entries.len() >= limit {
                            limit_reached = true;
                            return Ok(false);
                        }
                    }
                    Ok(true)
                })?;

            if limit_reached {
                break;
            }
        }

        if entries.len() > limit {
            entries = entries.split_off(entries.len() - limit);
        }

        Ok(entries)
    }

    fn total_entries(&self) -> u64 {
        self.db.inner.sparse_index.total_entries(&self.stream_id)
    }

    fn stream_id(&self) -> &str {
        &self.stream_id
    }
}
