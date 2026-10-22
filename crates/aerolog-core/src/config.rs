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

use std::collections::HashMap;

/// Filesystem synchronization discipline for write operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// Issue an fsync / fdatasync after every write operation.
    ///
    /// Provides maximum ACID durability against sudden power cuts,
    /// at the cost of lower throughput on mechanical/NAND storage.
    SyncOnWrite,

    /// Rely on periodic flushing and segment rolling to sync data.
    ///
    /// Strikes a balance between high write throughput and crash recovery.
    Periodic,

    /// Defer synchronization entirely to operating system page cache flush semantics.
    OsDefault,
}

/// Declarative retention policies for append-only streams and time-series partitions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RetentionPolicy {
    /// Retain all stream entries indefinitely (default).
    #[default]
    KeepAll,

    /// Retain at most the latest `max` entries in this stream partition.
    MaxEntries(u64),

    /// Retain entries authored within the last `ttl_millis` duration.
    TimeToLive(u64),
}

/// Configuration options for an [`AeroLogDb`](crate::AeroLogDb) instance.
#[derive(Debug, Clone)]
pub struct AeroLogConfig {
    /// Maximum size of an active segment file in bytes before rolling to a new file.
    /// Default: 16 MB (16 * 1024 * 1024 bytes).
    pub max_segment_size: u64,

    /// File synchronization mode for write operations.
    /// Default: [`SyncMode::Periodic`].
    pub sync_mode: SyncMode,

    /// Number of stream entries between sparse index checkpoints.
    /// Default: 100 entries per sparse checkpoint pointer.
    pub sparse_checkpoint_interval: u32,

    /// Optional time-to-live (in milliseconds) for soft-deleted tombstones during compaction.
    /// Default: None (retain tombstones until explicitly purged or watermark resolved).
    pub tombstone_ttl_millis: Option<u64>,

    /// Node identifier hash representing this local node (64-bit integer).
    /// Default: Derived deterministically or randomized.
    pub node_id_hash: u64,

    /// Default retention policy applied to streams unless an override is specified.
    pub default_stream_retention: RetentionPolicy,

    /// Stream-specific retention policy overrides.
    pub stream_retention_overrides: HashMap<String, RetentionPolicy>,

    /// Optional periodic background compaction interval.
    /// Default: None (compaction is manual via `db.compact()` unless explicitly scheduled).
    pub compaction_interval: Option<std::time::Duration>,
}

impl Default for AeroLogConfig {
    fn default() -> Self {
        Self {
            max_segment_size: 16 * 1024 * 1024, // 16 MB
            sync_mode: SyncMode::Periodic,
            sparse_checkpoint_interval: 100,
            tombstone_ttl_millis: None,
            node_id_hash: 0,
            default_stream_retention: RetentionPolicy::KeepAll,
            stream_retention_overrides: HashMap::new(),
            compaction_interval: None,
        }
    }
}

impl AeroLogConfig {
    /// Creates a new default configuration with a specific node identifier hash.
    pub fn new(node_id_hash: u64) -> Self {
        Self {
            node_id_hash,
            ..Default::default()
        }
    }

    /// Sets the maximum segment size in bytes.
    pub fn with_max_segment_size(mut self, size: u64) -> Self {
        self.max_segment_size = size;
        self
    }

    /// Sets the file synchronization mode.
    pub fn with_sync_mode(mut self, mode: SyncMode) -> Self {
        self.sync_mode = mode;
        self
    }

    /// Sets the sparse checkpoint interval for stream indexing.
    pub fn with_sparse_checkpoint_interval(mut self, interval: u32) -> Self {
        self.sparse_checkpoint_interval = interval.max(1);
        self
    }

    /// Sets the tombstone time-to-live in milliseconds.
    pub fn with_tombstone_ttl_millis(mut self, ttl: Option<u64>) -> Self {
        self.tombstone_ttl_millis = ttl;
        self
    }

    /// Sets the default stream retention policy.
    pub fn with_default_stream_retention(mut self, policy: RetentionPolicy) -> Self {
        self.default_stream_retention = policy;
        self
    }

    /// Configures a specific retention policy for a single stream partition.
    pub fn with_stream_retention_override(
        mut self,
        stream_id: impl Into<String>,
        policy: RetentionPolicy,
    ) -> Self {
        self.stream_retention_overrides
            .insert(stream_id.into(), policy);
        self
    }

    /// Sets an optional periodic background compaction interval.
    /// Pass `None` to disable automatic background compaction.
    pub fn with_compaction_interval(mut self, interval: Option<std::time::Duration>) -> Self {
        self.compaction_interval = interval;
        self
    }

    /// Resolves the retention policy for a specific stream identifier.
    pub fn retention_for_stream(&self, stream_id: &str) -> &RetentionPolicy {
        self.stream_retention_overrides
            .get(stream_id)
            .unwrap_or(&self.default_stream_retention)
    }
}
