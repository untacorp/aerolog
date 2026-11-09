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

#[test]
fn test_torn_write_recovery_and_append_continuity() {
    use std::io::Write;

    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0xDDD)
        .with_sync_mode(SyncMode::SyncOnWrite)
        .with_max_segment_size(1024 * 1024);

    {
        let db = AeroLogDb::open(dir.path(), config.clone()).unwrap();
        db.put("users", "alice", b"alice_val".to_vec()).unwrap();
        db.close().unwrap();
    }

    // Simulate crash tearing: append incomplete header and corrupt trailing bytes
    let seg_path = dir.path().join("data-000001.aerolog");
    {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&seg_path)
            .unwrap();
        // 0x41 0x4C magic bytes with garbage trailing payload
        file.write_all(&[0x41, 0x4C, 0x01, 0x00, 0xFF, 0xFF, 0xEE, 0xDD])
            .unwrap();
        file.sync_all().unwrap();
    }

    // Reopen 1: scanner isolates trailing corruption, truncates active segment
    {
        let db = AeroLogDb::open(dir.path(), config.clone()).unwrap();
        assert_eq!(db.get("users", "alice").unwrap().unwrap(), b"alice_val");
        // Append new record after recovery
        db.put("users", "bob", b"bob_val".to_vec()).unwrap();
        db.close().unwrap();
    }

    // Reopen 2: verify both records survive without corruption from previous crash
    {
        let db = AeroLogDb::open(dir.path(), config).unwrap();
        assert_eq!(db.get("users", "alice").unwrap().unwrap(), b"alice_val");
        assert_eq!(db.get("users", "bob").unwrap().unwrap(), b"bob_val");
        db.close().unwrap();
    }
}

#[test]
fn test_stream_sequence_monotonicity_across_reopen_after_compaction() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0xABC)
        .with_sync_mode(SyncMode::SyncOnWrite)
        .with_max_segment_size(250);

    // Step 1: Append 50 stream entries across segments
    {
        let db = AeroLogDb::open(dir.path(), config.clone()).unwrap();
        let stream = db
            .get_stream_with_retention("audit", RetentionPolicy::MaxEntries(10))
            .unwrap();

        for i in 0..50 {
            let (seq, _) = stream.append(format!("event_{i}").into_bytes()).unwrap();
            assert_eq!(seq, i as u64);
        }
        assert_eq!(stream.total_entries(), 50);

        // Step 2: Compact with MaxEntries(10)
        let stats = db.compact().unwrap();
        assert_eq!(stats.records_evicted, 40);

        // Step 3: Close database
        db.close().unwrap();
    }

    // Step 4: Reopen database and verify sequence numbers and total entries
    {
        let db = AeroLogDb::open(dir.path(), config).unwrap();
        let stream = db
            .get_stream_with_retention("audit", RetentionPolicy::MaxEntries(10))
            .unwrap();

        assert_eq!(stream.total_entries(), 50);

        let recent = stream.read_recent(10).unwrap();
        assert_eq!(recent.len(), 10);
        for (idx, entry) in recent.iter().enumerate() {
            let expected_seq = 40 + idx as u64;
            assert_eq!(entry.sequence, expected_seq);
            assert_eq!(entry.payload, format!("event_{expected_seq}").into_bytes());
        }

        let recent_5 = stream.read_recent(5).unwrap();
        assert_eq!(recent_5.len(), 5);
        for (idx, entry) in recent_5.iter().enumerate() {
            let expected_seq = 45 + idx as u64;
            assert_eq!(entry.sequence, expected_seq);
        }

        // Appending new entry continues strictly monotonically at 50
        let (seq50, _) = stream.append(b"event_50".to_vec()).unwrap();
        assert_eq!(seq50, 50);
        assert_eq!(stream.total_entries(), 51);

        db.close().unwrap();
    }
}

