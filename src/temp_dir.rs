#[cfg(feature = "rand_gen")]
use crate::global_consts::{num_retry, rand_fn_len, valid_chars};
#[cfg(feature = "rand_gen")]
use rand::Rng;
#[cfg(feature = "regex_support")]
use regex::Regex;
use std::env;
use std::fs;
#[cfg(unix)]
use std::fs::Permissions;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::TempResult;
use crate::helpers::normalize_path;
use crate::temp_file::TempFile;

/// A temporary directory that automatically cleans up its contents when dropped.
///
/// Files created through the `TempDir` are tracked and removed upon drop.
#[derive(Debug)]
pub struct TempDir {
    /// The full path to the temporary directory.
    path: Option<PathBuf>,
    /// Temporary files contained within the directory.
    files: Vec<TempFile>,
    /// The first created parent directory of the parent directories.
    created_parent: Option<PathBuf>,
}

impl TempDir {
    /// Creates a new temporary directory at the specified path.
    ///
    /// The directory (and any missing parent directories) will be created.
    ///
    /// # Arguments
    ///
    /// * `path` - The path at which to create the directory. If a relative path is provided, it is resolved relative to the system temporary directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub fn new<P: AsRef<Path>>(path: P) -> TempResult<Self> {
        let path_ref = normalize_path(path.as_ref());
        let path_buf = if path_ref.is_absolute() {
            path_ref
        } else {
            env::temp_dir().join(path_ref)
        };
        let created = Self::create_with_parent(&path_buf)?;
        Ok(Self {
            path: Some(path_buf),
            files: Vec::new(),
            created_parent: created,
        })
    }

    /// Creates a new temporary directory at the specified path.
    ///
    /// The directory (and any missing parent directories) will be created.
    ///
    /// # Arguments
    ///
    /// * `path` - The path at which to create the directory. If a relative path is provided, it is resolved relative to the current directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub fn new_here<P: AsRef<Path>>(path: P) -> TempResult<Self> {
        let path_ref = normalize_path(path.as_ref());
        let path_buf = if path_ref.is_absolute() {
            path_ref
        } else {
            env::current_dir()?.join(path_ref)
        };
        Self::new(path_buf)
    }

    #[cfg(feature = "rand_gen")]
    /// Creates a new temporary directory with a random name in the given parent directory.
    ///
    /// The directory name will consist of alphanumeric characters only, ensuring compatibility
    /// across different filesystems.
    ///
    /// # Arguments
    ///
    /// * `dir` - An optional parent directory in which to create the temporary directory. If a relative directory is provided, it is resolved relative to the system temporary directory.
    ///
    /// # Errors
    ///
    /// Returns an error if a unique directory name cannot be generated or if directory creation fails.
    pub fn new_random<P: AsRef<Path>>(dir: Option<P>) -> TempResult<Self> {
        let parent_dir = if let Some(d) = dir {
            let d_ref = normalize_path(d.as_ref());
            if d_ref.is_absolute() {
                d_ref
            } else {
                env::temp_dir().join(d_ref)
            }
        } else {
            env::temp_dir()
        };
        let mut rng = rand::rng();

        for _ in 0..num_retry() {
            let name: String = (0..rand_fn_len())
                .map(|_| {
                    let idx = rng.random_range(0..valid_chars().len());
                    valid_chars()[idx] as char
                })
                .collect();

            let full_path = parent_dir.join(&name);
            if !full_path.exists() {
                let created = Self::create_with_parent(&full_path)?;
                return Ok(Self {
                    path: Some(full_path),
                    files: Vec::new(),
                    created_parent: created,
                });
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Could not generate a unique directory name",
        )
        .into())
    }

    /// Function to create the directory and its parent directories, then set their permissions to rwx------, returning the first component of the parent's path which does not exist, or None if it all exists except for the child.
    fn create_with_parent(path: &PathBuf) -> TempResult<Option<PathBuf>> {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;
        let nonexistent = crate::helpers::first_missing_directory_component(path);
        fs::create_dir_all(path)?;

        #[cfg(unix)]
        if let Some(first_missing) = nonexistent.clone() {
            let mut current = first_missing;
            // Loop until the final directory in the path is reached.
            while current != *path {
                fs::set_permissions(&current, Permissions::from_mode(0o700))?;
                // Append the next path component.
                if let Some(component) = path.strip_prefix(&current).unwrap().components().next() {
                    current = current.join(component);
                } else {
                    break;
                }
            }
            // Finally, set permissions on the final directory.
            fs::set_permissions(path, Permissions::from_mode(0o700))?;
        } else {
            // If no directory was missing (only the child directory was created)
            fs::set_permissions(path, Permissions::from_mode(0o700))?;
        }

        Ok(nonexistent)
    }

    /// Creates a new temporary directory with a random name in the given parent directory.
    ///
    /// The directory name will consist of alphanumeric characters only, ensuring compatibility
    /// across different filesystems.
    ///
    /// # Arguments
    ///
    /// * `dir` - An optional parent directory in which to create the temporary directory. If a relative directory is provided, it is resolved relative to the current working directory.
    ///
    /// # Errors
    ///
    /// Returns an error if a unique directory name cannot be generated or if directory creation fails.
    #[cfg(feature = "rand_gen")]
    pub fn new_random_here<P: AsRef<Path>>(dir: Option<P>) -> TempResult<Self> {
        if let Some(dir) = dir {
            let d_ref = normalize_path(dir.as_ref());
            if d_ref.is_absolute() {
                Self::new_random(Some(d_ref))
            } else {
                Self::new_random(Some(&env::current_dir()?.join(d_ref)))
            }
        } else {
            Self::new_random(Some(&env::current_dir()?))
        }
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
        self.files
            .push(TempFile::new_random(Some(normalize_path(dir)))?);
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
    /// Creates a new temporary directory with a random name within the given parent directory.
    ///
    /// # Arguments
    ///
    /// * `path` - The parent directory in which to create the temporary directory. If a relative path is provided, it is resolved relative to the system temporary directory.
    ///
    /// # Errors
    ///
    /// Returns an error if a unique directory name cannot be generated or if directory creation fails.
    pub fn new_in<P: AsRef<Path>>(path: P) -> TempResult<Self> {
        Self::new_random(Some(path))
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
    pub fn find_files_by_pattern<S: AsRef<str>>(&self, pattern: S) -> TempResult<Vec<&TempFile>> {
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
    ) -> TempResult<Vec<&mut TempFile>> {
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
        match (self.path.take(), self.created_parent.take()) {
            (Some(p), None) => {
                self.files.clear();
                let _ = fs::remove_dir_all(p);
            }
            (Some(_), Some(d)) => {
                self.files.clear();
                let _ = fs::remove_dir_all(d);
            }
            _ => {}
        }
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        self.path.as_ref().expect("TempDir path is None")
    }
}
