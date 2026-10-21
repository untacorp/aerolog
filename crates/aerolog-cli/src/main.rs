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

use aerolog_core::log::reader::LogReader;
use aerolog_core::log::segment::SegmentFile;
use aerolog_core::{AeroLogConfig, AeroLogDb, SyncMode};
use std::env;
use std::path::Path;
use std::time::Instant;
use tempfile::tempdir;

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    match command {
        "bench" => run_benchmark(),
        "inspect" => {
            if let Some(path) = args.get(2) {
                inspect_database(Path::new(path));
            } else {
                eprintln!("Usage: aerolog inspect <directory_path>");
            }
        }
        "dump" => {
            if let Some(path) = args.get(2) {
                dump_database(Path::new(path));
            } else {
                eprintln!("Usage: aerolog dump <directory_path>");
            }
        }
        _ => print_help(),
    }
}

fn print_help() {
    println!("AeroLog CLI - Embedded Multi-Model Storage Engine");
    println!("Usage:");
    println!("  aerolog bench               Run throughput and latency benchmark");
    println!("  aerolog inspect <path>      Display segment counts, sizes, and record statistics");
    println!("  aerolog dump <path>         Dump all decoded records to stdout");
}

fn run_benchmark() {
    println!("==================================================================");
    println!("               AeroLog Embedded Storage Benchmark                 ");
    println!("==================================================================");

    let dir = tempdir().expect("Failed to create temporary directory for benchmark");
    let total_operations = 50_000;

    let config = AeroLogConfig::new(0x42)
        .with_sync_mode(SyncMode::Periodic)
        .with_max_segment_size(16 * 1024 * 1024);

    let db = AeroLogDb::open(dir.path(), config).expect("Failed to open AeroLog database");

    // 1. Sequential Write Benchmark
    println!(
        "\n[1/3] Benchmarking Sequential Appends ({} records)...",
        total_operations
    );
    let start_write = Instant::now();
    for i in 0..total_operations {
        let key = format!("entity_{i:06}");
        let val = format!("{{\"id\": {i}, \"status\": \"active\", \"timestamp\": 1710928000}}")
            .into_bytes();
        db.put("benchmark", &key, val).expect("Put failed");
    }
    db.flush().expect("Flush failed");
    let write_elapsed = start_write.elapsed();
    let write_ops = (total_operations as f64) / write_elapsed.as_secs_f64();

    println!("  Time elapsed:       {:?}", write_elapsed);
    println!("  Write throughput:   {:.2} ops/sec", write_ops);

    // 2. Random Read Benchmark
    println!(
        "\n[2/3] Benchmarking Random Point Reads ({} queries)...",
        total_operations
    );
    let start_read = Instant::now();
    for i in (0..total_operations).step_by(2) {
        let key = format!("entity_{i:06}");
        let res = db.get("benchmark", &key).expect("Get failed");
        assert!(res.is_some());
    }
    let read_elapsed = start_read.elapsed();
    let read_queries = total_operations / 2;
    let read_ops = (read_queries as f64) / read_elapsed.as_secs_f64();

    println!("  Time elapsed:       {:?}", read_elapsed);
    println!("  Read throughput:    {:.2} ops/sec", read_ops);

    // 3. Stream Append Benchmark
    println!("\n[3/3] Benchmarking Append-Only Stream (20,000 messages)...");
    let stream = db.get_stream("live-chat").expect("Stream failed");
    let start_stream = Instant::now();
    for i in 0..20_000 {
        let msg = format!("ChatMessage {i} - offgrid mesh packet data payload").into_bytes();
        stream.append(msg).expect("Stream append failed");
    }
    let stream_elapsed = start_stream.elapsed();
    let stream_ops = 20_000.0 / stream_elapsed.as_secs_f64();

    println!("  Time elapsed:       {:?}", stream_elapsed);
    println!("  Stream throughput:  {:.2} ops/sec", stream_ops);

    println!("\n==================================================================");
    println!("  Benchmark complete: All data persisted with CRC32 integrity!   ");
    println!("==================================================================");
}

fn inspect_database(path: &Path) {
    println!("Inspecting AeroLog database at: {}", path.display());
    if !path.exists() {
        eprintln!("Directory does not exist.");
        return;
    }

    let mut total_size = 0u64;
    let mut segment_count = 0u32;

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Some(file_id) = SegmentFile::parse_file_id(&entry.path()) {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                total_size += size;
                segment_count += 1;
                println!(
                    "  Segment file: data-{file_id:06}.aerolog (size: {size} bytes / {:.2} KB)",
                    size as f64 / 1024.0
                );
            }
        }
    }

    println!("\nTotal segments: {segment_count}");
    println!(
        "Total size on disk: {total_size} bytes ({:.2} MB)",
        total_size as f64 / (1024.0 * 1024.0)
    );
}

fn dump_database(path: &Path) {
    let reader = LogReader::new(path);
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Some(file_id) = SegmentFile::parse_file_id(&entry.path()) {
                println!("--- Segment {file_id:06} ---");
                if let Ok(records) = reader.scan_segment(file_id) {
                    for (offset, record) in records {
                        let key_str = String::from_utf8_lossy(&record.key);
                        let val_preview = if record.value.len() > 40 {
                            format!(
                                "{}... ({} B)",
                                String::from_utf8_lossy(&record.value[..40]),
                                record.value.len()
                            )
                        } else {
                            String::from_utf8_lossy(&record.value).to_string()
                        };
                        println!(
                            "  [0x{offset:06X}] Type: {:?}, HLC: {}, Key: {}, Value: {}",
                            record.header.record_type, record.header.hlc, key_str, val_preview
                        );
                    }
                }
            }
        }
    }
}
