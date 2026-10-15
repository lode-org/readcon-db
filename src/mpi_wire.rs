//! Width checks for MPI counts and for the ingest record prefix.
//!
//! Classic `MPI_Allgatherv` / `MPI_Alltoallv` counts and displacements are
//! signed 32-bit. Casting a `usize` with `as i32` wraps past 2 GiB and the
//! collective then reads the wrong bytes. Lengths that are not themselves
//! MPI counts (a frame's encoded keys, its stored bytes) are `u64`.

use crate::error::{Error, Result};

/// Largest byte count a classic MPI collective accepts.
pub(crate) const MPI_COUNT_MAX: u64 = i32::MAX as u64;

pub(crate) fn mpi_i32(n: u64) -> Result<i32> {
    i32::try_from(n).map_err(|_| Error::Message(format!("MPI count {n} exceeds {MPI_COUNT_MAX}")))
}

/// Displacements for `counts`. Errors when a count is negative or when any
/// displacement, including the total length, does not fit in a non-negative `i32`.
pub(crate) fn prefix_i32(counts: &[i32]) -> Result<Vec<i32>> {
    let mut acc: i32 = 0;
    let mut out = Vec::with_capacity(counts.len());
    for &c in counts {
        if c < 0 {
            return Err(Error::Message(format!("negative MPI count {c}")));
        }
        out.push(acc);
        acc = acc
            .checked_add(c)
            .ok_or_else(|| Error::Message(format!("MPI displacement exceeds {MPI_COUNT_MAX}")))?;
    }
    Ok(out)
}

/// True when every count and their sum fit in a non-negative `i32`.
pub(crate) fn fits_i32(counts: &[u64]) -> bool {
    let mut acc: u64 = 0;
    for &c in counts {
        if c > MPI_COUNT_MAX {
            return false;
        }
        match acc.checked_add(c) {
            Some(sum) if sum <= MPI_COUNT_MAX => acc = sum,
            _ => return false,
        }
    }
    true
}

pub(crate) fn fit_u32(n: usize) -> Result<u32> {
    u32::try_from(n).map_err(|_| Error::Message(format!("{n} exceeds u32::MAX")))
}

/// Frames whose 16-byte hashes fit in one `MPI_Allgatherv`.
pub(crate) fn hash_gather_chunk_frames() -> usize {
    (MPI_COUNT_MAX as usize) / 16
}

/// A duplicate whose winner is in an earlier batch must carry its own bytes.
/// The writer keeps a winner only until the end of the batch that reuses it.
pub(crate) fn send_stored_bytes(winner: usize, frame: usize, batch_start: usize) -> bool {
    winner == frame || winner < batch_start
}

/// Bytes of one gathered record: frame index, encoded keys, stored bytes.
pub(crate) fn record_bytes(enc_len: usize, stored_len: usize, send_blob: bool) -> u64 {
    let blob = if send_blob { stored_len as u64 } else { 0 };
    8 + 8 + enc_len as u64 + 8 + blob
}

pub(crate) fn push_u64(out: &mut Vec<u8>, n: u64) {
    out.extend_from_slice(&n.to_le_bytes());
}

pub(crate) fn push_record(out: &mut Vec<u8>, frame: u64, enc: &[u8], stored: &[u8]) {
    push_u64(out, frame);
    push_u64(out, enc.len() as u64);
    out.extend_from_slice(enc);
    push_u64(out, stored.len() as u64);
    out.extend_from_slice(stored);
}

pub(crate) fn read_u64(buf: &mut &[u8]) -> Result<u64> {
    if buf.len() < 8 {
        return Err(Error::Message("truncated MPI frame record".into()));
    }
    let (head, rest) = buf.split_at(8);
    *buf = rest;
    Ok(u64::from_le_bytes(head.try_into().unwrap()))
}

pub(crate) fn read_bytes<'a>(buf: &mut &'a [u8]) -> Result<&'a [u8]> {
    let n = read_u64(buf)?;
    let n = usize::try_from(n)
        .map_err(|_| Error::Message(format!("record length {n} exceeds usize")))?;
    if buf.len() < n {
        return Err(Error::Message("truncated MPI frame record".into()));
    }
    let (head, rest) = buf.split_at(n);
    *buf = rest;
    Ok(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_i32_rejects_a_sum_past_the_mpi_limit() {
        let half = (MPI_COUNT_MAX / 2 + 1) as i32;
        let err = prefix_i32(&[half, half]).unwrap_err();
        assert!(err.to_string().contains("displacement"), "{err}");
    }

    #[test]
    fn prefix_i32_accepts_exact_i32_max_and_rejects_negative() {
        assert_eq!(prefix_i32(&[i32::MAX]).unwrap(), vec![0]);
        assert_eq!(prefix_i32(&[1, 2, 0]).unwrap(), vec![0, 1, 3]);
        assert!(prefix_i32(&[-1]).is_err());
    }

    #[test]
    fn fits_i32_rejects_a_count_that_would_wrap() {
        let n = MPI_COUNT_MAX + 1;
        assert!(!fits_i32(&[n]));
        assert_ne!(n as i32 as u64, n);
        assert!(fits_i32(&[MPI_COUNT_MAX, 0]));
        assert!(!fits_i32(&[MPI_COUNT_MAX, 1]));
        assert_eq!(mpi_i32(MPI_COUNT_MAX).unwrap(), i32::MAX);
        assert!(mpi_i32(n).is_err());
    }

    #[test]
    fn record_length_fields_are_u64() {
        let mut buf = Vec::new();
        push_record(&mut buf, 7, &[1, 2], &[9]);
        assert_eq!(&buf[8..16], &2u64.to_le_bytes());
        let mut p = &buf[..];
        assert_eq!(read_u64(&mut p).unwrap(), 7);
        assert_eq!(read_bytes(&mut p).unwrap(), &[1, 2]);
        assert_eq!(read_bytes(&mut p).unwrap(), &[9]);
        assert!(p.is_empty());
        let wide = u32::MAX as u64 + 9;
        assert!(wide > u32::MAX as u64);
        let mut prefix = Vec::new();
        push_u64(&mut prefix, wide);
        let mut q = &prefix[..];
        assert_eq!(read_u64(&mut q).unwrap(), wide);
    }

    #[test]
    fn record_bytes_keep_a_length_past_u32() {
        let n = record_bytes(0, 50, true);
        assert_eq!(n, 8 + 8 + 8 + 50);
        #[cfg(target_pointer_width = "64")]
        {
            let wide = record_bytes(0, u32::MAX as usize + 1, true);
            assert!(wide > u32::MAX as u64);
        }
    }

    #[test]
    fn send_stored_bytes_resends_when_the_winner_is_in_an_earlier_batch() {
        assert!(send_stored_bytes(0, 0, 0));
        assert!(!send_stored_bytes(0, 5, 0));
        assert!(send_stored_bytes(0, 5, 4));
        assert!(!send_stored_bytes(3, 5, 2));
    }

    #[test]
    fn fit_u32_and_hash_chunk_stay_inside_their_limits() {
        assert_eq!(fit_u32(u32::MAX as usize).unwrap(), u32::MAX);
        #[cfg(target_pointer_width = "64")]
        assert!(fit_u32(u32::MAX as usize + 1).is_err());
        let chunk = hash_gather_chunk_frames();
        assert!(chunk > 0);
        assert!(chunk.saturating_mul(16) <= MPI_COUNT_MAX as usize);
    }
}
