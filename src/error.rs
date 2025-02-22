use std::error::Error;
#[cfg(feature = "regex_support")]
use regex::Error as RErr;
use std::fmt::{Display, Formatter};
use std::io;

#[derive(Debug)]
/// Errors that can occur when using `TempDir` or `TempFile`.
pub enum TempError {
    /// Occurs when attempting access a `None` file--one which was already closed.
    FileIsNone,
    /// A given file or path do not match.
    InvalidFileOrPath,
    /// An IO error.
    IO(io::Error),
    #[cfg(feature = "regex_support")]
    /// A `RegEx` error.
    Regex(RErr),
}

impl Display for TempError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileIsNone => write!(f, "File is None"),
            Self::InvalidFileOrPath => write!(f, "File or path is invalid"),
            Self::IO(e) => write!(f, "IO error: {e}"),
            #[cfg(feature = "regex_support")]
            Self::Regex(e) => write!(f, "Regex error: {e}"),
        }
    }
}

impl Error for TempError {}

/// Result type which uses a `TempError`
pub type TempResult<T> = Result<T, TempError>;

impl From<io::Error> for TempError {
    fn from(e: io::Error) -> Self {
        Self::IO(e)
    }
}

#[cfg(feature = "regex_support")]
impl From<RErr> for TempError {
    fn from(e: RErr) -> Self {
        Self::Regex(e)
    }
}

/// Error types for virtual filesystem operations.
#[derive(Debug)]
pub enum FsError {
    /// The file or directory was not found.
    NotFound(String),
    /// The file or directory already exists
    AlreadyExists(String),
    /// The path is invalid.
    InvalidPath(String),
}

impl Display for FsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self { 
            Self::NotFound(path) => write!(f, "Could not find file: {path}"),
            Self::AlreadyExists(path) => write!(f, "File already exists: {path}"),
            Self::InvalidPath(path) => write!(f, "Invalid path: {path}"),
        }
    }
}

impl Error for FsError {}
