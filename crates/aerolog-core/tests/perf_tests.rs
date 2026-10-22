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

use aerolog_core::{AeroLogConfig, AeroLogDb, SyncMode};
use std::time::Instant;
use tempfile::tempdir;

#[test]
fn test_native_rust_performance_benchmarks() {
    let dir = tempdir().unwrap();
    let count = 50_000;
    let payload = b"{\"device_id\":\"sensor_temp_01\",\"metric\":\"temperature\",\"val\":24.85,\"unit\":\"C\"}".to_vec();

    println!("\n==================================================================");
    println!("AeroLog Pure Rust Native Storage Engine - Benchmark Report");
    println!("==================================================================");
    println!(
        "Dataset: {} records | Payload size: {} bytes",
        count,
        payload.len()
    );

    let config = AeroLogConfig::new(0xA1B2C3D4E5F6)
        .with_sync_mode(SyncMode::Periodic)
        .with_max_segment_size(32 * 1024 * 1024);

    let db = AeroLogDb::open(dir.path(), config).unwrap();

    let write_start = Instant::now();
    for i in 0..count {
        let key = format!("node_{:06}", i);
        db.put("telemetry", &key, payload.clone()).unwrap();
    }
    let write_elapsed = write_start.elapsed();
    let write_ops = (count as f64) / write_elapsed.as_secs_f64();
    let write_latency_us = write_elapsed.as_micros() as f64 / count as f64;

    println!("1. Sequential Writes (SyncMode::Periodic):");
    println!("   Total Time   : {:.2?}", write_elapsed);
    println!("   Throughput   : {:.0} writes/sec", write_ops);
    println!("   Avg Latency  : {:.2} us/op", write_latency_us);

    let sample_reads = 50_000;
    let read_start = Instant::now();
    for i in 0..sample_reads {
        let idx = (i * 37) % count;
        let key = format!("node_{:06}", idx);
        let val = db.get("telemetry", &key).unwrap();
        assert!(val.is_some());
    }
    let read_elapsed = read_start.elapsed();
    let read_ops = (sample_reads as f64) / read_elapsed.as_secs_f64();
    let read_latency_us = read_elapsed.as_micros() as f64 / sample_reads as f64;

    println!("\n2. Random Point Reads (db.get):");
    println!("   Total Time   : {:.2?}", read_elapsed);
    println!("   Throughput   : {:.0} reads/sec", read_ops);
    println!("   Avg Latency  : {:.2} us/op", read_latency_us);

    let stream_count = 20_000;
    let stream = db.get_stream("mesh_chat_events").unwrap();
    let stream_payload = b"MESH_PACKET_ROUTING_HOP_ACK_PAYLOAD_VERIFIED".to_vec();

    let stream_start = Instant::now();
    for _ in 0..stream_count {
        stream.append(stream_payload.clone()).unwrap();
    }
    let stream_elapsed = stream_start.elapsed();
    let stream_ops = (stream_count as f64) / stream_elapsed.as_secs_f64();
    let stream_latency_us = stream_elapsed.as_micros() as f64 / stream_count as f64;

    println!("\n3. Append-Only Stream Ingestion:");
    println!("   Entries      : {}", stream_count);
    println!("   Throughput   : {:.0} msgs/sec", stream_ops);
    println!("   Avg Latency  : {:.2} us/append", stream_latency_us);

    let window_start = Instant::now();
    let recent_500 = stream.read_recent(500).unwrap();
    let window_elapsed = window_start.elapsed();
    assert_eq!(recent_500.len(), 500);

    println!(
        "   Read Recent 500 Window: {:.2?} ({} entries retrieved)",
        window_elapsed,
        recent_500.len()
    );

    db.close().unwrap();
    drop(stream);
    drop(db);

    let reopen_config = AeroLogConfig::new(0xA1B2C3D4E5F6)
        .with_sync_mode(SyncMode::Periodic)
        .with_max_segment_size(32 * 1024 * 1024);

    let recovery_start = Instant::now();
    let db2 = AeroLogDb::open(dir.path(), reopen_config).unwrap();
    let recovery_elapsed = recovery_start.elapsed();
    let total_reindexed = count + stream_count;
    let recovery_ops = (total_reindexed as f64) / recovery_elapsed.as_secs_f64();

    println!("\n4. Cold Startup Recovery from Disk:");
    println!("   Records Re-indexed: {}", total_reindexed);
    println!("   Recovery Duration : {:.2?}", recovery_elapsed);
    println!("   Re-index Speed    : {:.0} records/sec", recovery_ops);

    for i in (0..count).step_by(2) {
        let key = format!("node_{:06}", i);
        db2.delete("telemetry", &key).unwrap();
    }

    let compact_start = Instant::now();
    let stats = db2.compact().unwrap();
    let compact_elapsed = compact_start.elapsed();

    println!("\n5. Log Compaction Reclaim:");
    println!("   Compaction Time   : {:.2?}", compact_elapsed);
    println!(
        "   Bytes Before      : {:.2} MB",
        stats.bytes_before as f64 / (1024.0 * 1024.0)
    );
    println!(
        "   Bytes After       : {:.2} MB",
        stats.bytes_after as f64 / (1024.0 * 1024.0)
    );
    println!("   Records Evicted   : {}", stats.records_evicted);
    println!(
        "   Disk Space Saved  : {:.1}%",
        if stats.bytes_before > 0 {
            (stats.bytes_before - stats.bytes_after) as f64 / stats.bytes_before as f64 * 100.0
        } else {
            0.0
        }
    );
    println!("==================================================================\n");

    assert!(write_ops > 10_000.0);
    assert!(read_ops > 10_000.0);
    assert!(recovery_ops > 10_000.0);
}

#[test]
fn test_concurrent_multithreaded_throughput() {
    use std::sync::Arc;
    use std::thread;

    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x998877665544)
        .with_sync_mode(SyncMode::Periodic)
        .with_max_segment_size(32 * 1024 * 1024);

    let db = Arc::new(AeroLogDb::open(dir.path(), config).unwrap());
    let records_per_thread = 10_000;
    let writer_threads = 4;
    let total_writes = records_per_thread * writer_threads;

    println!("\n==================================================================");
    println!("AeroLog Concurrent Multi-Threaded Benchmark (4 Writers, 4 Readers)");
    println!("==================================================================");

    let start = Instant::now();
    let mut handles = Vec::new();

    for t_idx in 0..writer_threads {
        let db_clone = Arc::clone(&db);
        handles.push(thread::spawn(move || {
            let payload = b"CONCURRENT_WORKER_THREAD_PAYLOAD_TEST_DATA".to_vec();
            for i in 0..records_per_thread {
                let key = format!("t{}_k{}", t_idx, i);
                db_clone.put("concurrent", &key, payload.clone()).unwrap();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();
    let throughput = (total_writes as f64) / elapsed.as_secs_f64();
    println!(
        "Wrote {} records across {} threads in {:.2?}",
        total_writes, writer_threads, elapsed
    );
    println!(
        "Concurrent Ingestion Throughput: {:.0} writes/sec",
        throughput
    );
    println!("==================================================================\n");

    assert!(throughput > 10_000.0);
}
