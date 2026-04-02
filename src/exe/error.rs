use std::path::PathBuf;

use thiserror::Error;

use crate::core::thumbnail::ThumbnailError;

#[derive(Debug, Error)]
pub enum ExeThumbError {
    #[error("Unsupported platform for .exe extraction")]
    UnsupportedPlatform,
    #[error("Invalid PE format for '{path}'")]
    InvalidPeFormat { path: PathBuf },
    #[error("No icon resource found in executable '{path}'")]
    NoIconResource { path: PathBuf },
    #[error("Failed to decode icon resource for '{path}': {reason}")]
    DecodeFailed { path: PathBuf, reason: String },
    #[error("I/O error for '{path}': {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Permission denied for '{path}': {source}")]
    PermissionDenied {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Resource limit exceeded while processing '{path}'")]
    ResourceLimitExceeded { path: PathBuf },
    #[error("No icon available for executable '{path}'")]
    NoIconAvailable { path: PathBuf },
    #[error(transparent)]
    Thumbnail(#[from] ThumbnailError),
}
