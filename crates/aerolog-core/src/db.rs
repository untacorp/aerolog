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
use crate::config::AeroLogConfig;
use crate::error::{AeroLogError, Result};
use crate::hlc::{HlcGenerator, HlcTimestamp};
use crate::index::keydir::KeyDir;
use crate::index::pointer::RecordPointer;
use crate::index::sparse_stream::SparseStreamIndex;
use crate::log::reader::LogReader;
use crate::log::writer::LogWriter;
use crate::record::{Record, RecordType};
use crate::recovery::RecoveryManager;
use crate::stream::{AeroLogStream, StreamEntry};
use parking_lot::Mutex;
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
}

impl AeroLogDb {
    /// Opens or creates an AeroLog database at the specified directory path.
    pub fn open(path: impl AsRef<Path>, config: AeroLogConfig) -> Result<Self> {
        let base_dir = path.as_ref().to_path_buf();
        std::fs::create_dir_all(&base_dir)?;

        let keydir = KeyDir::new();
        let sparse_index = SparseStreamIndex::new(config.sparse_checkpoint_interval);

        // Scan existing segments and recover state
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
        });

        Ok(Self { inner })
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

        let record = self
            .inner
            .reader
            .read_record(ptr.file_id, ptr.offset, ptr.length)?;
        Ok(Some(record.value))
    }

    /// Retrieves the complete [`Record`] including header metadata and HLC timestamp.
    pub fn get_record(&self, collection: &str, key: &str) -> Result<Option<Record>> {
        self.check_open()?;
        let ptr = match self.inner.keydir.get(collection, key) {
            Some(p) => p,
            None => return Ok(None),
        };

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

    /// Scans all local mutations strictly newer than `since_hlc` (used by AirDB replication).
    pub fn scan_mutations_since(
        &self,
        since_hlc: Option<HlcTimestamp>,
        limit: usize,
    ) -> Result<Vec<Record>> {
        self.check_open()?;
        let mut candidates = Vec::new();

        let active_file_id = self.inner.writer.lock().active_file_id();

        // Scan each segment sequentially
        for file_id in 1..=active_file_id {
            if let Ok(records) = self.inner.reader.scan_segment(file_id) {
                for (_, record) in records {
                    if let Some(since) = since_hlc {
                        if record.header.hlc > since {
                            candidates.push(record);
                        }
                    } else {
                        candidates.push(record);
                    }

                    if candidates.len() >= limit {
                        return Ok(candidates);
                    }
                }
            }
        }

        Ok(candidates)
    }

    /// Performs background compaction to merge older sealed segment files.
    pub fn compact(&self) -> Result<CompactionStats> {
        self.check_open()?;
        let _guard = self.inner.compaction_lock.lock();

        // Flush active writer and roll to a new segment so current writes are not disturbed
        let active_id = {
            let mut writer = self.inner.writer.lock();
            writer.roll_segment()?
        };

        LogCompactor::compact(
            &self.inner.base_dir,
            active_id,
            &self.inner.keydir,
            &self.inner.config,
        )
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
        let meta = match self.db.inner.sparse_index.get_metadata(&self.stream_id) {
            Some(m) => m,
            None => return Ok(Vec::new()),
        };

        if meta.total_entries == 0 {
            return Ok(Vec::new());
        }

        let start_seq = meta.total_entries.saturating_sub(limit as u64);
        let checkpoint = self
            .db
            .inner
            .sparse_index
            .find_checkpoint_before(&self.stream_id, start_seq);

        let (scan_file_id, scan_offset, checkpoint_seq) = match checkpoint {
            Some(cp) => (cp.file_id, cp.offset, cp.sequence),
            None => (1, 0, 0),
        };

        let active_file_id = self.db.inner.writer.lock().active_file_id();
        let mut entries = Vec::new();
        let mut current_seq = checkpoint_seq;

        for fid in scan_file_id..=active_file_id {
            if let Ok(records) = self.db.inner.reader.scan_segment(fid) {
                for (offset, record) in records {
                    if fid == scan_file_id && offset < scan_offset {
                        continue;
                    }

                    if record.header.record_type == RecordType::StreamEntry
                        && record.key == self.stream_id.as_bytes()
                    {
                        if current_seq >= start_seq {
                            entries.push(StreamEntry {
                                sequence: current_seq,
                                hlc: record.header.hlc,
                                payload: record.value,
                            });
                        }
                        current_seq += 1;
                    }
                }
            }
        }

        // Return up to limit entries
        if entries.len() > limit {
            entries = entries.split_off(entries.len() - limit);
        }

        Ok(entries)
    }

    fn total_entries(&self) -> u64 {
        self.db
            .inner
            .sparse_index
            .get_metadata(&self.stream_id)
            .map(|m| m.total_entries)
            .unwrap_or(0)
    }

    fn stream_id(&self) -> &str {
        &self.stream_id
    }
}
