# Apple Platforms Guide (Swift)

AeroLog provides seamless, memory-safe Swift bindings for iOS, iPadOS, macOS, and visionOS via Mozilla UniFFI.

---

## 1. Opening and Initializing AeroLog in Swift

Initialize the database inside an actor or storage service class:

```swift
import AeroLogUniFFI
import Foundation

public actor AeroLogStorageService {
    private let db: FfiAeroLogDb

    public init(directoryURL: URL) throws {
        try FileManager.default.createDirectory(
            at: directoryURL,
            withIntermediateDirectories: true
        )
        self.db = try FfiAeroLogDb.open(
            path: directoryURL.path,
            maxSegmentSizeBytes: 64 * 1024 * 1024 // 64 MB
        )
    }

    deinit {
        db.shutdown()
    }
}
```

---

## 2. Point Key-Value Operations

```swift
extension AeroLogStorageService {
    public func save(collection: String, key: String, data: Data) throws {
        try db.put(collection: collection, key: key, value: data)
    }

    public func fetch(collection: String, key: String) throws -> Data? {
        try db.get(collection: collection, key: key)
    }

    public func delete(collection: String, key: String) throws {
        try db.delete(collection: collection, key: key)
    }
}
```

---

## 3. Streaming Event Log

```swift
extension AeroLogStorageService {
    public func appendEvent(streamName: String, payload: Data) throws -> UInt64 {
        let stream = try db.stream(streamName: streamName)
        return try stream.append(payload: payload)
    }

    public func fetchRecent(streamName: String, limit: UInt32) throws -> [Data] {
        let stream = try db.stream(streamName: streamName)
        let entries = try stream.readRecent(limit: limit)
        return entries.map { $0.payload }
    }
}
```

---

## 4. Background Compaction

Invoke compaction during app background refresh or idle states:

```swift
extension AeroLogStorageService {
    public func performCompaction() throws -> FfiCompactionStats {
        return try db.compact()
    }
}
```
