//! On-disk `frames` values.
//!
//! A 0.1.8 value is the CON UTF-8. A single new value is `0xFD 0x01` plus one
//! zstd frame of that same UTF-8. Up to [`GROUP_LEN`] consecutive frames in a
//! trajectory may instead share one `0xFD 0x03` value when that group is
//! shorter than the per-frame values; the other keys store a `0xFD 0x02`
//! redirect. The lead byte is illegal as UTF-8, so a legacy span cannot be
//! read as a tagged value. xxh3-128 is taken on each frame's UTF-8.
//! There is no dictionary. Level 1 is the writer setting; the frame header
//! carries what a decoder needs, and readers do not branch on the level.
//! When a tagged frame is not shorter than the UTF-8, the UTF-8 is stored.

use crate::error::{Error, Result};

/// Lead bytes of a one-frame compressed value. `0xFD` cannot start a UTF-8 span.
pub const FRAME_TAG: [u8; 2] = [0xFD, 0x01];

/// Redirect: `0xFD 0x02`, head frame index, slot in that group.
pub const REDIR_TAG: [u8; 2] = [0xFD, 0x02];

/// Group: `0xFD 0x03`, frame count, one zstd frame of length-prefixed UTF-8.
pub const GROUP_TAG: [u8; 2] = [0xFD, 0x03];

/// Consecutive frames that share one zstd value when the group is smaller.
pub const GROUP_LEN: u32 = 64;

/// Writer level. Readers accept any level the frame header describes.
pub const ZSTD_LEVEL: i32 = 1;

/// Little-endian zstd magic, so a payload can be checked as one zstd frame.
#[cfg(test)]
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

const REDIR_LEN: usize = 2 + 4 + 4;

pub fn is_tagged(stored: &[u8]) -> bool {
    stored.starts_with(&FRAME_TAG)
}

pub fn is_redirect(stored: &[u8]) -> bool {
    stored.starts_with(&REDIR_TAG)
}

pub fn is_group(stored: &[u8]) -> bool {
    stored.starts_with(&GROUP_TAG)
}

/// Bytes to put under one frame key. Call this outside the write transaction.
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

/// Replace per-frame values with a shared zstd group when the group is shorter.
///
/// `frames` is `(frame_idx, text)` in frame order. Runs are aligned to
/// [`GROUP_LEN`] on `frame_idx`, so a suffix of a trajectory packs the same
/// way as the same frames inside a full trajectory. A run that does not shrink
/// stays as per-frame values.
pub fn pack_aligned(frames: &[(u32, &str)]) -> Result<Vec<Vec<u8>>> {
    let mut out = Vec::with_capacity(frames.len());
    let mut i = 0;
    while i < frames.len() {
        let gid = frames[i].0 / GROUP_LEN;
        let mut j = i + 1;
        while j < frames.len() && frames[j].0 / GROUP_LEN == gid {
            j += 1;
        }
        let run = &frames[i..j];
        let mut singles = Vec::with_capacity(run.len());
        let mut single_sum = 0usize;
        for (_, text) in run {
            let one = store_frame(text, true)?;
            single_sum = single_sum.saturating_add(one.len());
            singles.push(one);
        }
        if run.len() > 1 {
            let grouped = encode_group(run)?;
            let total = grouped
                .len()
                .saturating_add(REDIR_LEN.saturating_mul(run.len() - 1));
            if total < single_sum {
                let head = run[0].0;
                out.push(grouped);
                for slot in 1..run.len() {
                    out.push(encode_redirect(head, slot as u32));
                }
                i = j;
                continue;
            }
        }
        out.extend(singles);
        i = j;
    }
    Ok(out)
}

