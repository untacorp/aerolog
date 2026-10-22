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

use aerolog_core::{AeroLogConfig, AeroLogDb, HlcTimestamp, RetentionPolicy, SyncMode};
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

        let hlc1 = db.put("users", "alice", b"{\"age\": 30}".to_vec()).unwrap();
        assert!(hlc1.physical_time > 0);

        db.put("users", "bob", b"{\"age\": 25}".to_vec()).unwrap();

        let alice_val = db.get("users", "alice").unwrap().unwrap();
        assert_eq!(alice_val, b"{\"age\": 30}");

        db.delete("users", "bob").unwrap();
        assert_eq!(db.get("users", "bob").unwrap(), None);

        db.close().unwrap();
    }

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

    for i in 0..150 {
        let msg = format!("Message #{i}").into_bytes();
        let (seq, _) = stream.append(msg).unwrap();
        assert_eq!(seq, i);
    }

    assert_eq!(stream.total_entries(), 150);

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
    let config = AeroLogConfig::new(0x303)
        .with_max_segment_size(200)
        .with_sync_mode(SyncMode::SyncOnWrite);

    let db = AeroLogDb::open(dir.path(), config).unwrap();

    for i in 0..100 {
        let val = format!("version_{i:04}").into_bytes();
        db.put("config", "theme", val).unwrap();
    }

    let stats = db.compact().unwrap();
    assert!(stats.records_evicted > 50);

    let final_val = db.get("config", "theme").unwrap().unwrap();
    assert_eq!(final_val, b"version_0099");
}

#[test]
fn test_multithreaded_concurrency() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x404);
    let db = Arc::new(AeroLogDb::open(dir.path(), config).unwrap());

    let mut handles = Vec::new();

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

    db.put_with_hlc("status", "node-1", b"ONLINE".to_vec(), t_newer)
        .unwrap();

    let accepted = db
        .put_with_hlc("status", "node-1", b"OFFLINE".to_vec(), t_older)
        .unwrap();

    assert!(!accepted);

    assert_eq!(db.get("status", "node-1").unwrap().unwrap(), b"ONLINE");
}

#[test]
fn test_stream_retention_policy_max_entries() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x505)
        .with_max_segment_size(200)
        .with_sync_mode(SyncMode::SyncOnWrite)
        .with_default_stream_retention(RetentionPolicy::MaxEntries(10));

    let db = AeroLogDb::open(dir.path(), config).unwrap();
    let stream = db.get_stream("alerts").unwrap();

    for i in 0..50 {
        let payload = format!("Alert #{i:04}").into_bytes();
        stream.append(payload).unwrap();
    }

    let stats = db.compact().unwrap();
    assert_eq!(stats.records_evicted, 40);

    let recent = stream.read_recent(10).unwrap();
    assert_eq!(recent.len(), 10);
    assert_eq!(recent[0].payload, b"Alert #0040");
    assert_eq!(recent[9].payload, b"Alert #0049");
}

#[test]
fn test_tombstone_ttl_compaction_preservation() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x606)
        .with_max_segment_size(200)
        .with_sync_mode(SyncMode::SyncOnWrite)
        .with_tombstone_ttl_millis(Some(3_600_000));

    let db = AeroLogDb::open(dir.path(), config).unwrap();

    db.put("users", "alice", b"initial_data".to_vec()).unwrap();
    db.delete("users", "alice").unwrap();

    let stats = db.compact().unwrap();
    assert_eq!(stats.records_evicted, 1);

    assert!(db.get("users", "alice").unwrap().is_none());
}

#[test]
fn test_composite_key_with_colons_persistence() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x707);
    let db = AeroLogDb::open(dir.path(), config.clone()).unwrap();

    db.put("mesh:devices:v1", "sensor:temp:01", b"24.5".to_vec())
        .unwrap();

    assert_eq!(
        db.get("mesh:devices:v1", "sensor:temp:01")
            .unwrap()
            .unwrap(),
        b"24.5"
    );

    drop(db);
    let db2 = AeroLogDb::open(dir.path(), config).unwrap();
    assert_eq!(
        db2.get("mesh:devices:v1", "sensor:temp:01")
            .unwrap()
            .unwrap(),
        b"24.5"
    );
}

#[test]
fn test_tombstone_purging_removes_dangling_keydir_pointers() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x808)
        .with_max_segment_size(200)
        .with_sync_mode(SyncMode::SyncOnWrite)
        .with_tombstone_ttl_millis(Some(1));

    let db = AeroLogDb::open(dir.path(), config).unwrap();

    db.put("users", "alice", b"initial_data".to_vec()).unwrap();
    db.delete("users", "alice").unwrap();

    std::thread::sleep(std::time::Duration::from_millis(15));

    let stats = db.compact().unwrap();
    assert_eq!(stats.records_evicted, 2); // Put + expired Delete both evicted

    assert!(db.get("users", "alice").unwrap().is_none());

    let record_opt = db.get_record("users", "alice").unwrap();
    assert!(record_opt.is_none());
}