#[test]
fn test_compactor_lifecycle_independent_of_db_clone_drops() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0xCAFE)
        .with_sync_mode(SyncMode::SyncOnWrite)
        .with_max_segment_size(200);

    let db = AeroLogDb::open(dir.path(), config).unwrap();
    db.start_background_compactor(std::time::Duration::from_millis(50))
        .unwrap();

    // Clone db handle and immediately drop the clone
    let db_clone = db.clone();
    drop(db_clone);

    // Append records across segment rolls to produce multiple sealed segments
    for i in 0..40 {
        db.put("metrics", &format!("k{i}"), b"val".to_vec())
            .unwrap();
    }

    // Allow background compactor to trigger and merge inactive segments
    std::thread::sleep(std::time::Duration::from_millis(250));

    // The primary handle is still open, compactor is still running
    db.close().unwrap();
}

#[test]
fn test_stale_lww_write_rejection_avoids_disk_append() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x777).with_sync_mode(SyncMode::SyncOnWrite);
    let db = AeroLogDb::open(dir.path(), config).unwrap();

    // Put key with HLC physical time 1000
    let hlc1000 = HlcTimestamp::new(1000, 0, 0x111);
    let accepted = db
        .put_with_hlc("kv", "sensor", b"temp_25".to_vec(), hlc1000)
        .unwrap();
    assert!(accepted);

    let active_seg_path = dir.path().join("data-000001.aerolog");
    let size_before = std::fs::metadata(&active_seg_path).unwrap().len();

    // Attempt stale write with HLC physical time 800
    let hlc800 = HlcTimestamp::new(800, 0, 0x111);
    let accepted_stale = db
        .put_with_hlc("kv", "sensor", b"temp_20".to_vec(), hlc800)
        .unwrap();
    assert!(!accepted_stale);

    // Verify active segment file size did not increase (disk write was avoided)
    let size_after = std::fs::metadata(&active_seg_path).unwrap().len();
    assert_eq!(size_before, size_after);

    // Verify value was not updated
    assert_eq!(db.get("kv", "sensor").unwrap().unwrap(), b"temp_25");

    db.close().unwrap();
}

#[test]
fn test_stale_lww_delete_rejection_avoids_disk_append() {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x778).with_sync_mode(SyncMode::SyncOnWrite);
    let db = AeroLogDb::open(dir.path(), config).unwrap();

    // Put key with HLC physical time 1000
    let hlc1000 = HlcTimestamp::new(1000, 0, 0x111);
    db.put_with_hlc("kv", "sensor", b"temp_25".to_vec(), hlc1000)
        .unwrap();

    let active_seg_path = dir.path().join("data-000001.aerolog");
    let size_before = std::fs::metadata(&active_seg_path).unwrap().len();

    // Attempt stale delete with HLC physical time 800
    let hlc800 = HlcTimestamp::new(800, 0, 0x111);
    let accepted_stale = db.delete_with_hlc("kv", "sensor", hlc800).unwrap();
    assert!(!accepted_stale);

    // Verify active segment file size did not increase (disk write was avoided)
    let size_after = std::fs::metadata(&active_seg_path).unwrap().len();
    assert_eq!(size_before, size_after);

    // Verify value still exists
    assert_eq!(db.get("kv", "sensor").unwrap().unwrap(), b"temp_25");

    // Newer delete with HLC physical time 1200 is accepted and appends tombstone
    let hlc1200 = HlcTimestamp::new(1200, 0, 0x111);
    let accepted_newer = db.delete_with_hlc("kv", "sensor", hlc1200).unwrap();
    assert!(accepted_newer);

    let size_final = std::fs::metadata(&active_seg_path).unwrap().len();
    assert!(size_final > size_after);
    assert_eq!(db.get("kv", "sensor").unwrap(), None);

    db.close().unwrap();
}

#[test]
fn test_segment_parser_bounds_validation() {
    use aerolog_core::log::segment::SegmentFile;
    use std::path::Path;

    assert_eq!(SegmentFile::parse_file_id(Path::new("data-aerolog")), None);
    assert_eq!(SegmentFile::parse_file_id(Path::new("data-.aerolog")), None);
    assert_eq!(SegmentFile::parse_file_id(Path::new(".aerolog")), None);
    assert_eq!(
        SegmentFile::parse_file_id(Path::new("data-000001.aerolog")),
        Some(1)
    );
    assert_eq!(
        SegmentFile::parse_file_id(Path::new("/var/log/data-000042.aerolog")),
        Some(42)
    );
    assert_eq!(SegmentFile::parse_file_id(Path::new("random-file.txt")), None);
}
