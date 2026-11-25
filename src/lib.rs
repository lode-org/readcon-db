//! Embedded CON/convel corpus on LMDB (Heed) with non-SQL selection and xxHash3 exact match.
//!
//! Optional `python` feature builds PyO3 bindings. The optional `capi` feature
//! compiles the C ABI (`src/ffi.rs`) into the `cdylib`/`staticlib`. Without it no
//! unmangled `rkrdb_*` symbols are exported.
//!
//! The usage guide for creating a corpus, streaming appends, and `StoredFrame`
//! reads lives in `docs/orgmode/howto/streaming.org`.
#![deny(missing_docs)]

// Pulls mimalloc into this crate so local_dynamic_tls is unified onto
// the global allocator installed by readcon-core.
#[allow(unused_imports)]
use mimalloc as _mimalloc_local_dynamic_tls;

pub mod archive;
mod canonical;
mod catalog;
mod cooked_soa;
mod corpus;
mod error;
mod export_h5md;
mod export_xyz;
mod frame_codec;
mod frame_scalars;
mod keys;
#[cfg(feature = "mpi")]
pub mod mpi_ingest;
#[cfg(feature = "mpi")]
pub mod mpi_read;
#[cfg(any(feature = "mpi", test))]
mod mpi_wire;
mod select;
mod shard;
mod topology;
mod units_canon;

pub use archive::ObservationArchive;
pub use canonical::{
    atom_order_permutation, canonical_identity, AtomOrderHit, CanonicalIdentity,
    CANONICAL_IDENTITY_VERSION,
};
pub use catalog::{
    CatalogRecord, ClaimStatus, Clock, SystemClock, MAX_CATALOG_COMPONENT_BYTES,
    MAX_CATALOG_RECORD_BYTES,
};
pub use cooked_soa::CookedSoa;
pub use corpus::{frame_fmax, ConCorpus, TrajMeta};
pub use error::{ClaimError, Error, Result};
pub use export_h5md::{H5mdArrays, H5mdCollectOpts};
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
pub use units_canon::{canonicalize_unit, canonicalize_units_object, NATURAL_TIME_UNIT};

#[cfg(feature = "capi")]
pub mod ffi;

#[cfg(feature = "python")]
mod python;
