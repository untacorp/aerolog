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

//! Mozilla UniFFI bindings export for AeroLog.
//!
//! Generates native, idiomatic Kotlin and Swift bindings without manual JNI or C bridging.

uniffi::setup_scaffolding!();

use aerolog_core::{AeroLogConfig, AeroLogDb, AeroLogStream, HlcTimestamp, SyncMode};
use std::sync::Arc;
use thiserror::Error;

/// UniFFI-compatible error type mapping internal AeroLog errors.
#[derive(Debug, Error, uniffi::Error)]
pub enum AeroLogFfiError {
    #[error("Database error: {0}")]
    EngineError(String),
    #[error("Key not found: {0}")]
    NotFound(String),
}

impl From<aerolog_core::AeroLogError> for AeroLogFfiError {
    fn from(err: aerolog_core::AeroLogError) -> Self {
        Self::EngineError(err.to_string())
    }
}

/// UniFFI-compatible representation of an entry retrieved from an append-only stream.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiStreamEntry {
    pub sequence: u64,
    pub hlc_string: String,
    pub physical_time: u64,
    pub logical_counter: u16,
    pub node_id_hash: u64,
    pub payload: Vec<u8>,
}

/// UniFFI-compatible key-value pair.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiKeyValuePair {
    pub key: String,
    pub value: Vec<u8>,
}

/// UniFFI-compatible representation of a replicated mutation record for distributed sync.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiMutationRecord {
    pub key: String,
    pub record_type: u8,
    pub physical_time: u64,
    pub logical_counter: u16,
    pub node_id_hash: u64,
    pub hlc_string: String,
    pub payload: Vec<u8>,
}

/// UniFFI-compatible compaction metrics.
#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct FfiCompactionStats {
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub records_evicted: u64,
}

/// Append-only stream partition handle exported via UniFFI.
#[derive(uniffi::Object)]
pub struct FfiStream {
    inner: Arc<dyn AeroLogStream>,
}

#[uniffi::export]
impl FfiStream {
    /// Appends a payload to the stream.
    /// Returns a tuple of (sequence_number, hlc_string).
    pub fn append(&self, payload: Vec<u8>) -> Result<FfiStreamEntry, AeroLogFfiError> {
        let (seq, hlc) = self.inner.append(payload.clone())?;
        Ok(FfiStreamEntry {
            sequence: seq,
            hlc_string: hlc.to_string(),
            physical_time: hlc.physical_time,
            logical_counter: hlc.logical_counter,
            node_id_hash: hlc.node_id_hash,
            payload,
        })
    }

    /// Reads up to `limit` of the most recent entries in chronological order.
    pub fn read_recent(&self, limit: u32) -> Result<Vec<FfiStreamEntry>, AeroLogFfiError> {
        let entries = self.inner.read_recent(limit as usize)?;
        Ok(entries
            .into_iter()
            .map(|e| FfiStreamEntry {
                sequence: e.sequence,
                hlc_string: e.hlc.to_string(),
                physical_time: e.hlc.physical_time,
                logical_counter: e.hlc.logical_counter,
                node_id_hash: e.hlc.node_id_hash,
                payload: e.payload,
            })
            .collect())
    }

    /// Returns the total number of entries currently appended to this stream.
    pub fn total_entries(&self) -> u64 {
        self.inner.total_entries()
    }

    /// Returns the unique identifier of this stream.
    pub fn stream_id(&self) -> String {
        self.inner.stream_id().to_string()
    }
}

/// Primary database instance exported via UniFFI.
#[derive(uniffi::Object)]
pub struct FfiAeroLogDb {
    inner: AeroLogDb,
}

#[uniffi::export]
impl FfiAeroLogDb {
    /// Opens an AeroLog database at the given path with standard configuration.
    #[uniffi::constructor]
    pub fn open(path: String, node_id_hash: u64) -> Result<Arc<Self>, AeroLogFfiError> {
        let config = AeroLogConfig::new(node_id_hash)
            .with_sync_mode(SyncMode::Periodic)
            .with_max_segment_size(16 * 1024 * 1024);

        let db = AeroLogDb::open(path, config)?;
        Ok(Arc::new(Self { inner: db }))
    }

