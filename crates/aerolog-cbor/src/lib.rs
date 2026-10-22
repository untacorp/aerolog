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

//! # AeroLog CBOR
//!
//! Compact, zero-dependency CBOR (RFC 8949) encoder and decoder designed for AeroLog
//! payloads and low-overhead wireless transport serialization.

pub mod decoder;
pub mod encoder;
pub mod value;

pub use decoder::{decode, CborDecodeError};
pub use encoder::encode;
pub use value::CborValue;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbor_roundtrip_primitives() {
        let values = vec![
            CborValue::Null,
            CborValue::Bool(true),
            CborValue::Bool(false),
            CborValue::Integer(0),
            CborValue::Integer(23),
            CborValue::Integer(24),
            CborValue::Integer(1000),
            CborValue::Integer(-1),
            CborValue::Integer(-500),
            CborValue::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]),
            CborValue::Text("Hello, AeroLog!".to_string()),
        ];

        for val in values {
            let encoded = encode(&val);
            let decoded = decode(&encoded).unwrap();
            assert_eq!(val, decoded);
        }
    }

    #[test]
    fn test_cbor_roundtrip_nested_map_and_array() {
        let map = CborValue::Map(vec![
            (
                CborValue::Text("author".to_string()),
                CborValue::Text("alice".to_string()),
            ),
            (
                CborValue::Text("content".to_string()),
                CborValue::Text("Need medical supplies at Sector 4".to_string()),
            ),
            (
                CborValue::Text("tags".to_string()),
                CborValue::Array(vec![
                    CborValue::Text("urgent".to_string()),
                    CborValue::Text("medical".to_string()),
                ]),
            ),
            (
                CborValue::Text("delivered".to_string()),
                CborValue::Bool(false),
            ),
        ]);

        let encoded = encode(&map);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(map, decoded);

        assert_eq!(
            decoded.get_field("author").unwrap().as_text(),
            Some("alice")
        );
    }

    #[test]
    fn test_cbor_i64_extremes() {
        let values = vec![
            CborValue::Integer(i64::MIN),
            CborValue::Integer(i64::MIN + 1),
            CborValue::Integer(-1),
            CborValue::Integer(0),
            CborValue::Integer(1),
            CborValue::Integer(i64::MAX - 1),
            CborValue::Integer(i64::MAX),
        ];

        for val in values {
            let encoded = encode(&val);
            let decoded = decode(&encoded).unwrap();
            assert_eq!(val, decoded);
        }
    }

    #[test]
    fn test_cbor_recursion_depth_limit() {
        let mut deeply_nested = CborValue::Integer(42);
        for _ in 0..70 {
            deeply_nested = CborValue::Array(vec![deeply_nested]);
        }
        let encoded = encode(&deeply_nested);
        let res = decode(&encoded);
        assert!(matches!(
            res,
            Err(CborDecodeError::RecursionDepthExceeded(_))
        ));
    }

    #[test]
    fn test_cbor_unbounded_length_does_not_oom() {
        let malicious = vec![0x9A, 0x00, 0xFF, 0xFF, 0xFF];
        let res = decode(&malicious);
        assert!(matches!(res, Err(CborDecodeError::UnexpectedEof(_))));
    }
}
