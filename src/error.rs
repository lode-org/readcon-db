use crate::keys::FrameKey;
use thiserror::Error;

#[derive(Error, Debug)]
/// Errors returned by the corpus, the C ABI, and the bindings.
pub enum Error {
    #[error("heed: {0}")]
    /// LMDB rejected the call.
    Heed(#[from] heed::Error),
    #[error("io: {0}")]
    /// A filesystem call failed.
    Io(#[from] std::io::Error),
    #[error("parse frame: {0}")]
    /// CON text could not be parsed.
    Parse(String),
    #[error("missing frame {0:?}")]
    /// The corpus has no frame for this key.
    MissingFrame(FrameKey),
    #[error("trajectory {0} already exists (n_frames={1}); use a new traj_id")]
    /// That trajectory id is already in the corpus.
    TrajExists(u64, u32),
    #[error("utf-8: {0}")]
    /// Stored bytes were not UTF-8.
    Utf8(#[from] std::str::Utf8Error),
    #[error("json: {0}")]
    /// Stored metadata was not the expected JSON.
    Json(#[from] serde_json::Error),
    #[error("nul in path or string")]
    /// A path or string passed to the C ABI contained a NUL byte.
    Nul,
    #[error("{0}")]
    /// A failure that does not fit a more specific variant.
    Message(String),
}

/// Corpus result. The error type is [`enum@Error`].
pub type Result<T> = std::result::Result<T, Error>;
