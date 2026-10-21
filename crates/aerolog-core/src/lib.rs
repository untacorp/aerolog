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

//! # AeroLog Core
//!
//! **AeroLog** is an ultra-fast, local-first, multi-model embedded storage engine
//! written in pure Rust.
//!
//! Designed specifically for edge computing, mobile devices, and off-grid wireless
//! database synchronization (e.g., Bluetooth Low Energy mesh networks).
//!
//! ## Key Capabilities
//! - **Append-Only Bitcask Architecture**: All mutations are $\mathcal{O}(1)$ sequential appends to `.aerolog` segment files.
//! - **Dual-Tier Indexing**: In-memory `KeyDir` for instant Key-Value lookups and memory-bounded `SparseStreamIndex` for chat/events.
//! - **Deterministic Causality**: Built-in monotonic Hybrid Logical Clocks (`HlcTimestamp`) for zero-NTP Last-Write-Wins convergence.
//! - **Crash-Resilience**: Per-record hardware CRC32 validation isolates corruption and prevents data loss across process kills.
//! - **Zero Platform Leaks**: Pure standard library and systems-level I/O.

pub mod compactor;
pub mod config;
pub mod db;
pub mod error;
pub mod hlc;
pub mod index;
pub mod log;
pub mod record;
pub mod recovery;
pub mod stream;

pub use config::{AeroLogConfig, RetentionPolicy, SyncMode};
pub use db::AeroLogDb;
pub use error::{AeroLogError, Result};
pub use hlc::{HlcGenerator, HlcTimestamp};
pub use index::pointer::RecordPointer;
pub use record::{Record, RecordHeader, RecordType};
pub use stream::{AeroLogStream, StreamEntry};
