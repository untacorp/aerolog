# Android & JVM Guide (Kotlin)

AeroLog provides high-performance Kotlin bindings and an idiomatic Android/JVM adapter with zero-copy buffer handoffs and native coroutines compatibility.

---

## 1. Installation

### Gradle (Android AAR)

Add the dependency to your module's `build.gradle.kts`:

```kotlin
dependencies {
    implementation("io.aerolog:aerolog-android:0.1.0")
    implementation("net.java.dev.jna:jna:5.14.0@aar")
}
```

---

## 2. Lifecycle & AutoCloseable Management

`AeroLog` implements `java.lang.AutoCloseable`. Tie database instances to an Android `ViewModel`, `Application`, or repository:

```kotlin
import io.aerolog.AeroLog
import io.aerolog.AeroLogStream
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

class StorageRepository(dataDir: File) : AutoCloseable {

    private val db: AeroLog = AeroLog.open(
        dataDirectory = dataDir,
        nodeIdHash = 0UL
    )

    suspend fun saveUserProfile(userId: String, profileJson: String) = withContext(Dispatchers.IO) {
        db.put("profiles", userId, profileJson)
    }

    suspend fun getUserProfile(userId: String): String? = withContext(Dispatchers.IO) {
        db.getString("profiles", userId)
    }

    override fun close() {
        db.close()
    }
}
```

---

## 3. Managing Append-Only Streams in Coroutines

Streams provide high-throughput sequential appends for chat messaging, audit trails, and sensor telemetry:

```kotlin
suspend fun appendChatMessage(roomId: String, messageText: String): ULong = withContext(Dispatchers.IO) {
    val stream = db.stream("chat:$roomId")
    val entry = stream.append(messageText)
    entry.sequenceNumber
}

suspend fun loadRecentMessages(roomId: String, count: UInt): List<String> = withContext(Dispatchers.IO) {
    val stream = db.stream("chat:$roomId")
    stream.readRecent(count).map { entry ->
        entry.payload.decodeToString()
    }
}
```

---

## 4. Distributed Sync & Delta Scanning

Extract incremental mutations for peer-to-peer transmission (such as over Bluetooth Low Energy):

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

## 5. Background Compaction

Reclaim storage space during background maintenance tasks (e.g. Android `WorkManager`):

```kotlin
suspend fun runMaintenance() = withContext(Dispatchers.IO) {
    val stats = db.compact()
    println("Compaction completed: merged ${stats.filesMerged} files, reclaimed ${stats.bytesReclaimed} bytes.")
}
```
