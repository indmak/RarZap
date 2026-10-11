//! Library-level errors (thiserror). App layer uses anyhow with context.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("RAR file not found or in use: {0}")]
    SourceUnavailable(PathBuf),

    #[error("RAR archive is corrupt or unreadable: {0}")]
    CorruptArchive(String),

    #[error("cannot write to target directory: {0}")]
    TargetNotWritable(PathBuf),

    #[error("disk full while writing: {0}")]
    DiskFull(PathBuf),

    #[error("filename encoding not recognized in entry: {0}")]
    BadFileName(String),

    #[error("cancelled by user")]
    Cancelled,

    #[error("zip write failed: {0}")]
    ZipWrite(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
