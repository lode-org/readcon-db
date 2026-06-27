use crate::keys::FrameKey;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("heed: {0}")]
    Heed(#[from] heed::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse frame: {0}")]
    Parse(String),
    #[error("missing frame {0:?}")]
    MissingFrame(FrameKey),
    #[error("trajectory {0} already exists (n_frames={1}); use a new traj_id")]
    TrajExists(u64, u32),
    #[error("utf-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("nul in path or string")]
    Nul,
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, Error>;
