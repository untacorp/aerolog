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

/// Encodes a [`CborValue`] into standard compact binary representation (RFC 8949).
pub fn encode(value: &CborValue) -> Vec<u8> {
    let mut buf = Vec::new();
    encode_into(value, &mut buf);
    buf
}

fn write_type_and_num(major: u8, val: u64, buf: &mut Vec<u8>) {
    let major_shift = major << 5;
    if val < 24 {
        buf.push(major_shift | (val as u8));
    } else if val <= u8::MAX as u64 {
        buf.push(major_shift | 24);
        buf.push(val as u8);
    } else if val <= u16::MAX as u64 {
        buf.push(major_shift | 25);
        let mut b = [0u8; 2];
        BigEndian::write_u16(&mut b, val as u16);
        buf.extend_from_slice(&b);
    } else if val <= u32::MAX as u64 {
        buf.push(major_shift | 26);
        let mut b = [0u8; 4];
        BigEndian::write_u32(&mut b, val as u32);
        buf.extend_from_slice(&b);
    } else {
        buf.push(major_shift | 27);
        let mut b = [0u8; 8];
        BigEndian::write_u64(&mut b, val);
        buf.extend_from_slice(&b);
    }
}

fn encode_into(value: &CborValue, buf: &mut Vec<u8>) {
    match value {
        CborValue::Null => {
            buf.push((7 << 5) | 22);
        }
        CborValue::Bool(b) => {
            buf.push((7 << 5) | if *b { 21 } else { 20 });
        }
        CborValue::Integer(i) => {
            if *i >= 0 {
                write_type_and_num(0, *i as u64, buf);
            } else {
                let neg = (-1 - *i) as u64;
                write_type_and_num(1, neg, buf);
            }
        }
        CborValue::Bytes(bytes) => {
            write_type_and_num(2, bytes.len() as u64, buf);
            buf.extend_from_slice(bytes);
        }
        CborValue::Text(text) => {
            let bytes = text.as_bytes();
            write_type_and_num(3, bytes.len() as u64, buf);
            buf.extend_from_slice(bytes);
        }
        CborValue::Array(items) => {
            write_type_and_num(4, items.len() as u64, buf);
            for item in items {
                encode_into(item, buf);
            }
        }
        CborValue::Map(pairs) => {
            write_type_and_num(5, pairs.len() as u64, buf);
            for (k, v) in pairs {
                encode_into(k, buf);
                encode_into(v, buf);
            }
        }
    }
}
