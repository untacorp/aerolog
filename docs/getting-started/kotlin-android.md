# Android & JVM Guide (Kotlin)

AeroLog provides high-performance Kotlin bindings generated via Mozilla UniFFI, offering zero-copy buffer handoffs and native coroutines compatibility.

---

## 1. Lifecycle & AutoCloseable Management

`FfiAeroLogDb` implements `java.lang.AutoCloseable`. Always wrap instances in Kotlin's `.use { }` block or tie them to an Android `ViewModel` / `Application` lifecycle:

```kotlin
import io.aerolog.uniffi.FfiAeroLogDb
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

class StorageRepository(dataDir: File) : AutoCloseable {

    private val db: FfiAeroLogDb = FfiAeroLogDb.open(
        path = dataDir.absolutePath,
        maxSegmentSizeBytes = 64UL * 1024UL * 1024UL
    )

    suspend fun saveUserProfile(userId: String, profileJson: String) = withContext(Dispatchers.IO) {
        db.put(
            collection = "profiles",
            key = userId,
            value = profileJson.toByteArray(Charsets.UTF_8)
        )
    }

    suspend fun getUserProfile(userId: String): String? = withContext(Dispatchers.IO) {
        db.get(collection = "profiles", key = userId)?.decodeToString()
    }

    override fun close() {
        db.close()
    }
}
```

---

## 2. Managing Append-Only Streams in Coroutines

Streams are ideal for chat messaging, audit logs, and sensor telemetry:

```kotlin
suspend fun appendChatMessage(roomId: String, messageText: String): ULong = withContext(Dispatchers.IO) {
    val stream = db.stream("chat:$roomId")
    stream.append(messageText.toByteArray(Charsets.UTF_8))
}

suspend fun loadRecentMessages(roomId: String, count: UInt): List<String> = withContext(Dispatchers.IO) {
    val stream = db.stream("chat:$roomId")
    stream.readRecent(count).map { entry ->
        entry.payload.decodeToString()
    }
}
```

---

## 3. Distributed Sync & Delta Scanning

Extract incremental mutations for peer-to-peer transmission (e.g. over Bluetooth Low Energy):

```kotlin
suspend fun getDeltaChanges(sinceTimestamp: ULong?): List<Pair<String, ByteArray>> = withContext(Dispatchers.IO) {
    val mutations = db.scanMutationsSince(
        sincePhysicalTime = sinceTimestamp,
        sinceLogicalCounter = null,
        sinceNodeHash = null,
        limit = 100u
    )

    mutations.map { record ->
        record.key to (db.get("profiles", record.key) ?: ByteArray(0))
    }
}
```

---

## 4. Triggering Compaction

Reclaim storage space during background maintenance tasks (e.g., Android `WorkManager`):

```kotlin
suspend fun runMaintenance() = withContext(Dispatchers.IO) {
    val stats = db.compact()
    println("Compaction completed: merged ${stats.filesMerged} files, reclaimed ${stats.bytesReclaimed} bytes.")
}
```