/// CON UTF-8 for a stored value. Tagged bytes decompress once. Untagged
/// bytes must already be UTF-8. A group or a redirect is not a single frame;
/// [`decode_frame`] follows those.
pub fn load_frame(stored: &[u8]) -> Result<String> {
    if is_group(stored) || is_redirect(stored) {
        return Err(Error::Message(
            "corrupt frame value: grouped value needs decode_frame".into(),
        ));
    }
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

/// One frame's UTF-8. `fetch_head` loads the head value of a redirect.
pub fn decode_frame(
    stored: &[u8],
    fetch_head: &mut dyn FnMut(u32) -> Result<Vec<u8>>,
) -> Result<String> {
    if is_redirect(stored) {
        let (head, slot) = parse_redirect(stored)?;
        let head_bytes = fetch_head(head)?;
        if is_redirect(&head_bytes) {
            return Err(Error::Message("corrupt frame value".into()));
        }
        let texts = open_group_or_single(&head_bytes)?;
        return texts
            .into_iter()
            .nth(slot as usize)
            .ok_or_else(|| Error::Message("corrupt frame value".into()));
    }
    let texts = open_group_or_single(stored)?;
    texts
        .into_iter()
        .next()
        .ok_or_else(|| Error::Message("corrupt frame value".into()))
}

fn open_group_or_single(stored: &[u8]) -> Result<Vec<String>> {
    if is_group(stored) {
        decode_group(stored)
    } else {
        Ok(vec![load_frame(stored)?])
    }
}

fn encode_group(run: &[(u32, &str)]) -> Result<Vec<u8>> {
    let mut payload = Vec::new();
    for (_, text) in run {
        let b = text.as_bytes();
        if b.len() > u32::MAX as usize {
            return Err(Error::Message("frame longer than u32".into()));
        }
        payload.extend_from_slice(&(b.len() as u32).to_le_bytes());
        payload.extend_from_slice(b);
    }
    let compressed = zstd::stream::encode_all(&payload[..], ZSTD_LEVEL)
        .map_err(|e| Error::Message(format!("zstd encode: {e}")))?;
    let mut out = Vec::with_capacity(6 + compressed.len());
    out.extend_from_slice(&GROUP_TAG);
    out.extend_from_slice(&(run.len() as u32).to_le_bytes());
    out.extend_from_slice(&compressed);
    Ok(out)
}

fn encode_redirect(head: u32, slot: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(REDIR_LEN);
    out.extend_from_slice(&REDIR_TAG);
    out.extend_from_slice(&head.to_le_bytes());
    out.extend_from_slice(&slot.to_le_bytes());
    out
}

fn parse_redirect(stored: &[u8]) -> Result<(u32, u32)> {
    if stored.len() != REDIR_LEN {
        return Err(Error::Message("corrupt frame value".into()));
    }
    let head = u32::from_le_bytes(stored[2..6].try_into().unwrap());
    let slot = u32::from_le_bytes(stored[6..10].try_into().unwrap());
    if slot == 0 {
        return Err(Error::Message("corrupt frame value".into()));
    }
    Ok((head, slot))
}

fn decode_group(stored: &[u8]) -> Result<Vec<String>> {
    if stored.len() < 6 + 4 {
        return Err(Error::Message("corrupt frame value".into()));
    }
    let n = u32::from_le_bytes(stored[2..6].try_into().unwrap()) as usize;
    if n < 2 {
        return Err(Error::Message("corrupt frame value".into()));
    }
    let plain = zstd::stream::decode_all(&stored[6..])
        .map_err(|e| Error::Message(format!("corrupt frame value: {e}")))?;
    let mut out = Vec::with_capacity(n);
    let mut i = 0;
    while i < plain.len() {
        if plain.len() - i < 4 {
            return Err(Error::Message("corrupt frame value".into()));
        }
        let len = u32::from_le_bytes(plain[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        if plain.len() - i < len {
            return Err(Error::Message("corrupt frame value".into()));
        }
        let text = std::str::from_utf8(&plain[i..i + len])
            .map_err(|_| Error::Message("corrupt frame value".into()))?;
        out.push(text.to_owned());
        i += len;
    }
    if out.len() != n || i != plain.len() {
        return Err(Error::Message("corrupt frame value".into()));
    }
    Ok(out)
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

    #[test]
    fn aligned_group_is_smaller_and_a_second_pack_matches() {
        let text = "Cu 1.0 2.0 3.0 fixed 1\n".repeat(8);
        let frames: Vec<(u32, String)> = (0..GROUP_LEN)
            .map(|i| (i, format!("{text}{i}\n")))
            .collect();
        let view: Vec<(u32, &str)> = frames.iter().map(|(i, s)| (*i, s.as_str())).collect();
        let packed = pack_aligned(&view).unwrap();
        assert!(is_group(&packed[0]), "{:?}", &packed[0][..6]);
        assert!(packed[0][6..].starts_with(&ZSTD_MAGIC));
        let singles: usize = view
            .iter()
            .map(|(_, t)| store_frame(t, true).unwrap().len())
            .sum();
        let grouped: usize = packed.iter().map(|s| s.len()).sum();
        assert!(grouped < singles, "{grouped} vs {singles}");
        let mut heads = std::collections::HashMap::new();
        heads.insert(0u32, packed[0].clone());
        for (slot, stored) in packed.iter().enumerate() {
            let text = decode_frame(stored, &mut |head| {
                heads
                    .get(&head)
                    .cloned()
                    .ok_or_else(|| Error::Message("missing head".into()))
            })
            .unwrap();
            assert_eq!(text, frames[slot].1);
            assert!(!is_redirect(stored) || stored.len() == REDIR_LEN);
        }
        let again = pack_aligned(&view).unwrap();
        assert_eq!(again, packed);
    }
}
