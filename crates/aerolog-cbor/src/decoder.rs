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

use crate::value::CborValue;
use byteorder::{BigEndian, ByteOrder};
use thiserror::Error;

/// Errors that can occur during CBOR binary decoding.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum CborDecodeError {
    /// Unexpected end of byte buffer.
    #[error("Unexpected end of buffer at offset {0}")]
    UnexpectedEof(usize),
    /// Invalid UTF-8 sequence in text string.
    #[error("Invalid UTF-8 sequence at offset {0}")]
    InvalidUtf8(usize),
    /// Unrecognized simple value or major type code.
    #[error("Unknown or unsupported CBOR type tag: 0x{0:02X}")]
    UnknownTag(u8),
    /// Maximum nesting recursion depth exceeded.
    #[error("Exceeded maximum recursion depth of {0}")]
    RecursionDepthExceeded(usize),
}

/// Maximum recursion depth allowed during CBOR decoding to prevent stack exhaustion.
pub const MAX_DECODE_DEPTH: usize = 64;

/// Maximum initial capacity for pre-allocated vectors to prevent memory exhaustion DoS.
pub const MAX_INITIAL_CAPACITY: usize = 1024;

/// Decodes raw binary CBOR bytes into a [`CborValue`].
pub fn decode(bytes: &[u8]) -> Result<CborValue, CborDecodeError> {
    let (val, _) = decode_one(bytes, 0, 0)?;
    Ok(val)
}

fn read_type_and_num(bytes: &[u8], offset: usize) -> Result<(u8, u64, usize), CborDecodeError> {
    if offset >= bytes.len() {
        return Err(CborDecodeError::UnexpectedEof(offset));
    }

    let initial = bytes[offset];
    let major = initial >> 5;
    let info = initial & 0x1F;

    if info < 24 {
        Ok((major, info as u64, 1))
    } else if info == 24 {
        if offset + 1 >= bytes.len() {
            return Err(CborDecodeError::UnexpectedEof(offset + 1));
        }
        Ok((major, bytes[offset + 1] as u64, 2))
    } else if info == 25 {
        if offset + 2 >= bytes.len() {
            return Err(CborDecodeError::UnexpectedEof(offset + 2));
        }
        let val = BigEndian::read_u16(&bytes[offset + 1..offset + 3]);
        Ok((major, val as u64, 3))
    } else if info == 26 {
        if offset + 4 >= bytes.len() {
            return Err(CborDecodeError::UnexpectedEof(offset + 4));
        }
        let val = BigEndian::read_u32(&bytes[offset + 1..offset + 5]);
        Ok((major, val as u64, 5))
    } else if info == 27 {
        if offset + 8 >= bytes.len() {
            return Err(CborDecodeError::UnexpectedEof(offset + 8));
        }
        let val = BigEndian::read_u64(&bytes[offset + 1..offset + 9]);
        Ok((major, val, 9))
    } else {
        Err(CborDecodeError::UnknownTag(initial))
    }
}

fn decode_one(
    bytes: &[u8],
    offset: usize,
    depth: usize,
) -> Result<(CborValue, usize), CborDecodeError> {
    if depth > MAX_DECODE_DEPTH {
        return Err(CborDecodeError::RecursionDepthExceeded(depth));
    }

    let (major, num, header_len) = read_type_and_num(bytes, offset)?;
    let mut total_len = header_len;
    let data_start = offset + header_len;

    match major {
        0 => Ok((CborValue::Integer(num as i64), total_len)),
        1 => {
            // RFC 8949 major type 1 is -1 - num.
            // In two's complement, -1 - n == !n, avoiding signed overflow on boundary values.
            let val = (!num) as i64;
            Ok((CborValue::Integer(val), total_len))
        }
        2 => {
            let end = match data_start.checked_add(num as usize) {
                Some(e) if e <= bytes.len() => e,
                _ => return Err(CborDecodeError::UnexpectedEof(bytes.len())),
            };
            let slice = bytes[data_start..end].to_vec();
            total_len += num as usize;
            Ok((CborValue::Bytes(slice), total_len))
        }
        3 => {
            let end = match data_start.checked_add(num as usize) {
                Some(e) if e <= bytes.len() => e,
                _ => return Err(CborDecodeError::UnexpectedEof(bytes.len())),
            };
            let s = std::str::from_utf8(&bytes[data_start..end])
                .map_err(|_| CborDecodeError::InvalidUtf8(data_start))?
                .to_string();
            total_len += num as usize;
            Ok((CborValue::Text(s), total_len))
        }
        4 => {
            let cap = (num as usize).min(MAX_INITIAL_CAPACITY);
            let mut items = Vec::with_capacity(cap);
            let mut item_offset = data_start;
            for _ in 0..num {
                let (item, len) = decode_one(bytes, item_offset, depth + 1)?;
                items.push(item);
                item_offset += len;
                total_len += len;
            }
            Ok((CborValue::Array(items), total_len))
        }
        5 => {
            let cap = (num as usize).min(MAX_INITIAL_CAPACITY);
            let mut pairs = Vec::with_capacity(cap);
            let mut item_offset = data_start;
            for _ in 0..num {
                let (k, k_len) = decode_one(bytes, item_offset, depth + 1)?;
                item_offset += k_len;
                total_len += k_len;

                let (v, v_len) = decode_one(bytes, item_offset, depth + 1)?;
                item_offset += v_len;
                total_len += v_len;

                pairs.push((k, v));
            }
            Ok((CborValue::Map(pairs), total_len))
        }
        7 => match num {
            20 => Ok((CborValue::Bool(false), total_len)),
            21 => Ok((CborValue::Bool(true), total_len)),
            22 => Ok((CborValue::Null, total_len)),
            other => Err(CborDecodeError::UnknownTag(other as u8)),
        },
        other => Err(CborDecodeError::UnknownTag(other)),
    }
}
