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

use aerolog_uniffi::FfiAeroLogDb;
use tempfile::tempdir;

#[test]
fn test_uniffi_database_lifecycle_and_kv_crud() {
    let dir = tempdir().unwrap();
    let path = dir.path().to_str().unwrap().to_string();

    let db = FfiAeroLogDb::open_advanced(
        path.clone(),
        0x1234,
        Some(1024 * 1024),
        Some("SyncOnWrite".to_string()),
    )
    .unwrap();

    let hlc1 = db
        .put("users".to_string(), "u1".to_string(), b"Alice".to_vec())
        .unwrap();
    assert!(!hlc1.is_empty());

    let val = db.get("users".to_string(), "u1".to_string()).unwrap();
    assert_eq!(val, Some(b"Alice".to_vec()));

    let pairs = db.scan_collection("users".to_string()).unwrap();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].key, "u1");
    assert_eq!(pairs[0].value, b"Alice");

    let hlc2 = db.delete("users".to_string(), "u1".to_string()).unwrap();
    assert!(!hlc2.is_empty());
    assert_eq!(db.get("users".to_string(), "u1".to_string()).unwrap(), None);

    db.shutdown().unwrap();
    drop(db);

    let db2 = FfiAeroLogDb::open(path, 0x1234).unwrap();
    assert_eq!(
        db2.get("users".to_string(), "u1".to_string()).unwrap(),
        None
    );
    assert_eq!(db2.scan_collection("users".to_string()).unwrap().len(), 0);
}

#[test]
fn test_uniffi_stream_operations() {
    let dir = tempdir().unwrap();
    let path = dir.path().to_str().unwrap().to_string();

    let db = FfiAeroLogDb::open_advanced(path, 0x5678, Some(200), None).unwrap();
    let stream = db
        .get_stream_with_max_entries("telemetry".to_string(), 5)
        .unwrap();

    for i in 0..10 {
        let entry = stream.append(format!("msg_{i}").into_bytes()).unwrap();
        assert_eq!(entry.sequence, i as u64);
    }

    assert_eq!(stream.total_entries(), 10);

    let recent = stream.read_recent(5).unwrap();
    assert_eq!(recent.len(), 5);
    assert_eq!(recent[4].payload, b"msg_9");

    let stats = db.compact().unwrap();
    assert_eq!(stats.records_evicted, 5);

    let recent_after = stream.read_recent(5).unwrap();
    assert_eq!(recent_after.len(), 5);
    assert_eq!(recent_after[0].sequence, 5);
    assert_eq!(recent_after[0].payload, b"msg_5");
    assert_eq!(recent_after[4].sequence, 9);
    assert_eq!(recent_after[4].payload, b"msg_9");
}

#[test]
fn test_uniffi_peer_replication_hlc() {
    let dir = tempdir().unwrap();
    let path = dir.path().to_str().unwrap().to_string();

    let db = FfiAeroLogDb::open(path, 0x9999).unwrap();

    let accepted = db
        .put_with_hlc(
            "nodes".to_string(),
            "n1".to_string(),
            b"ONLINE".to_vec(),
            2000,
            0,
            0x8888,
        )
        .unwrap();
    assert!(accepted);

    let rejected = db
        .put_with_hlc(
            "nodes".to_string(),
            "n1".to_string(),
            b"OFFLINE".to_vec(),
            1000,
            0,
            0x8888,
        )
        .unwrap();
    assert!(!rejected);

    let mutations = db
        .scan_mutations_since(Some(1500), Some(0), Some(0), 10)
        .unwrap();
    assert_eq!(mutations.len(), 1);
    assert_eq!(mutations[0].payload, b"ONLINE");
}
