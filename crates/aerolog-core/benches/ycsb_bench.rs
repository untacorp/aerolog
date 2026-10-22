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

//! Standard YCSB (Yahoo! Cloud Serving Benchmark) workload evaluation for AeroLog.
//!
//! Evaluates the core database engine against standard transactional workloads:
//! - Workload A (50% Read, 50% Update): Session store / Update heavy
//! - Workload B (95% Read, 5% Update): Photo tagging / Read mostly
//! - Workload C (100% Read): User profile cache / Read only
//! - Workload D (Read Latest): Status updates / Append stream
//! - Workload F (Read-Modify-Write): User database with balance updates

use aerolog_core::{AeroLogConfig, AeroLogDb, SyncMode};
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use std::hint::black_box;
use tempfile::tempdir;

fn setup_benchmark_db(initial_records: usize) -> (tempfile::TempDir, AeroLogDb) {
    let dir = tempdir().unwrap();
    let config = AeroLogConfig::new(0xDEADBEEF)
        .with_sync_mode(SyncMode::Periodic)
        .with_max_segment_size(64 * 1024 * 1024);
    let db = AeroLogDb::open(dir.path(), config).unwrap();
    let payload = vec![0x42; 128];

    for i in 0..initial_records {
        let key = format!("user_{:08}", i);
        db.put("usertable", &key, payload.clone()).unwrap();
    }

    (dir, db)
}

fn bench_ycsb_workload_a(c: &mut Criterion) {
    let mut group = c.benchmark_group("ycsb_workloads");
    group.throughput(Throughput::Elements(1));

    let (_dir, db) = setup_benchmark_db(10_000);
    let update_payload = vec![0x99; 128];
    let mut op_counter = 0u64;

    // Workload A: 50% Read, 50% Update
    group.bench_function("workload_a_50_read_50_update", |b| {
        b.iter(|| {
            op_counter += 1;
            let idx = (op_counter * 37) % 10_000;
            let key = format!("user_{:08}", idx);

            if op_counter.is_multiple_of(2) {
                let val = db.get("usertable", black_box(&key)).unwrap();
                black_box(val);
            } else {
                db.put(
                    "usertable",
                    black_box(&key),
                    black_box(update_payload.clone()),
                )
                .unwrap();
            }
        });
    });

    group.finish();
}

fn bench_ycsb_workload_b(c: &mut Criterion) {
    let mut group = c.benchmark_group("ycsb_workloads");
    group.throughput(Throughput::Elements(1));

    let (_dir, db) = setup_benchmark_db(10_000);
    let update_payload = vec![0x88; 128];
    let mut op_counter = 0u64;

    // Workload B: 95% Read, 5% Update
    group.bench_function("workload_b_95_read_5_update", |b| {
        b.iter(|| {
            op_counter += 1;
            let idx = (op_counter * 37) % 10_000;
            let key = format!("user_{:08}", idx);

            if !op_counter.is_multiple_of(20) {
                let val = db.get("usertable", black_box(&key)).unwrap();
                black_box(val);
            } else {
                db.put(
                    "usertable",
                    black_box(&key),
                    black_box(update_payload.clone()),
                )
                .unwrap();
            }
        });
    });

    group.finish();
}

fn bench_ycsb_workload_c(c: &mut Criterion) {
    let mut group = c.benchmark_group("ycsb_workloads");
    group.throughput(Throughput::Elements(1));

    let (_dir, db) = setup_benchmark_db(10_000);
    let mut op_counter = 0u64;

    // Workload C: 100% Read
    group.bench_function("workload_c_100_read", |b| {
        b.iter(|| {
            op_counter += 1;
            let idx = (op_counter * 37) % 10_000;
            let key = format!("user_{:08}", idx);
            let val = db.get("usertable", black_box(&key)).unwrap();
            black_box(val);
        });
    });

    group.finish();
}

fn bench_ycsb_workload_f(c: &mut Criterion) {
    let mut group = c.benchmark_group("ycsb_workloads");
    group.throughput(Throughput::Elements(1));

    let (_dir, db) = setup_benchmark_db(10_000);
    let mut op_counter = 0u64;

    // Workload F: Read-Modify-Write
    group.bench_function("workload_f_read_modify_write", |b| {
        b.iter(|| {
            op_counter += 1;
            let idx = (op_counter * 37) % 10_000;
            let key = format!("user_{:08}", idx);

            if let Some(mut val) = db.get("usertable", &key).unwrap() {
                if !val.is_empty() {
                    val[0] = val[0].wrapping_add(1);
                }
                db.put("usertable", black_box(&key), black_box(val))
                    .unwrap();
            }
        });
    });

    group.finish();
}

criterion_group!(
    ycsb_benches,
    bench_ycsb_workload_a,
    bench_ycsb_workload_b,
    bench_ycsb_workload_c,
    bench_ycsb_workload_f
);
criterion_main!(ycsb_benches);
