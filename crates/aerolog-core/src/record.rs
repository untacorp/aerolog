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
use byteorder::{BigEndian, ByteOrder};
use crc32fast::Hasher;

/// Fixed magic bytes identifying an AeroLog segment record: ASCII "AL" (0x41 0x4C).
pub const MAGIC_BYTES: u16 = 0x414C;

/// Current wire and storage layout version format.
pub const CURRENT_VERSION: u8 = 0x01;

/// Fixed byte size of an encoded record header.
pub const HEADER_SIZE: usize = 32;

/// Maximum key size in bytes (64 KB).
pub const MAX_KEY_SIZE: usize = u16::MAX as usize;

/// Maximum value payload size in bytes (256 MB).
pub const MAX_VALUE_SIZE: usize = 256 * 1024 * 1024;

/// Classification of mutations stored in an `.aerolog` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordType {
    /// Active Key-Value entity write.
    Put = 0x00,
    /// Soft-deletion tombstone.
    Delete = 0x01,
    /// Append-only message or event in a stream partition.
    StreamEntry = 0x02,
}

impl RecordType {
    /// Converts a raw byte to a [`RecordType`].
    pub fn from_u8(value: u8) -> Result<Self> {
        match value {
            0x00 => Ok(Self::Put),
            0x01 => Ok(Self::Delete),
            0x02 => Ok(Self::StreamEntry),
            other => Err(AeroLogError::CorruptRecord {
                offset: 0,
                reason: format!("Unknown record type code: 0x{other:02X}"),
            }),
        }
    }
}

/// Fixed 32-byte binary header for each record in an `.aerolog` segment file.
///
/// Layout (Network Byte Order / Big-Endian):
/// ```text
/// Offset (B)  Size (B)  Field
/// 00 - 03     4         CRC32 Checksum (covers bytes 04..32 + key + value)
/// 04 - 05     2         Magic Bytes (0x41 0x4C)
/// 06          1         Version (0x01)
/// 07          1         Record Type (0x00=Put, 0x01=Delete, 0x02=StreamEntry)
/// 08 - 15     8         HLC Physical Time (Epoch ms)
/// 16 - 17     2         HLC Logical Counter
/// 18 - 25     8         Node ID Hash (64-bit originating node fingerprint)
/// 26 - 27     2         Key Length (0 - 65,535 bytes)
/// 28 - 31     4         Value / Payload Length (0 - 4 GB)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordHeader {
    /// Checksum verifying integrity of header metadata, key, and value payload.
    pub crc32: u32,
    /// Magic signature bytes (0x414C).
    pub magic: u16,
    /// Record serialization layout version.
    pub version: u8,
    /// Mutation category.
    pub record_type: RecordType,
    /// Hybrid Logical Clock timestamp.
    pub hlc: HlcTimestamp,
    /// Byte length of the UTF-8 key.
    pub key_len: u16,
    /// Byte length of the trailing payload.
    pub value_len: u32,
}

impl RecordHeader {
    /// Encodes the header into a 32-byte array.
    pub fn encode(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        BigEndian::write_u32(&mut buf[0..4], self.crc32);
        BigEndian::write_u16(&mut buf[4..6], self.magic);
        buf[6] = self.version;
        buf[7] = self.record_type as u8;
        BigEndian::write_u64(&mut buf[8..16], self.hlc.physical_time);
        BigEndian::write_u16(&mut buf[16..18], self.hlc.logical_counter);
        BigEndian::write_u64(&mut buf[18..26], self.hlc.node_id_hash);
        BigEndian::write_u16(&mut buf[26..28], self.key_len);
        BigEndian::write_u32(&mut buf[28..32], self.value_len);
        buf
    }

    /// Decodes a 32-byte buffer into a [`RecordHeader`].
    pub fn decode(buf: &[u8; HEADER_SIZE]) -> Result<Self> {
        let crc32 = BigEndian::read_u32(&buf[0..4]);
        let magic = BigEndian::read_u16(&buf[4..6]);
        if magic != MAGIC_BYTES {
            return Err(AeroLogError::InvalidMagic {
                expected: MAGIC_BYTES,
                actual: magic,
            });
        }

        let version = buf[6];
        if version != CURRENT_VERSION {
            return Err(AeroLogError::UnsupportedVersion(version));
        }

        let record_type = RecordType::from_u8(buf[7])?;
        let physical_time = BigEndian::read_u64(&buf[8..16]);
        let logical_counter = BigEndian::read_u16(&buf[16..18]);
        let node_id_hash = BigEndian::read_u64(&buf[18..26]);
        let key_len = BigEndian::read_u16(&buf[26..28]);
        let value_len = BigEndian::read_u32(&buf[28..32]);

        Ok(Self {
            crc32,
            magic,
            version,
            record_type,
            hlc: HlcTimestamp::new(physical_time, logical_counter, node_id_hash),
            key_len,
            value_len,
        })
    }

