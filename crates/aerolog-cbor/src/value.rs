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

use std::fmt;

/// Intermediate AST representation of a CBOR data item (RFC 8949).
#[derive(Debug, Clone, PartialEq)]
pub enum CborValue {
    /// Null value (Major 7, value 22).
    Null,
    /// Boolean value (Major 7, value 20 or 21).
    Bool(bool),
    /// 64-bit signed integer.
    Integer(i64),
    /// Byte string (Major 2).
    Bytes(Vec<u8>),
    /// UTF-8 text string (Major 3).
    Text(String),
    /// Array of items (Major 4).
    Array(Vec<CborValue>),
    /// Map of key-value item pairs (Major 5).
    Map(Vec<(CborValue, CborValue)>),
}

impl CborValue {
    /// Returns the text slice if this value is `CborValue::Text`.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Returns byte slice if this value is `CborValue::Bytes`.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Returns integer if this value is `CborValue::Integer`.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Returns boolean if this value is `CborValue::Bool`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Looks up a value in a map using a string key.
    pub fn get_field(&self, key: &str) -> Option<&CborValue> {
        match self {
            Self::Map(entries) => {
                for (k, v) in entries {
                    if let Self::Text(k_str) = k {
                        if k_str == key {
                            return Some(v);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
}

impl fmt::Display for CborValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Integer(i) => write!(f, "{i}"),
            Self::Bytes(b) => write!(f, "h'{}'", hex::encode(b)),
            Self::Text(s) => write!(f, "\"{s}\""),
            Self::Array(items) => {
                write!(f, "[")?;
                for (idx, item) in items.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Self::Map(pairs) => {
                write!(f, "{{")?;
                for (idx, (k, v)) in pairs.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter().map(|b| format!("{b:02x}")).collect()
    }
}
