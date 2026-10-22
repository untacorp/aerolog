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
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use tempfile::tempdir;

fn bench_sequential_puts(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine_sequential_puts");

    for size in [64, 512, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{size}B")),
            size,
            |b, &s| {
                let dir = tempdir().unwrap();
                let config = AeroLogConfig::new(0x12345678)
                    .with_sync_mode(SyncMode::Periodic)
                    .with_max_segment_size(64 * 1024 * 1024);
                let db = AeroLogDb::open(dir.path(), config).unwrap();
                let payload = vec![0xAB; s];
                let mut counter = 0u64;

                b.iter(|| {
                    counter += 1;
                    let key = format!("k_{:010}", counter);
                    db.put("benchmark", black_box(&key), black_box(payload.clone()))
                        .unwrap();
                });
            },
        );
    }
    group.finish();
}

fn bench_random_point_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine_point_reads");
    group.throughput(Throughput::Elements(1));

    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0x12345678)
        .with_sync_mode(SyncMode::Periodic)
        .with_max_segment_size(64 * 1024 * 1024);
    let db = AeroLogDb::open(dir.path(), config).unwrap();
    let payload = vec![0xCD; 128];
    let num_keys = 10_000;

    for i in 0..num_keys {
        let key = format!("k_{:06}", i);
        db.put("benchmark", &key, payload.clone()).unwrap();
    }

    let mut counter = 0u64;
    group.bench_function("point_get_10k_dataset", |b| {
        b.iter(|| {
            counter += 1;
            let idx = (counter * 37) % (num_keys as u64);
            let key = format!("k_{:06}", idx);
            let val = db.get("benchmark", black_box(&key)).unwrap();
            black_box(val);
        });
    });

    group.finish();
}

fn bench_stream_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine_stream_ops");

    // Benchmark Stream Append Throughput
    group.throughput(Throughput::Elements(1));
    group.bench_function("stream_append", |b| {
        let dir = tempdir().unwrap();
        let config = AeroLogConfig::new(0x12345678)
            .with_sync_mode(SyncMode::Periodic)
            .with_max_segment_size(64 * 1024 * 1024);
        let db = AeroLogDb::open(dir.path(), config).unwrap();
        let stream = db.get_stream("events").unwrap();
        let payload = vec![0xEF; 64];

        b.iter(|| {
            stream.append(black_box(payload.clone())).unwrap();
        });
    });

    // Benchmark Stream Recent Retrieval (Sparse Index Seek)
    group.bench_function("stream_read_recent_100", |b| {
        let dir = tempdir().unwrap();
        let config = AeroLogConfig::new(0x12345678)
            .with_sync_mode(SyncMode::Periodic)
            .with_sparse_checkpoint_interval(50)
            .with_max_segment_size(64 * 1024 * 1024);
        let db = AeroLogDb::open(dir.path(), config).unwrap();
        let stream = db.get_stream("events").unwrap();
        let payload = vec![0xEF; 64];

        for _ in 0..5_000 {
            stream.append(payload.clone()).unwrap();
        }

        b.iter(|| {
            let entries = stream.read_recent(black_box(100)).unwrap();
            black_box(entries);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sequential_puts,
    bench_random_point_reads,
    bench_stream_operations
);
criterion_main!(benches);