    /// Computes the CRC32 checksum over the header fields (excluding CRC32 itself), key, and payload.
    pub fn compute_crc(
        record_type: RecordType,
        hlc: &HlcTimestamp,
        key: &[u8],
        value: &[u8],
    ) -> u32 {
        let mut hasher = Hasher::new();

        // 28 bytes of header after CRC32 field
        let mut header_tail = [0u8; 28];
        BigEndian::write_u16(&mut header_tail[0..2], MAGIC_BYTES);
        header_tail[2] = CURRENT_VERSION;
        header_tail[3] = record_type as u8;
        BigEndian::write_u64(&mut header_tail[4..12], hlc.physical_time);
        BigEndian::write_u16(&mut header_tail[12..14], hlc.logical_counter);
        BigEndian::write_u64(&mut header_tail[14..22], hlc.node_id_hash);
        BigEndian::write_u16(&mut header_tail[22..24], key.len() as u16);
        BigEndian::write_u32(&mut header_tail[24..28], value.len() as u32);

        hasher.update(&header_tail);
        hasher.update(key);
        hasher.update(value);
        hasher.finalize()
    }

    /// Calculates total record size on disk (header + key + value).
    pub fn total_record_size(&self) -> u64 {
        HEADER_SIZE as u64 + self.key_len as u64 + self.value_len as u64
    }
}

/// Complete in-memory representation of an uncompressed database entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Decoded fixed 32-byte header.
    pub header: RecordHeader,
    /// Raw byte representation of the key.
    pub key: Vec<u8>,
    /// Value payload bytes.
    pub value: Vec<u8>,
}

impl Record {
    /// Constructs a new record, automatically calculating its CRC32 checksum.
    pub fn new(
        record_type: RecordType,
        hlc: HlcTimestamp,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<Self> {
        if key.len() > MAX_KEY_SIZE {
            return Err(AeroLogError::SizeLimitExceeded(format!(
                "Key size {} exceeds maximum allowable {}",
                key.len(),
                MAX_KEY_SIZE
            )));
        }
        if value.len() > MAX_VALUE_SIZE {
            return Err(AeroLogError::SizeLimitExceeded(format!(
                "Value size {} exceeds maximum allowable {}",
                value.len(),
                MAX_VALUE_SIZE
            )));
        }

        let crc32 = RecordHeader::compute_crc(record_type, &hlc, &key, &value);
        let header = RecordHeader {
            crc32,
            magic: MAGIC_BYTES,
            version: CURRENT_VERSION,
            record_type,
            hlc,
            key_len: key.len() as u16,
            value_len: value.len() as u32,
        };

        Ok(Self { header, key, value })
    }

    /// Serializes the entire record into a contiguous byte buffer.
    pub fn to_bytes(&self) -> Vec<u8> {
        let total_size = self.header.total_record_size() as usize;
        let mut buf = Vec::with_capacity(total_size);
        buf.extend_from_slice(&self.header.encode());
        buf.extend_from_slice(&self.key);
        buf.extend_from_slice(&self.value);
        buf
    }

    /// Verifies that the internal checksum matches the payload bytes.
    pub fn verify_checksum(&self) -> Result<()> {
        let expected = RecordHeader::compute_crc(
            self.header.record_type,
            &self.header.hlc,
            &self.key,
            &self.value,
        );
        if expected != self.header.crc32 {
            return Err(AeroLogError::CrcMismatch {
                expected: self.header.crc32,
                actual: expected,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_header_encode_decode() {
        let hlc = HlcTimestamp::new(1710928300000, 5, 0x123456789ABCDEF0);
        let key = b"users/alice".to_vec();
        let value = b"{\"name\":\"Alice\",\"status\":\"active\"}".to_vec();

        let record = Record::new(RecordType::Put, hlc, key.clone(), value.clone()).unwrap();
        assert_eq!(record.header.magic, MAGIC_BYTES);
        assert_eq!(record.header.version, CURRENT_VERSION);
        assert_eq!(record.header.record_type, RecordType::Put);
        assert_eq!(record.header.key_len, key.len() as u16);
        assert_eq!(record.header.value_len, value.len() as u32);

        let encoded_bytes = record.to_bytes();
        assert_eq!(encoded_bytes.len(), HEADER_SIZE + key.len() + value.len());

        let mut header_buf = [0u8; HEADER_SIZE];
        header_buf.copy_from_slice(&encoded_bytes[0..HEADER_SIZE]);
        let decoded_header = RecordHeader::decode(&header_buf).unwrap();

        assert_eq!(decoded_header, record.header);
        assert!(record.verify_checksum().is_ok());
    }

    #[test]
    fn test_corrupted_payload_fails_checksum() {
        let hlc = HlcTimestamp::new(1000, 0, 1);
        let mut record =
            Record::new(RecordType::Put, hlc, b"key".to_vec(), b"original".to_vec()).unwrap();

        // Tamper with value bytes
        record.value[0] ^= 0xFF;
        assert!(record.verify_checksum().is_err());
    }
}
