#[cfg(feature = "rand_gen")]
use crate::global_consts::{NUM_RETRY, RAND_FN_LEN, VALID_CHARS};
#[cfg(feature = "rand_gen")]
use rand::Rng;
#[cfg(feature = "regex_support")]
use regex::{Error as RErr, Regex};
#[cfg(feature = "rand_gen")]
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::TempResult;
use crate::temp_file::TempFile;

/// A temporary directory that automatically cleans up its contents when dropped.
///
/// Files created through the `TempDir` are tracked and removed upon drop.
/// 
/// # ***IMPORTANT***:
/// 
pub struct TempDir {
    /// The full path to the temporary directory.
    path: Option<PathBuf>,
    /// Temporary files contained within the directory.
    files: Vec<TempFile>,
}

impl TempDir {
    /// Creates a new temporary directory at the specified path.
    ///
    /// The directory (and any missing parent directories) will be created.
    ///
    /// # Arguments
    ///
    /// * `path` - The path at which to create the directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub fn new<P: AsRef<Path>>(path: P) -> TempResult<Self> {
        let path_buf = path.as_ref().to_owned();
        fs::create_dir_all(&path_buf)?;
        Ok(Self {
            path: Some(path_buf),
            files: Vec::new(),
        })
    }

    #[cfg(feature = "rand_gen")]
    /// Creates a new temporary directory with a random name in the given parent directory.
    ///
    /// The directory name will consist of alphanumeric characters only, ensuring compatibility
    /// across different filesystems.
    ///
    /// # Arguments
    ///
    /// * `dir` - An optional parent directory in which to create the temporary directory.
    ///
    /// # Errors
    ///
    /// Returns an error if a unique directory name cannot be generated or if directory creation fails.
    pub fn random<P: AsRef<Path>>(dir: Option<P>) -> TempResult<Self> {
        let parent_dir = dir.map_or(env::temp_dir(), |d| d.as_ref().to_path_buf());
        let mut rng = rand::rng();

        for _ in 0..NUM_RETRY {
            let name: String = (0..RAND_FN_LEN)
                .map(|_| {
                    let idx = rng.random_range(0..VALID_CHARS.len());
                    VALID_CHARS[idx] as char
                })
                .collect();

            let full_path = parent_dir.join(&name);
            if !full_path.exists() {
                fs::create_dir(&full_path)?;
                return Ok(Self {
                    path: Some(full_path),
                    files: Vec::new(),
                });
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Could not generate a unique directory name",
        )
        .into())
    }

    /// Creates a new temporary file with the given filename in the directory.
    ///
    /// The created file is tracked and will be automatically deleted on drop.
    ///
    /// # Arguments
    ///
    /// * `filename` - The name of the file to create.
    ///
    /// # Errors
    ///
    /// This function will return an error if the inner path is `None`.
    #[allow(clippy::missing_panics_doc)]
    pub fn create_file<S: AsRef<str>>(&mut self, filename: S) -> TempResult<&mut TempFile> {
        let dir = self.path.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::Other, "Temporary directory path is not set")
        })?;
        let file_path = dir.join(filename.as_ref());
        self.files.push(TempFile::new(file_path)?);
        Ok(self.files.last_mut().unwrap())
    }

    #[cfg(feature = "rand_gen")]
    /// Creates a new temporary file with a random name in the directory.
    ///
    /// The file is tracked and will be automatically deleted on drop.
    ///
    /// # Errors
    ///
    /// Returns an error if a unique filename cannot be generated or if file creation fails.
    #[allow(clippy::missing_panics_doc)]
    pub fn create_random_file(&mut self) -> TempResult<&mut TempFile> {
        let dir = self.path.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::Other, "Temporary directory path is not set")
        })?;
        self.files.push(TempFile::new_random(Some(dir))?);
        Ok(self.files.last_mut().unwrap())
    }

    /// Removes a file from the directory's management.
    ///
    /// This does not delete the file immediately—it will be removed when the directory is dropped.
    ///
    /// # Arguments
    ///
    /// * `filename` - The name of the file to remove from management.
    pub fn remove_file<S: AsRef<str>>(&mut self, filename: S) {
        let filename = filename.as_ref();
        self.files.retain(|f| {
            f.path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                != Some(filename)
        });
    }

    /// Retrieves a reference to a temporary file by filename.
    ///
    /// # Arguments
    ///
    /// * `filename` - The name of the file to retrieve.
    pub fn get_file<S: AsRef<str>>(&self, filename: S) -> Option<&TempFile> {
        let filename = filename.as_ref();
        self.files.iter().find(|f| {
            f.path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                == Some(filename)
        })
    }

    /// Retrieves a mutable reference to a temporary file by filename.
    ///
    /// # Arguments
    ///
    /// * `filename` - The name of the file to retrieve.
    pub fn get_file_mut<S: AsRef<str>>(&mut self, filename: S) -> Option<&mut TempFile> {
        let filename = filename.as_ref();
        self.files.iter_mut().find(|f| {
            f.path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                == Some(filename)
        })
    }

    /// Returns the path of the temporary directory.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Consumes the `TempDir`, returning its path and preventing cleanup.
    #[must_use]
    pub fn into_path(mut self) -> Option<PathBuf> {
        self.path.take()
    }

    /// Lists the paths of all files managed by the directory.
    #[must_use]
    pub fn list_files(&self) -> Vec<&Path> {
        self.files
            .iter()
            .filter_map(|f| f.path.as_deref())
            .collect()
    }

    #[cfg(feature = "rand_gen")]
    /// Creates a new `TempDir` in the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if a unique directory name cannot be generated or if directory creation fails.
    pub fn new_in<P: AsRef<Path>>(path: P) -> TempResult<Self> {
        Self::random(Some(path))
    }
}

#[cfg(feature = "regex_support")]
impl TempDir {
    /// Finds files with names matching a regex pattern.
    ///
    /// # Arguments
    ///
    /// * `pattern` - A regex pattern to match file names.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    pub fn find_files_by_pattern<S: AsRef<str>>(&self, pattern: S) -> Result<Vec<&TempFile>, RErr> {
        let re = Regex::new(pattern.as_ref())?;
        Ok(self
            .files
            .iter()
            .filter(|f| {
                f.path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| re.is_match(name))
            })
            .collect())
    }

    /// Finds mutable references to files with names matching a regex pattern.
    ///
    /// # Arguments
    ///
    /// * `pattern` - A regex pattern to match file names.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    pub fn find_files_by_pattern_mut<S: AsRef<str>>(
        &mut self,
        pattern: S,
    ) -> Result<Vec<&mut TempFile>, RErr> {
        let re = Regex::new(pattern.as_ref())?;
        Ok(self
            .files
            .iter_mut()
            .filter(|f| {
                f.path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| re.is_match(name))
            })
            .collect())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        self.files.clear();
        if let Some(ref path) = self.path {
            let _ = fs::remove_dir_all(path);
        }
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        self.path.as_ref().expect("TempDir path is None")
    }
}
