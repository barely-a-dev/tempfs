//! A library primarily providing an interface to create temporary directories and files.
//!
//! It also provides several features:
//! - `rand_gen` : Support for randomly generated file and directory names.
//! - `mmap_support` : Support for memory mapping temporary files with memmap2.
//! - `regex_support` : Support for searching temporary directory's contained files using regex.
//! - `virt_fs` : Provides a virtual, in-memory filesystem with files, directories, permissions, metadata, and generally mimics a Linux filesystem.
//! `display_files` : Allows Displaying `TempFile` and `VirtFile`.
//! - `full` : Enables all of the above.

/// Errors which can occur when using the types provided by tempfs.
pub mod error;
/// Global constants for the program.
mod global_consts;
/// Module providing temporary directories.
pub mod temp_dir;
/// Module providing temporary files.
pub mod temp_file;
#[cfg(feature = "virt_fs")]
/// Module providing a virtual unix-like filesystem.
pub mod virt_fs;
/// Helpers for `temp_file` and `temp_dir`.
mod helpers;

pub use error::*;
pub use temp_dir::TempDir;
pub use temp_file::TempFile;
#[cfg(feature = "virt_fs")]
pub use virt_fs::*;
