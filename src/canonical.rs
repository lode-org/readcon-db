//! Opt-in identity that ignores atom row order.
//!
//! The content hash is xxHash3-128 of the stored CON bytes, so the same
//! atoms written in a different order are a different key. Version 1 of
//! this identity reorders rows into increasing `atom_id` and hashes that
//! text instead. The stored frame and `hash_by_frame` are left alone.
//!
//! On disk, when a corpus has been indexed:
//!
//! * `canon_by_frame`: `FrameKey` → version byte ‖ canonical hash ‖ content hash
//! * `idx_canon`: version byte ‖ canonical hash ‖ `FrameKey` → ()
//!
//! The version byte is [`CANONICAL_IDENTITY_VERSION`]. A readonly open of a
//! corpus that has never had these databases has no canonical index.

use std::io::Cursor;

use readcon_core::correspondence::{
    atom_id_permutation, check_atom_id_consistency, reorder_like, CorrespondenceError,
    IdentityCheck,
};
use readcon_core::iterators::ConFrameIterator;
use readcon_core::storage_dtype::FloatArray1;
use readcon_core::types::ConFrame;
use readcon_core::writer::ConFrameWriter;

use crate::error::{Error, Result};
use crate::keys::{hash_frame_bytes, ContentHash, FrameKey};

/// Canonical identity version stored in `idx_canon` and `canon_by_frame`.
///
/// Version 1 reorders atoms into increasing `atom_id` with
/// [`reorder_like`](https://docs.rs/readcon-core/0.22.0/readcon_core/correspondence/fn.reorder_like.html)
/// under the strict symbol, mass, and fixed-mask check, then takes
/// xxHash3-128 of the canonical CON writer bytes
/// (`FloatFormat::RoundTrip`, metadata keys sorted).
pub const CANONICAL_IDENTITY_VERSION: u8 = 1;

/// Versioned xxHash3-128 of a frame in atom-id order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CanonicalIdentity {
    /// [`CANONICAL_IDENTITY_VERSION`] that produced [`Self::hash`].
    pub version: u8,
    /// xxHash3-128 of the canonical CON text, little-endian.
    pub hash: ContentHash,
}

/// One stored frame that matches a query up to atom-id row order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomOrderHit {
    /// Zero-based index of the query frame inside the text or file.
    pub query_index: u32,
    /// Stored frame.
    pub key: FrameKey,
    /// `permutation[i]` is the index in the query of the atom whose
    /// `atom_id` equals stored atom `i`.
    pub permutation: Vec<usize>,
}

/// xxHash3-128 of `frame` rewritten into increasing `atom_id` order.
///
/// A duplicate `atom_id` is an error. The frame is not stored.
pub fn canonical_identity(frame: &ConFrame) -> Result<CanonicalIdentity> {
    let ordered = canonical_frame(frame)?;
    let bytes = canonical_con_bytes(&ordered)?;
    Ok(CanonicalIdentity {
        version: CANONICAL_IDENTITY_VERSION,
        hash: hash_frame_bytes(&bytes),
    })
}

/// `permutation[i]` is the index in `other` of the atom whose `atom_id`
/// equals `reference.atom_data[i].atom_id`.
///
/// Duplicate ids, an id present on only one side, and a symbol, mass, or
/// fixed-mask mismatch are errors. The message names the `atom_id`.
pub fn atom_order_permutation(reference: &ConFrame, other: &ConFrame) -> Result<Vec<usize>> {
    atom_id_permutation(reference, other, IdentityCheck::Strict).map_err(correspondence_error)
}

/// One indexed frame: the key, the canonical version, and both hashes.
pub(crate) struct IndexedIdentity {
    /// Stored frame.
    pub key: FrameKey,
    /// [`CANONICAL_IDENTITY_VERSION`] written with the record.
    pub version: u8,
    /// Canonical xxHash3-128.
    pub hash: [u8; 16],
    /// Content hash of the stored CON text at index time.
    pub content: [u8; 16],
}

