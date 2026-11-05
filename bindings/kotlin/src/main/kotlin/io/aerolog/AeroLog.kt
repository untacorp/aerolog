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

package io.aerolog

import io.aerolog.uniffi.FfiAeroLogDb
import io.aerolog.uniffi.FfiCompactionStats
import io.aerolog.uniffi.FfiKeyValuePair
import io.aerolog.uniffi.FfiMutationRecord
import io.aerolog.uniffi.FfiStream
import io.aerolog.uniffi.FfiStreamEntry
import java.io.File

/**
 * High-level idiomatic Kotlin facade for the AeroLog embedded storage engine.
 */
class AeroLog private constructor(private val raw: FfiAeroLogDb) : AutoCloseable {

    companion object {
        fun open(dataDirectory: File, nodeIdHash: ULong = 0UL): AeroLog {
            if (!dataDirectory.exists()) {
                dataDirectory.mkdirs()
            }
            val rawDb = FfiAeroLogDb.open(dataDirectory.absolutePath, nodeIdHash)
            return AeroLog(rawDb)
        }

        fun open(path: String, nodeIdHash: ULong = 0UL): AeroLog {
            return open(File(path), nodeIdHash)
        }

        fun openAdvanced(
            dataDirectory: File,
            nodeIdHash: ULong = 0UL,
            maxSegmentSizeBytes: ULong? = null,
            syncMode: String? = null
        ): AeroLog {
            if (!dataDirectory.exists()) {
                dataDirectory.mkdirs()
            }
            val rawDb = FfiAeroLogDb.openAdvanced(
                dataDirectory.absolutePath,
                nodeIdHash,
                maxSegmentSizeBytes,
                syncMode
            )
            return AeroLog(rawDb)
        }

        fun openAdvanced(
            path: String,
            nodeIdHash: ULong = 0UL,
            maxSegmentSizeBytes: ULong? = null,
            syncMode: String? = null
        ): AeroLog {
            return openAdvanced(File(path), nodeIdHash, maxSegmentSizeBytes, syncMode)
        }
    }

    fun put(collection: String, key: String, value: ByteArray): String {
        return raw.put(collection, key, value)
    }

    fun put(collection: String, key: String, value: String): String {
        return put(collection, key, value.toByteArray(Charsets.UTF_8))
    }

    fun putWithHlc(
        collection: String,
        key: String,
        value: ByteArray,
        physicalTime: ULong,
        logicalCounter: UShort,
        nodeIdHash: ULong
    ): Boolean {
        return raw.putWithHlc(collection, key, value, physicalTime, logicalCounter, nodeIdHash)
    }

    fun putWithHlc(
        collection: String,
        key: String,
        value: String,
        physicalTime: ULong,
        logicalCounter: UShort,
        nodeIdHash: ULong
    ): Boolean {
        return putWithHlc(
            collection,
            key,
            value.toByteArray(Charsets.UTF_8),
            physicalTime,
            logicalCounter,
            nodeIdHash
        )
    }

    fun get(collection: String, key: String): ByteArray? {
        return raw.get(collection, key)
    }

    fun getString(collection: String, key: String): String? {
        return get(collection, key)?.decodeToString()
    }

    fun delete(collection: String, key: String): String {
        return raw.delete(collection, key)
    }

    fun deleteWithHlc(
        collection: String,
        key: String,
        physicalTime: ULong,
        logicalCounter: UShort,
        nodeIdHash: ULong
    ): Boolean {
        return raw.deleteWithHlc(collection, key, physicalTime, logicalCounter, nodeIdHash)
    }

    fun scanCollection(collection: String): List<FfiKeyValuePair> {
        return raw.scanCollection(collection)
    }

    fun scanMutationsSince(
        sincePhysicalTime: ULong? = null,
        sinceLogicalCounter: UShort? = null,
        sinceNodeHash: ULong? = null,
        limit: UInt = 100u
    ): List<FfiMutationRecord> {
        return raw.scanMutationsSince(sincePhysicalTime, sinceLogicalCounter, sinceNodeHash, limit)
    }

    fun stream(streamId: String): AeroLogStream {
        return AeroLogStream(raw.getStream(streamId))
    }

    fun streamWithMaxEntries(streamId: String, maxEntries: ULong): AeroLogStream {
        return AeroLogStream(raw.getStreamWithMaxEntries(streamId, maxEntries))
    }

    fun streamWithTtl(streamId: String, ttlMillis: ULong): AeroLogStream {
        return AeroLogStream(raw.getStreamWithTtl(streamId, ttlMillis))
    }

    fun compact(): FfiCompactionStats {
        return raw.compact()
    }

    fun flush() {
        raw.flush()
    }

    override fun close() {
        try {
            raw.shutdown()
        } finally {
            raw.close()
        }
    }
}

/**
 * Idiomatic wrapper for append-only stream partitions.
 */
class AeroLogStream internal constructor(private val raw: FfiStream) : AutoCloseable {

    fun append(payload: ByteArray): FfiStreamEntry {
        return raw.append(payload)
    }

    fun append(text: String): FfiStreamEntry {
        return append(text.toByteArray(Charsets.UTF_8))
    }

    fun readRecent(limit: UInt = 10u): List<FfiStreamEntry> {
        return raw.readRecent(limit)
    }

    val streamId: String
        get() = raw.streamId()

    val totalEntries: ULong
        get() = raw.totalEntries()

    override fun close() {
        raw.close()
    }
}
