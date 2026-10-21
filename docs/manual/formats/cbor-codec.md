# Zero-Dependency CBOR Codec

The `aerolog-cbor` crate provides a lightweight, pure-Rust implementation of the **RFC 8949 Concise Binary Object Representation (CBOR)** standard.

---

## Why CBOR for Embedded Storage?

JSON is human-readable but introduces significant storage and CPU overhead:
* Numeric values require decimal string conversion (e.g. `12345678` takes 8 bytes in ASCII vs 4 bytes in binary).
* Field names are repeated in every document.
* Parsing JSON requires complex string parsing and dynamic memory allocations.

CBOR encodes structured data directly into dense binary representations, reducing payload sizes by **40% to 70%** and eliminating parsing CPU overhead.

---

## Supported Data Types

`aerolog-cbor` supports full round-trip serialization for:
* Unsigned and Signed Integers (0 to 64-bit precision)
* Text Strings (UTF-8 encoded)
* Byte Strings (Raw binary blobs)
* Arrays (Heterogeneous lists)
* Maps (Key-value dictionaries)
* Booleans and Null

---

## Code Example

```rust
use aerolog_cbor::{CborValue, encode, decode};
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Construct structured data
    let mut map = BTreeMap::new();
    map.insert("sensor_id".to_string(), CborValue::Text("temp_01".to_string()));
    map.insert("reading".to_string(), CborValue::Integer(24));
    map.insert("active".to_string(), CborValue::Boolean(true));

    let root = CborValue::Map(map);

    // Encode to binary buffer
    let binary_bytes = encode(&root)?;

    // Decode back from binary buffer
    let decoded_val = decode(&binary_bytes)?;
    assert_eq!(root, decoded_val);

    Ok(())
}
```