#[test]
fn test_stream_sequence_monotonicity_preserved_across_compaction() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x909)
        .with_max_segment_size(200)
        .with_sync_mode(SyncMode::SyncOnWrite)
        .with_default_stream_retention(RetentionPolicy::MaxEntries(10));

    let db = AeroLogDb::open(dir.path(), config).unwrap();
    let stream = db.get_stream("alerts").unwrap();

    for i in 0..50 {
        let payload = format!("Alert #{i:04}").into_bytes();
        let (seq, _) = stream.append(payload).unwrap();
        assert_eq!(seq, i as u64);
    }
    assert_eq!(stream.total_entries(), 50);

    let stats = db.compact().unwrap();
    assert_eq!(stats.records_evicted, 40);

    let recent = stream.read_recent(10).unwrap();
    assert_eq!(recent.len(), 10);
    assert_eq!(recent[0].sequence, 40);
    assert_eq!(recent[0].payload, b"Alert #0040");
    assert_eq!(recent[9].sequence, 49);
    assert_eq!(recent[9].payload, b"Alert #0049");

    assert_eq!(stream.total_entries(), 50);

    let (next_seq, _) = stream.append(b"Alert #0050".to_vec()).unwrap();
    assert_eq!(next_seq, 50);
    assert_eq!(stream.total_entries(), 51);
}

#[test]
fn test_dynamic_get_stream_with_retention() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0xAAA)
        .with_max_segment_size(200)
        .with_sync_mode(SyncMode::SyncOnWrite);

    let db = AeroLogDb::open(dir.path(), config).unwrap();

    let stream = db
        .get_stream_with_retention("sensors", RetentionPolicy::MaxEntries(5))
        .unwrap();

    for i in 0..20 {
        stream.append(format!("Reading #{i}").into_bytes()).unwrap();
    }

    let stats = db.compact().unwrap();
    assert_eq!(stats.records_evicted, 15);

    let recent = stream.read_recent(5).unwrap();
    assert_eq!(recent.len(), 5);
    assert_eq!(recent[0].sequence, 15);
    assert_eq!(recent[0].payload, b"Reading #15");
    assert_eq!(recent[4].sequence, 19);
    assert_eq!(recent[4].payload, b"Reading #19");
}

#[test]
fn test_optional_background_compactor() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0xBBB)
        .with_max_segment_size(200)
        .with_sync_mode(SyncMode::SyncOnWrite)
        .with_tombstone_ttl_millis(Some(1))
        .with_compaction_interval(Some(std::time::Duration::from_millis(100)));

    let db = AeroLogDb::open(dir.path(), config).unwrap();

    for i in 0..5 {
        db.put("temp", &format!("k{i}"), b"val".to_vec()).unwrap();
        db.delete("temp", &format!("k{i}")).unwrap();
    }

    std::thread::sleep(std::time::Duration::from_millis(250));

    for i in 0..5 {
        assert!(db.get("temp", &format!("k{i}")).unwrap().is_none());
        assert!(db.get_record("temp", &format!("k{i}")).unwrap().is_none());
    }

    db.close().unwrap();
}

#[test]
fn test_concurrent_compaction_and_continuous_reads_writes() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0xCCC)
        .with_max_segment_size(1024)
        .with_sync_mode(SyncMode::Periodic);

    let db = Arc::new(AeroLogDb::open(dir.path(), config).unwrap());
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));

    let mut handles = Vec::new();

    for tid in 0..3 {
        let db_clone = Arc::clone(&db);
        let run_clone = Arc::clone(&running);
        handles.push(std::thread::spawn(move || {
            let mut i = 0u64;
            while run_clone.load(std::sync::atomic::Ordering::Relaxed) {
                let key = format!("worker_{tid}_{}", i % 50);
                let val = format!("val_{tid}_{i}").into_bytes();
                db_clone.put("stress", &key, val).unwrap();
                i += 1;
                std::thread::yield_now();
            }
        }));
    }

    for tid in 0..3 {
        let db_clone = Arc::clone(&db);
        let run_clone = Arc::clone(&running);
        handles.push(std::thread::spawn(move || {
            let mut i = 0u64;
            while run_clone.load(std::sync::atomic::Ordering::Relaxed) {
                let key = format!("worker_{tid}_{}", i % 50);
                let _ = db_clone.get("stress", &key);
                i += 1;
                std::thread::yield_now();
            }
        }));
    }

    {
        let db_clone = Arc::clone(&db);
        let run_clone = Arc::clone(&running);
        handles.push(std::thread::spawn(move || {
            let stream = db_clone.get_stream("stress_events").unwrap();
            let mut i = 0u64;
            while run_clone.load(std::sync::atomic::Ordering::Relaxed) {
                let msg = format!("msg_{i}").into_bytes();
                stream.append(msg).unwrap();
                if i.is_multiple_of(10) {
                    let _ = stream.read_recent(5);
                }
                i += 1;
                std::thread::yield_now();
            }
        }));
    }

    for _ in 0..5 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let _ = db.compact();
    }

    running.store(false, std::sync::atomic::Ordering::SeqCst);
    for h in handles {
        h.join().unwrap();
    }

    let final_compaction = db.compact();
    assert!(final_compaction.is_ok());

    db.close().unwrap();
}
