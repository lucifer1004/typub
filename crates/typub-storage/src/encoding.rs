//! Encoding utilities for asset data.

use std::path::Path;

/// Encode binary data to base64 data URI
pub fn to_data_uri(data: &[u8], path: &Path) -> String {
    let mime = crate::mime_type_from_path(path);
    let b64 = base64_encode(data);
    format!("data:{};base64,{}", mime, b64)
}

/// Simple base64 encoding
pub fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::new();
    for chunk in data.chunks(3) {
        let mut val = 0u32;
        for (i, &byte) in chunk.iter().enumerate() {
            val |= (byte as u32) << (16 - 8 * i);
        }

        let chars = match chunk.len() {
            1 => 2,
            2 => 3,
            _ => 4,
        };

        for i in 0..chars {
            let idx = ((val >> (18 - 6 * i)) & 0x3F) as usize;
            result.push(ALPHABET[idx] as char);
        }

        for _ in chars..4 {
            result.push('=');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn test_base64_encode_empty() {
        assert_eq!(base64_encode(&[]), "");
    }

    #[test]
    fn test_base64_encode_hello() {
        // "Hello" -> "SGVsbG8="
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
    }

    #[test]
    fn test_base64_encode_padding() {
        // "a" -> "YQ=="
        assert_eq!(base64_encode(b"a"), "YQ==");
        // "ab" -> "YWI="
        assert_eq!(base64_encode(b"ab"), "YWI=");
        // "abc" -> "YWJj"
        assert_eq!(base64_encode(b"abc"), "YWJj");
    }

    #[test]
    fn test_to_data_uri() {
        let data = b"test";
        let path = Path::new("image.png");
        let uri = to_data_uri(data, path);
        assert!(uri.starts_with("data:image/png;base64,"));
    }
}
