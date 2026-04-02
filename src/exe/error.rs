use std::path::PathBuf;

use thiserror::Error;

use crate::core::thumbnail::ThumbnailError;

#[derive(Debug, Error)]
pub enum ExeThumbError {
    #[error("Unsupported platform for .exe extraction")]
    UnsupportedPlatform,
    #[error("No icon available for executable '{path}'")]
    NoIconAvailable { path: PathBuf },
    #[error("I/O error for '{path}': {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Thumbnail(#[from] ThumbnailError),
}
