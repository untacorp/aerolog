# Apple Platforms Guide (Swift)

AeroLog provides thread-safe Swift bindings for iOS, iPadOS, macOS, and visionOS with modern Swift concurrency (`actor` and `async/await`).

---

## 1. Installation

### Swift Package Manager (Xcode)

Add AeroLog to your dependencies in `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/untacorp/aerolog.git", from: "0.1.0")
]
```

Or add the repository URL directly via Xcode: **File > Add Package Dependencies**.

---

## 2. Opening and Initializing AeroLog

Initialize `AeroLog` inside an actor or storage service. By default, `AeroLog(name:)` places database files in the platform's standard `Application Support` directory:

```swift
import AeroLog
import Foundation

public actor StorageService {
    private let db: AeroLog

    public init() throws {
        // Automatically resolves Application Support / aerolog_data
        self.db = try AeroLog(name: "app_data")
    }

    public init(customURL: URL) throws {
        self.db = try AeroLog(dataDirectory: customURL)
    }

    public func close() async throws {
        try await db.close()
    }
}
```

---

## 3. Point Key-Value Operations

```swift
extension StorageService {
    public func saveUserProfile(userId: String, json: String) async throws {
        _ = try await db.put(collection: "profiles", key: userId, text: json)
    }

    public func fetchUserProfile(userId: String) async throws -> String? {
        return try await db.getText(collection: "profiles", key: userId)
    }

    public func deleteUserProfile(userId: String) async throws {
        _ = try await db.delete(collection: "profiles", key: userId)
    }
}
```

---

## 4. Streaming Event Log

Append-only streams provide sequential storage for event logs, chat rooms, and telemetry:

```swift
extension StorageService {
    public func appendMessage(roomId: String, messageText: String) async throws -> UInt64 {
        let stream = try await db.stream(streamId: "chat:\(roomId)")
        let entry = try stream.append(text: messageText)
        return entry.sequenceNumber
    }

    public func fetchRecentMessages(roomId: String, limit: UInt32 = 20) async throws -> [String] {
        let stream = try await db.stream(streamId: "chat:\(roomId)")
        let entries = try stream.readRecent(limit: limit)
        return entries.compactMap { String(data: $0.payload, encoding: .utf8) }
    }
}
```

---

## 5. Background Compaction

Trigger compaction during background tasks (such as `BGProcessingTask` in iOS):

```swift
extension StorageService {
    public func runMaintenance() async throws -> FfiCompactionStats {
        return try await db.compact()
    }
}
```
