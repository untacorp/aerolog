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

use parking_lot::Mutex;
use std::cmp::Ordering;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// Monotonically increasing Hybrid Logical Clock (HLC) timestamp.
///
/// Combines physical wall-clock time, an incremental counter for events
/// occurring within the same millisecond, and a deterministic node hash tie-breaker.
///
/// Guaranteed to preserve total causal ordering across distributed edge nodes
/// without requiring network time synchronization (NTP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HlcTimestamp {
    /// Wall-clock epoch time in milliseconds.
    pub physical_time: u64,
    /// Logical counter incremented for sub-millisecond collisions.
    pub logical_counter: u16,
    /// Deterministic 64-bit tie-breaker hash representing the originating node.
    pub node_id_hash: u64,
}

impl HlcTimestamp {
    /// Zero timestamp representation.
    pub const ZERO: Self = Self {
        physical_time: 0,
        logical_counter: 0,
        node_id_hash: 0,
    };

    /// Creates a new HLC timestamp.
    pub const fn new(physical_time: u64, logical_counter: u16, node_id_hash: u64) -> Self {
        Self {
            physical_time,
            logical_counter,
            node_id_hash,
        }
    }
}

impl Ord for HlcTimestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        self.physical_time
            .cmp(&other.physical_time)
            .then_with(|| self.logical_counter.cmp(&other.logical_counter))
            .then_with(|| self.node_id_hash.cmp(&other.node_id_hash))
    }
}

impl PartialOrd for HlcTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for HlcTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}-{}-{:016x}",
            self.physical_time, self.logical_counter, self.node_id_hash
        )
    }
}

/// Thread-safe generator for monotonically increasing [`HlcTimestamp`] values.
#[derive(Debug)]
pub struct HlcGenerator {
    node_id_hash: u64,
    state: Mutex<HlcTimestamp>,
}

impl HlcGenerator {
    /// Creates a new generator instance for the given node hash.
    pub fn new(node_id_hash: u64) -> Self {
        Self {
            node_id_hash,
            state: Mutex::new(HlcTimestamp::new(0, 0, node_id_hash)),
        }
    }

    /// Obtains current physical wall-clock time in epoch milliseconds.
    fn current_physical_time() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Advances the logical clock locally and returns a monotonically increasing timestamp.
    pub fn now(&self) -> HlcTimestamp {
        let mut state = self.state.lock();
        let wall = Self::current_physical_time();

        if wall > state.physical_time {
            state.physical_time = wall;
            state.logical_counter = 0;
        } else {
            // Clock skew backwards or sub-millisecond collision: ratchet counter forward
            state.logical_counter = state.logical_counter.saturating_add(1);
        }

        HlcTimestamp::new(
            state.physical_time,
            state.logical_counter,
            self.node_id_hash,
        )
    }

    /// Updates the local clock with an observed remote timestamp.
    ///
    /// Ratchets the local clock forward to ensure strict causality with peer events.
    pub fn update(&self, remote: HlcTimestamp) -> HlcTimestamp {
        let mut state = self.state.lock();
        let wall = Self::current_physical_time();

        let max_pt = wall.max(state.physical_time).max(remote.physical_time);

        if max_pt == state.physical_time && max_pt == remote.physical_time {
            state.logical_counter = state
                .logical_counter
                .max(remote.logical_counter)
                .saturating_add(1);
        } else if max_pt == state.physical_time {
            state.logical_counter = state.logical_counter.saturating_add(1);
        } else if max_pt == remote.physical_time {
            state.logical_counter = remote.logical_counter.saturating_add(1);
        } else {
            state.logical_counter = 0;
        }

        state.physical_time = max_pt;

        HlcTimestamp::new(
            state.physical_time,
            state.logical_counter,
            self.node_id_hash,
        )
    }

    /// Returns the latest timestamp produced by this generator without advancing it.
    pub fn current(&self) -> HlcTimestamp {
        let state = self.state.lock();
        *state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hlc_total_ordering() {
        let t1 = HlcTimestamp::new(1000, 0, 1);
        let t2 = HlcTimestamp::new(1000, 1, 1);
        let t3 = HlcTimestamp::new(1001, 0, 1);
        let t4 = HlcTimestamp::new(1001, 0, 2);

        assert!(t1 < t2);
        assert!(t2 < t3);
        assert!(t3 < t4);
    }

    #[test]
    fn test_monotonic_generation() {
        let gen = HlcGenerator::new(0xABCD);
        let a = gen.now();
        let b = gen.now();
        assert!(b >= a);
    }

    #[test]
    fn test_remote_update_ratchets_forward() {
        let gen = HlcGenerator::new(0x01);
        let remote = HlcTimestamp::new(9_999_999_000, 42, 0x02);
        let updated = gen.update(remote);

        assert!(updated.physical_time >= remote.physical_time);
        assert!(updated > remote);
    }
}
