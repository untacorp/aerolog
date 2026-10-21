# UniFFI Foreign API Contract

This reference details the foreign interface exposed by `aerolog-uniffi` to Kotlin and Swift.

---

## 1. `FfiAeroLogDb`

### Constructors
* **Kotlin**: `FfiAeroLogDb.open(path: String, maxSegmentSizeBytes: ULong): FfiAeroLogDb`
* **Swift**: `FfiAeroLogDb.open(path: String, maxSegmentSizeBytes: UInt64) throws -> FfiAeroLogDb`

### Methods
* `put(collection: String, key: String, value: ByteArray/Data)`
* `get(collection: String, key: String): ByteArray?/Data?`
* `delete(collection: String, key: String)`
* `stream(streamName: String): FfiStream`
* `scanCollection(collection: String): List<FfiKeyValuePair> / [FfiKeyValuePair]`
* `scanMutationsSince(sincePhysicalTime: ULong?, sinceLogicalCounter: UShort?, sinceNodeHash: ULong?, limit: UInt): List<FfiMutationRecord> / [FfiMutationRecord]`
* `compact(): FfiCompactionStats`
* `shutdown()`

---

## 2. `FfiStream`

### Methods
* `append(payload: ByteArray/Data): ULong / UInt64`
* `readRecent(limit: UInt): List<FfiStreamEntry> / [FfiStreamEntry]`
* `setRetentionMaxEntries(maxEntries: ULong / UInt64)`
* `setRetentionTtl(ttlSeconds: ULong / UInt64)`

---

## 3. Data Records

### `FfiMutationRecord`
```kotlin
data class FfiMutationRecord(
    val key: String,
    val recordType: UByte,
    val physicalTime: ULong,
    val logicalCounter: UShort,
    val nodeIdHash: ULong,
    val collection: String
)
```

### `FfiStreamEntry`
```kotlin
data class FfiStreamEntry(
    val offset: ULong,
    val physicalTime: ULong,
    val payload: ByteArray
)
```

### `FfiCompactionStats`
```kotlin
data class FfiCompactionStats(
    val filesMerged: UInt,
    val bytesReclaimed: ULong
)
```
