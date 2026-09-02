//! Embedded CON/convel corpus on LMDB (Heed) with non-SQL selection and xxHash3 exact match.
//!
//! Optional `python` feature builds PyO3 bindings. C ABI is always compiled into
//! `cdylib`/`staticlib` (`src/ffi.rs`).

pub mod archive;
mod cooked_soa;
mod corpus;
mod error;
mod export_h5md;
mod export_xyz;
mod frame_scalars;
mod keys;
mod select;
mod shard;
mod topology;
mod units_canon;

pub use archive::ObservationArchive;
pub use cooked_soa::CookedSoa;
pub use corpus::{frame_fmax, ConCorpus, TrajMeta};
pub use error::{Error, Result};
pub use export_h5md::H5mdArrays;
pub use export_xyz::{write_frame_extxyz, write_frames_extxyz};
pub use frame_scalars::{frame_cell_volume, frame_total_mass};
pub use keys::{
    composition_formula, hash_frame_bytes, species_counts_from_symbols, ContentHash, FrameIdx,
    FrameKey, TrajId,
};
pub use select::Select;
pub use shard::{
    join_corpus_dirs, join_drained_roots, open_single_env_for_export, CorpusExportKind,
    ShardManifest, ShardedConCorpus, DEFAULT_N_SHARDS,
};
pub use topology::{
    parse_fingerprint_json_line, parse_fingerprint_json_stdout, parse_fingerprint_text,
    AnnotateTopologyOpts, FingerprintRecord, TopologyParams, SEAMS_MISSING,
};
pub use units_canon::{canonicalize_unit, canonicalize_units_object};

pub mod ffi;

#[cfg(feature = "python")]
mod python;
