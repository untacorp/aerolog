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

use aerolog_core::{AeroLogConfig, AeroLogDb, HlcTimestamp, SyncMode};
use std::sync::Arc;
use std::thread;
use tempfile::tempdir;

#[test]
fn test_basic_crud_and_persistence_across_reopen() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x101)
        .with_sync_mode(SyncMode::SyncOnWrite)
        .with_max_segment_size(1024 * 1024);

    {
        let db = AeroLogDb::open(dir.path(), config.clone()).unwrap();

        // 1. Put
        let hlc1 = db.put("users", "alice", b"{\"age\": 30}".to_vec()).unwrap();
        assert!(hlc1.physical_time > 0);

        db.put("users", "bob", b"{\"age\": 25}".to_vec()).unwrap();

        // 2. Get
        let alice_val = db.get("users", "alice").unwrap().unwrap();
        assert_eq!(alice_val, b"{\"age\": 30}");

        // 3. Delete
        db.delete("users", "bob").unwrap();
        assert_eq!(db.get("users", "bob").unwrap(), None);

        db.close().unwrap();
    }

    // 4. Reopen and verify persistence
    {
        let db = AeroLogDb::open(dir.path(), config).unwrap();
        let alice_val = db.get("users", "alice").unwrap().unwrap();
        assert_eq!(alice_val, b"{\"age\": 30}");
        assert_eq!(db.get("users", "bob").unwrap(), None);
    }
}

#[test]
fn test_stream_append_and_recent_read() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x202).with_sparse_checkpoint_interval(10);
    let db = AeroLogDb::open(dir.path(), config).unwrap();

    let stream = db.get_stream("chat-room-general").unwrap();

    // Append 150 messages
    for i in 0..150 {
        let msg = format!("Message #{i}").into_bytes();
        let (seq, _) = stream.append(msg).unwrap();
        assert_eq!(seq, i);
    }

    assert_eq!(stream.total_entries(), 150);

    // Read the 10 most recent messages
    let recent = stream.read_recent(10).unwrap();
    assert_eq!(recent.len(), 10);
    assert_eq!(recent[0].sequence, 140);
    assert_eq!(recent[0].payload, b"Message #140");
    assert_eq!(recent[9].sequence, 149);
    assert_eq!(recent[9].payload, b"Message #149");
}

#[test]
fn test_compaction_reclaims_disk_space() {
    let dir = tempdir().unwrap();
    // Use small segment size to force multiple segments
    let config = AeroLogConfig::new(0x303)
        .with_max_segment_size(200)
        .with_sync_mode(SyncMode::SyncOnWrite);

    let db = AeroLogDb::open(dir.path(), config).unwrap();

    // Overwrite the same key 100 times to create dead records
    for i in 0..100 {
        let val = format!("version_{i:04}").into_bytes();
        db.put("config", "theme", val).unwrap();
    }

    // Run compaction
    let stats = db.compact().unwrap();
    assert!(stats.records_evicted > 50);

    // Latest value must still be intact
    let final_val = db.get("config", "theme").unwrap().unwrap();
    assert_eq!(final_val, b"version_0099");
}

#[test]
fn test_multithreaded_concurrency() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x404);
    let db = Arc::new(AeroLogDb::open(dir.path(), config).unwrap());

    let mut handles = Vec::new();

    // Spawn 10 writer threads
    for t in 0..10 {
        let db_clone = Arc::clone(&db);
        let handle = thread::spawn(move || {
            for i in 0..50 {
                let key = format!("worker_{t}_{i}");
                let val = format!("val_{t}_{i}").into_bytes();
                db_clone.put("metrics", &key, val).unwrap();
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }

    // Verify all keys are present
    for t in 0..10 {
        for i in 0..50 {
            let key = format!("worker_{t}_{i}");
            let expected = format!("val_{t}_{i}").into_bytes();
            let actual = db.get("metrics", &key).unwrap().unwrap();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn test_lww_out_of_order_remote_sync() {
    let dir = tempdir().unwrap();
    let db = AeroLogDb::open(dir.path(), AeroLogConfig::default()).unwrap();

    let t_older = HlcTimestamp::new(1000, 0, 1);
    let t_newer = HlcTimestamp::new(2000, 0, 1);

    // Put newer first (e.g. received first over BLE)
    db.put_with_hlc("status", "node-1", b"ONLINE".to_vec(), t_newer)
        .unwrap();

    // Attempt to put older record for the same key
    let accepted = db
        .put_with_hlc("status", "node-1", b"OFFLINE".to_vec(), t_older)
        .unwrap();

    // Must be rejected as stale
    assert!(!accepted);

    // Value must remain ONLINE
    assert_eq!(db.get("status", "node-1").unwrap().unwrap(), b"ONLINE");
}
