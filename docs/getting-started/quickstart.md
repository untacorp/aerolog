# 5-Minute Quickstart

Get up and running with AeroLog across your preferred language in under five minutes.

---

## The Core Lifecycle

Every AeroLog integration follows four steps:
1. **Open Database**: Provide a storage directory path and configuration.
2. **Point Key-Value Operations**: Write and read mutable entities by collection and key.
3. **Stream Operations**: Append ordered events to named stream partitions.
4. **Shutdown / Disposal**: Close file handles and flush active segments cleanly.

---

## Code Walkthrough

=== "Rust"

    ```rust
    use aerolog_core::{AeroLogDb, AeroLogConfig, Result};

    fn main() -> Result<()> {
        // 1. Open Database
        let config = AeroLogConfig::new("./data_demo")
            .with_max_segment_size(32 * 1024 * 1024); // 32 MB segments
        let db = AeroLogDb::open(config)?;

        // 2. Mutable Point Write (Collection, Key, Payload)
        db.put("devices", "sensor_01", b"{\"status\":\"ONLINE\",\"temp\":24.2}")?;

        // 3. Point Read via O(1) KeyDir Index
        if let Some(payload) = db.get("devices", "sensor_01")? {
            println!("Device 01: {}", String::from_utf8_lossy(&payload));
        }

        // 4. Append-Only Stream (e.g. Telemetry Log)
        let stream = db.stream("telemetry:sensor_01")?;
        let offset = stream.append(b"{\"reading\":24.2,\"unit\":\"C\"}")?;
        println!("Appended event at stream offset: {}", offset);

        // 5. Read Recent Stream Entries
        let recent_entries = stream.read_recent(10)?;
        for entry in recent_entries {
            println!("Offset {}: {} bytes", entry.offset, entry.payload.len());
        }

        Ok(())
    }
    ```

=== "Kotlin"

    ```kotlin
    import io.aerolog.uniffi.FfiAeroLogDb
    import java.io.File

    fun main() {
        val storageDir = File("./data_demo")
        storageDir.mkdirs()

        // 1. Open Database with AutoCloseable safety
        val db = FfiAeroLogDb.open(
            path = storageDir.absolutePath,
            maxSegmentSizeBytes = 32UL * 1024UL * 1024UL
        )

        db.use { engine ->
            // 2. Key-Value Write
            engine.put("devices", "sensor_01", "{\"status\":\"ONLINE\"}".toByteArray())

            // 3. Point Read
            val deviceBytes = engine.get("devices", "sensor_01")
            println("Device status: ${deviceBytes?.decodeToString()}")

            // 4. Stream Append
            val stream = engine.stream("telemetry:sensor_01")
            val offset = stream.append("{\"reading\": 24.2}".toByteArray())
            println("Stream offset: $offset")

            // 5. Read Recent Entries
            val entries = stream.readRecent(limit = 10u)
            entries.forEach { entry ->
                println("Offset ${entry.offset}: ${entry.payload.decodeToString()}")
            }
        }
    }
    ```

=== "Swift"

    ```swift
    import AeroLogUniFFI
    import Foundation

    func runStorage() throws {
        // 1. Open Database
        let db = try FfiAeroLogDb.open(
            path: "./data_demo",
            maxSegmentSizeBytes: 32 * 1024 * 1024
        )
        defer { db.shutdown() }

        // 2. Key-Value Write
        let payload = Data("{\"status\":\"ONLINE\"}".utf8)
        try db.put(collection: "devices", key: "sensor_01", value: payload)

        // 3. Point Read
        if let data = try db.get(collection: "devices", key: "sensor_01") {
            let jsonString = String(data: data, encoding: .utf8) ?? ""
            print("Device status: \(jsonString)")
        }

        // 4. Stream Append
        let stream = try db.stream(streamName: "telemetry:sensor_01")
        let offset = try stream.append(payload: Data("{\"reading\": 24.2}".utf8))
        print("Stream offset: \(offset)")

        // 5. Read Recent Entries
        let entries = try stream.readRecent(limit: 10)
        for entry in entries {
            print("Offset \(entry.offset): \(entry.payload.count) bytes")
        }
    }
    ```
