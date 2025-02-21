#[cfg(feature = "regex_support")]
use regex::Error as RErr;
use std::fmt::{Display, Formatter};
use std::io;

#[derive(Debug)]
/// Errors that can occur when using `TempDir` or `TempFile`.
pub enum TempError {
    /// Occurs when attempting access a `None` file--one which was already closed.
    FileIsNone,
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
            Self::IO(e) => write!(f, "IO error: {e}"),
            #[cfg(feature = "regex_support")]
            Self::Regex(e) => write!(f, "Regex error: {e}"),
        }
    }
}

impl std::error::Error for TempError {}

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
