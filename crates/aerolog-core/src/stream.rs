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

use crate::error::{AeroLogError, Result};
use crate::hlc::HlcTimestamp;

/// Model representing a single entry retrieved from an append-only stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamEntry {
    /// Monotonically increasing sequence number within the stream.
    pub sequence: u64,
    /// Hybrid Logical Clock timestamp of the entry.
    pub hlc: HlcTimestamp,
    /// Raw payload bytes of the message or event.
    pub payload: Vec<u8>,
}

impl StreamEntry {
    /// Packs a monotonic sequence number as an 8-byte big-endian prefix onto user payload bytes.
    pub fn pack_payload(seq: u64, payload: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + payload.len());
        buf.extend_from_slice(&seq.to_be_bytes());
        buf.extend_from_slice(payload);
        buf
    }

    /// Defensively unpacks an 8-byte big-endian sequence prefix and user payload.
    pub fn unpack_payload(raw: &[u8]) -> Result<(u64, Vec<u8>)> {
        let seq = Self::peek_sequence(raw)?;
        let payload = raw[8..].to_vec();
        Ok((seq, payload))
    }

    /// Zero-allocation inspection function reading the 8-byte big-endian sequence prefix.
    pub fn peek_sequence(raw: &[u8]) -> Result<u64> {
        if raw.len() < 8 {
            return Err(AeroLogError::CorruptRecord {
                offset: 0,
                reason: format!(
                    "Stream payload size {} is smaller than 8-byte sequence prefix",
                    raw.len()
                ),
            });
        }
        let bytes: [u8; 8] = match raw[..8].try_into() {
            Ok(b) => b,
            Err(_) => {
                return Err(AeroLogError::CorruptRecord {
                    offset: 0,
                    reason: "Failed to extract 8-byte sequence prefix".to_string(),
                });
            }
        };
        Ok(u64::from_be_bytes(bytes))
    }
}

/// High-level interface for reading and writing to an append-only stream partition.
pub trait AeroLogStream: Send + Sync {
    /// Appends a new payload to the stream.
    /// Returns the assigned sequence number and HLC timestamp.
    fn append(&self, payload: Vec<u8>) -> Result<(u64, HlcTimestamp)>;

    /// Reads up to `limit` of the most recent entries in the stream, ordered chronologically.
    fn read_recent(&self, limit: usize) -> Result<Vec<StreamEntry>>;

    /// Returns the total number of entries recorded in this stream.
    fn total_entries(&self) -> u64;

    /// Returns the unique identifier of this stream.
    fn stream_id(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_wire_framing_roundtrip() {
        let original_seq = 123456789u64;
        let original_payload = b"sensor-temperature-reading:24.5C";

        let packed = StreamEntry::pack_payload(original_seq, original_payload);
        assert_eq!(packed.len(), 8 + original_payload.len());

        let peeked = StreamEntry::peek_sequence(&packed).unwrap();
        assert_eq!(peeked, original_seq);

        let (unpacked_seq, unpacked_payload) = StreamEntry::unpack_payload(&packed).unwrap();
        assert_eq!(unpacked_seq, original_seq);
        assert_eq!(unpacked_payload, original_payload);
    }

    #[test]
    fn test_stream_wire_framing_empty_payload() {
        let seq = 42u64;
        let empty_payload = b"";

        let packed = StreamEntry::pack_payload(seq, empty_payload);
        assert_eq!(packed.len(), 8);

        let peeked = StreamEntry::peek_sequence(&packed).unwrap();
        assert_eq!(peeked, seq);

        let (unpacked_seq, unpacked_payload) = StreamEntry::unpack_payload(&packed).unwrap();
        assert_eq!(unpacked_seq, seq);
        assert!(unpacked_payload.is_empty());
    }

    #[test]
    fn test_stream_wire_framing_truncated_payload_error() {
        let truncated = vec![0x00, 0x01, 0x02];
        let peek_err = StreamEntry::peek_sequence(&truncated);
        assert!(matches!(peek_err, Err(AeroLogError::CorruptRecord { .. })));

        let unpack_err = StreamEntry::unpack_payload(&truncated);
        assert!(matches!(
            unpack_err,
            Err(AeroLogError::CorruptRecord { .. })
        ));
    }
}
