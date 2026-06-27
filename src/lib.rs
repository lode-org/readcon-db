//! Embedded CON/convel corpus on LMDB (Heed) with non-SQL selection and xxHash3 exact match.
//!
//! Optional `python` feature builds PyO3 bindings. C ABI is always compiled into
//! `cdylib`/`staticlib` (`src/ffi.rs`).

mod corpus;
mod error;
mod export_xyz;
mod keys;
mod select;

pub use corpus::{frame_fmax, ConCorpus};
pub use error::{Error, Result};
pub use export_xyz::{write_frame_extxyz, write_frames_extxyz};
pub use keys::{
    composition_formula, hash_frame_bytes, species_counts_from_symbols, ContentHash, FrameIdx,
    FrameKey, TrajId,
};
pub use select::Select;

pub mod ffi;

#[cfg(feature = "python")]
mod python;
