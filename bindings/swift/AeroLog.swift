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

import Foundation

#if canImport(AeroLogUniFFI)
import AeroLogUniFFI
#endif

/// High-level idiomatic Swift actor providing thread-safe concurrency for the AeroLog engine.
public actor AeroLog {
    private let raw: FfiAeroLogDb

    /// Initializes and opens an AeroLog database instance at the designated file URL.
    public init(dataDirectory: URL, nodeIdHash: UInt64 = 0) throws {
        try FileManager.default.createDirectory(at: dataDirectory, withIntermediateDirectories: true)
        self.raw = try FfiAeroLogDb.open(path: dataDirectory.path, nodeIdHash: nodeIdHash)
    }

    /// Initializes and opens an AeroLog database in the standard Application Support directory.
    public init(name: String = "aerolog_data", nodeIdHash: UInt64 = 0) throws {
        let appSupport = try FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        let dbURL = appSupport.appendingPathComponent(name, isDirectory: true)
        try self.init(dataDirectory: dbURL, nodeIdHash: nodeIdHash)
    }

    /// Opens an AeroLog database with advanced segment size and fsync tuning options.
    public static func openAdvanced(
        dataDirectory: URL,
        nodeIdHash: UInt64 = 0,
        maxSegmentSizeBytes: UInt64? = nil,
        syncMode: String? = nil
    ) throws -> AeroLog {
        try FileManager.default.createDirectory(at: dataDirectory, withIntermediateDirectories: true)
        let rawDb = try FfiAeroLogDb.openAdvanced(
            path: dataDirectory.path,
            nodeIdHash: nodeIdHash,
            maxSegmentSizeBytes: maxSegmentSizeBytes,
            syncMode: syncMode
        )
        return AeroLog(raw: rawDb)
    }

    private init(raw: FfiAeroLogDb) {
        self.raw = raw
    }

    /// Stores a raw byte payload in the specified collection and key.
    public func put(collection: String, key: String, value: Data) throws -> String {
        return try raw.put(collection: collection, key: key, value: value)
    }

    /// Stores a UTF-8 string value in the specified collection and key.
    public func put(collection: String, key: String, text: String) throws -> String {
        return try put(collection: collection, key: key, value: Data(text.utf8))
    }

    /// Stores a key-value record with an explicit remote HLC timestamp for distributed replication.
    public func putWithHlc(
        collection: String,
        key: String,
        value: Data,
        physicalTime: UInt64,
        logicalCounter: UInt16,
        nodeIdHash: UInt64
    ) throws -> Bool {
        return try raw.putWithHlc(
            collection: collection,
            key: key,
            value: value,
            physicalTime: physicalTime,
            logicalCounter: logicalCounter,
            nodeIdHash: nodeIdHash
        )
    }

    /// Retrieves raw binary payload for a key. Returns nil if missing or tombstoned.
    public func get(collection: String, key: String) throws -> Data? {
        return try raw.get(collection: collection, key: key)
    }

    /// Retrieves a UTF-8 string value for a key. Returns nil if missing or tombstoned.
    public func getText(collection: String, key: String) throws -> String? {
        guard let data = try get(collection: collection, key: key) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// Appends a tombstone record marking the key as deleted.
    public func delete(collection: String, key: String) throws -> String {
        return try raw.delete(collection: collection, key: key)
    }

    /// Deletes a key with an explicit remote HLC timestamp under LWW conflict resolution.
    public func deleteWithHlc(
        collection: String,
        key: String,
        physicalTime: UInt64,
        logicalCounter: UInt16,
        nodeIdHash: UInt64
    ) throws -> Bool {
        return try raw.deleteWithHlc(
            collection: collection,
            key: key,
            physicalTime: physicalTime,
            logicalCounter: logicalCounter,
            nodeIdHash: nodeIdHash
        )
    }

    /// Scans all active key-value pairs in a collection.
    public func scanCollection(collection: String) throws -> [FfiKeyValuePair] {
        return try raw.scanCollection(collection: collection)
    }

    /// Scans incremental mutations strictly newer than the given physical timestamp.
    public func scanMutationsSince(
        sincePhysicalTime: UInt64? = nil,
        sinceLogicalCounter: UInt16? = nil,
        sinceNodeHash: UInt64? = nil,
        limit: UInt32 = 100
    ) throws -> [FfiMutationRecord] {
        return try raw.scanMutationsSince(
            sincePhysicalTime: sincePhysicalTime,
            sinceLogicalCounter: sinceLogicalCounter,
            sinceNodeHash: sinceNodeHash,
            limit: limit
        )
    }

    /// Opens an append-only stream partition handle.
    public func stream(streamId: String) throws -> AeroLogStream {
        let rawStream = try raw.getStream(streamId: streamId)
        return AeroLogStream(raw: rawStream)
    }

    /// Opens an append-only stream partition handle with MaxEntries retention.
    public func streamWithMaxEntries(streamId: String, maxEntries: UInt64) throws -> AeroLogStream {
        let rawStream = try raw.getStreamWithMaxEntries(streamId: streamId, maxEntries: maxEntries)
        return AeroLogStream(raw: rawStream)
    }

    /// Opens an append-only stream partition handle with TimeToLive retention.
    public func streamWithTtl(streamId: String, ttlMillis: UInt64) throws -> AeroLogStream {
        let rawStream = try raw.getStreamWithTtl(streamId: streamId, ttlMillis: ttlMillis)
        return AeroLogStream(raw: rawStream)
    }

    /// Triggers background compaction, merging inactive segments and purging dead tombstones.
    public func compact() throws -> FfiCompactionStats {
        return try raw.compact()
    }

    /// Flushes unwritten append buffers to OS disk cache.
    public func flush() throws {
        try raw.flush()
    }

    /// Shuts down the database and releases file descriptors.
    public func close() throws {
        try raw.shutdown()
    }

    deinit {
        try? raw.shutdown()
    }
}

/// Thread-safe wrapper for append-only sequential stream partitions.
public struct AeroLogStream: Sendable {
    private let raw: FfiStream

    internal init(raw: FfiStream) {
        self.raw = raw
    }

    /// Appends a raw byte payload to the stream.
    public func append(payload: Data) throws -> FfiStreamEntry {
        return try raw.append(payload: payload)
    }

    /// Appends a UTF-8 text string to the stream.
    public func append(text: String) throws -> FfiStreamEntry {
        return try append(payload: Data(text.utf8))
    }

    /// Reads up to limit most recent entries in chronological order.
    public func readRecent(limit: UInt32 = 10) throws -> [FfiStreamEntry] {
        return try raw.readRecent(limit: limit)
    }

    /// Returns the unique partition stream identifier.
    public var streamId: String {
        return raw.streamId()
    }

    /// Returns the total count of entries appended to this partition.
    public var totalEntries: UInt64 {
        return raw.totalEntries()
    }
}
