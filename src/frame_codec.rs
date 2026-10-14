//! On-disk `frames` values.
//!
//! A 0.1.8 value is the CON UTF-8. A new value is `0xFD 0x01` plus one zstd
//! frame of that same UTF-8. The lead byte is illegal as UTF-8, so a legacy
//! span cannot be read as a tagged value. xxh3-128 is taken on the UTF-8.
//! There is no dictionary. Level 1 is the writer setting; the frame header
//! carries what a decoder needs, and readers do not branch on the level.
//! When the tagged frame is not shorter than the UTF-8, the UTF-8 is stored.

use crate::error::{Error, Result};

/// Lead bytes of a compressed frame value. `0xFD` cannot start a UTF-8 span.
pub const FRAME_TAG: [u8; 2] = [0xFD, 0x01];

/// Writer level. Readers accept any level the frame header describes.
pub const ZSTD_LEVEL: i32 = 1;

/// Little-endian zstd magic, so a payload can be checked as one zstd frame.
#[cfg(test)]
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

pub fn is_tagged(stored: &[u8]) -> bool {
    stored.starts_with(&FRAME_TAG)
}

/// Bytes to put under the frame key. Call this outside the write transaction.
pub fn store_frame(text: &str, compress: bool) -> Result<Vec<u8>> {
    if !compress {
        return Ok(text.as_bytes().to_vec());
    }
    let compressed = zstd::stream::encode_all(text.as_bytes(), ZSTD_LEVEL)
        .map_err(|e| Error::Message(format!("zstd encode: {e}")))?;
    if compressed.len().saturating_add(FRAME_TAG.len()) >= text.len() {
        return Ok(text.as_bytes().to_vec());
    }
    let mut out = Vec::with_capacity(FRAME_TAG.len() + compressed.len());
    out.extend_from_slice(&FRAME_TAG);
    out.extend_from_slice(&compressed);
    Ok(out)
}

/// CON UTF-8 for a stored value. Tagged bytes decompress once. Untagged
/// bytes must already be UTF-8. Anything else is corrupt.
pub fn load_frame(stored: &[u8]) -> Result<String> {
    if is_tagged(stored) {
        let plain = zstd::stream::decode_all(&stored[FRAME_TAG.len()..])
            .map_err(|e| Error::Message(format!("corrupt frame value: {e}")))?;
        String::from_utf8(plain).map_err(|e| Error::Message(format!("corrupt frame value: {e}")))
    } else {
        std::str::from_utf8(stored)
            .map(str::to_owned)
            .map_err(|_| Error::Message("corrupt frame value".into()))
    }
}

/// True when `stored` is one tagged zstd frame whose payload is not itself tagged.
#[cfg(test)]
pub(crate) fn is_single_zstd_frame(stored: &[u8]) -> bool {
    stored.len() > FRAME_TAG.len() + ZSTD_MAGIC.len()
        && is_tagged(stored)
        && stored[FRAME_TAG.len()..].starts_with(&ZSTD_MAGIC)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_input_stays_raw_utf8() {
        let stored = store_frame("x", true).unwrap();
        assert_eq!(stored, b"x");
        assert!(!is_tagged(&stored));
        assert_eq!(load_frame(&stored).unwrap(), "x");
    }

    #[test]
    fn repeated_text_is_one_zstd_frame_and_a_second_store_matches() {
        let text = "Cu 1.0 2.0 3.0\n".repeat(64);
        let once = store_frame(&text, false).unwrap();
        assert_eq!(once, text.as_bytes());
        let tagged = store_frame(&text, true).unwrap();
        assert!(is_single_zstd_frame(&tagged), "{tagged:?}");
        assert!(tagged.len() < text.len());
        let back = load_frame(&tagged).unwrap();
        assert_eq!(back, text);
        assert_eq!(store_frame(&back, true).unwrap(), tagged);
        assert!(std::str::from_utf8(&tagged).is_err());
    }
}