pub(crate) fn parse_con_frames(text: &str) -> Result<Vec<ConFrame>> {
    let mut frames = Vec::new();
    for item in ConFrameIterator::new(text) {
        frames.push(item.map_err(|e| Error::Parse(e.to_string()))?);
    }
    if frames.is_empty() {
        return Err(Error::Parse("no frame".into()));
    }
    Ok(frames)
}

fn canonical_frame(frame: &ConFrame) -> Result<ConFrame> {
    check_atom_id_consistency(std::slice::from_ref(frame), IdentityCheck::Strict)
        .map_err(correspondence_error)?;
    let n = frame.atom_data.len();
    if frame.masses.len() != n {
        return Err(Error::Message(format!(
            "masses has {} rows for {n} atoms",
            frame.masses.len()
        )));
    }
    let reference = reference_in_atom_id_order(frame)?;
    reorder_like(&reference, frame, IdentityCheck::Strict).map_err(correspondence_error)
}

fn reference_in_atom_id_order(frame: &ConFrame) -> Result<ConFrame> {
    let n = frame.atom_data.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| frame.atom_data[i].atom_id);
    let mut reference = frame.clone();
    reference.atom_data = order.iter().map(|&i| frame.atom_data[i].clone()).collect();
    let mut masses = FloatArray1::zeros_f64(n);
    for (slot, &i) in order.iter().enumerate() {
        masses.set_f64(slot, frame.masses.get_f64(i));
    }
    reference.masses = masses;
    Ok(reference)
}

fn canonical_con_bytes(frame: &ConFrame) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut writer = ConFrameWriter::new(&mut buf).canonical(true);
        writer
            .write_frame(frame)
            .map_err(|e| Error::Message(format!("canonical CON: {e}")))?;
        writer
            .flush()
            .map_err(|e| Error::Message(format!("canonical CON: {e}")))?;
    }
    Ok(buf.into_inner())
}

fn correspondence_error(err: CorrespondenceError) -> Error {
    Error::Message(err.to_string())
}

pub(crate) fn canon_record(version: u8, canon: &[u8; 16], content: &[u8; 16]) -> [u8; 33] {
    let mut out = [0u8; 33];
    out[0] = version;
    out[1..17].copy_from_slice(canon);
    out[17..].copy_from_slice(content);
    out
}

pub(crate) fn parse_canon_record(bytes: &[u8]) -> Option<(u8, [u8; 16], [u8; 16])> {
    if bytes.len() != 33 {
        return None;
    }
    let mut canon = [0u8; 16];
    let mut content = [0u8; 16];
    canon.copy_from_slice(&bytes[1..17]);
    content.copy_from_slice(&bytes[17..]);
    Some((bytes[0], canon, content))
}

pub(crate) fn canon_index_key(version: u8, hash: &[u8; 16], fk: FrameKey) -> [u8; 29] {
    let mut out = [0u8; 29];
    out[0] = version;
    out[1..17].copy_from_slice(hash);
    out[17..].copy_from_slice(&fk.to_bytes());
    out
}

pub(crate) fn canon_index_prefix(version: u8, hash: &[u8; 16]) -> [u8; 17] {
    let mut out = [0u8; 17];
    out[0] = version;
    out[1..].copy_from_slice(hash);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_key_starts_with_the_version_byte() {
        let hash = [0xab; 16];
        let fk = FrameKey {
            traj_id: 4,
            frame_idx: 9,
        };
        let key = canon_index_key(CANONICAL_IDENTITY_VERSION, &hash, fk);
        assert_eq!(key[0], CANONICAL_IDENTITY_VERSION);
        assert_eq!(&key[1..17], &hash);
        assert_eq!(&key[17..], &fk.to_bytes());
        let record = canon_record(1, &hash, &[0x11; 16]);
        let (version, canon, content) = parse_canon_record(&record).unwrap();
        assert_eq!(version, 1);
        assert_eq!(canon, hash);
        assert_eq!(content, [0x11; 16]);
        assert!(parse_canon_record(&record[..32]).is_none());
    }
}