    /// Stores a Key-Value pair, generating a new monotonic HLC timestamp.
    pub fn put(
        &self,
        collection: String,
        key: String,
        value: Vec<u8>,
    ) -> Result<String, AeroLogFfiError> {
        let hlc = self.inner.put(&collection, &key, value)?;
        Ok(hlc.to_string())
    }

    /// Stores a Key-Value pair with an explicit remote HLC timestamp (peer replication).
    /// Returns `true` if accepted under Last-Write-Wins rules.
    pub fn put_with_hlc(
        &self,
        collection: String,
        key: String,
        value: Vec<u8>,
        physical_time: u64,
        logical_counter: u16,
        node_id_hash: u64,
    ) -> Result<bool, AeroLogFfiError> {
        let hlc = HlcTimestamp::new(physical_time, logical_counter, node_id_hash);
        let accepted = self.inner.put_with_hlc(&collection, &key, value, hlc)?;
        Ok(accepted)
    }

    /// Retrieves a value by collection and key.
    pub fn get(&self, collection: String, key: String) -> Result<Option<Vec<u8>>, AeroLogFfiError> {
        let val = self.inner.get(&collection, &key)?;
        Ok(val)
    }

    /// Soft-deletes a key, appending a tombstone record with a new HLC timestamp.
    pub fn delete(&self, collection: String, key: String) -> Result<String, AeroLogFfiError> {
        let hlc = self.inner.delete(&collection, &key)?;
        Ok(hlc.to_string())
    }

    /// Retrieves an append-only stream partition handle.
    pub fn get_stream(&self, stream_id: String) -> Result<Arc<FfiStream>, AeroLogFfiError> {
        let stream = self.inner.get_stream(&stream_id)?;
        Ok(Arc::new(FfiStream { inner: stream }))
    }

    /// Scans and returns all live key-value pairs in a collection.
    pub fn scan_collection(
        &self,
        collection: String,
    ) -> Result<Vec<FfiKeyValuePair>, AeroLogFfiError> {
        let pairs = self.inner.scan_collection(&collection)?;
        Ok(pairs
            .into_iter()
            .map(|(key, value)| FfiKeyValuePair { key, value })
            .collect())
    }

    /// Scans all mutations strictly newer than the given timestamp (for distributed sync).
    pub fn scan_mutations_since(
        &self,
        since_physical_time: Option<u64>,
        since_logical_counter: Option<u16>,
        since_node_hash: Option<u64>,
        limit: u32,
    ) -> Result<Vec<FfiMutationRecord>, AeroLogFfiError> {
        let since_hlc = match (since_physical_time, since_logical_counter, since_node_hash) {
            (Some(pt), Some(lc), Some(nh)) => Some(HlcTimestamp::new(pt, lc, nh)),
            _ => None,
        };

        let records = self.inner.scan_mutations_since(since_hlc, limit as usize)?;

        Ok(records
            .into_iter()
            .map(|r| FfiMutationRecord {
                key: String::from_utf8_lossy(&r.key).to_string(),
                record_type: r.header.record_type as u8,
                physical_time: r.header.hlc.physical_time,
                logical_counter: r.header.hlc.logical_counter,
                node_id_hash: r.header.hlc.node_id_hash,
                hlc_string: r.header.hlc.to_string(),
                payload: r.value,
            })
            .collect())
    }

    /// Performs background compaction to reclaim disk space.
    pub fn compact(&self) -> Result<FfiCompactionStats, AeroLogFfiError> {
        let stats = self.inner.compact()?;
        Ok(FfiCompactionStats {
            bytes_before: stats.bytes_before,
            bytes_after: stats.bytes_after,
            records_evicted: stats.records_evicted,
        })
    }

    /// Flushes all pending writes to disk.
    pub fn flush(&self) -> Result<(), AeroLogFfiError> {
        self.inner.flush()?;
        Ok(())
    }

    /// Explicitly shuts down the database, flushing and syncing all files.
    pub fn shutdown(&self) -> Result<(), AeroLogFfiError> {
        self.inner.close()?;
        Ok(())
    }
}

impl Drop for FfiAeroLogDb {
    fn drop(&mut self) {
        let _ = self.inner.close();
    }
}
