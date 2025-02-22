#[cfg(feature = "rand_gen")]
use crate::global_consts::{NUM_RETRY, RAND_FN_LEN, VALID_CHARS};
#[cfg(feature = "mmap_support")]
use memmap2::{Mmap, MmapMut, MmapOptions};
#[cfg(feature = "rand_gen")]
use rand::Rng;
use std::env;
use std::fmt::{Debug, Formatter};
use std::fs::{File, OpenOptions};
use std::io::{self, IoSlice, IoSliceMut, Read, Seek, SeekFrom, Write};
use std::ops::{Deref, DerefMut};
#[cfg(unix)]
use std::os::fd::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};

use crate::error::{TempError, TempResult};

/// A temporary file that is automatically deleted when dropped unless explicitly closed.
///
/// The file is opened with read and write permissions. When the instance is dropped,
/// the underlying file is removed unless deletion is disarmed (for example, by calling
/// [`close`](TempFile::close) or [`into_inner`](TempFile::into_inner)).
pub struct TempFile {
    /// The full path to the temporary file.
    pub(crate) path: Option<PathBuf>,
    /// The underlying file handle.
    file: Option<File>,
}

impl TempFile {
    /// Creates a new temporary file at the specified path.
    ///
    /// The file is created with read and write permissions.
    ///
    /// # Arguments
    ///
    /// * `path` - The path at which to create the file. If a relative path is provided, it is resolved relative to the system temporary directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created.
    pub fn new<P: AsRef<Path>>(path: P) -> TempResult<TempFile> {
        let path_ref = path.as_ref();
        let path_buf = if path_ref.is_absolute() {
            path_ref.to_owned()
        } else {
            env::temp_dir().join(path_ref)
        };
        let file = Self::open(&path_buf)?;
        Ok(Self {
            path: Some(path_buf),
            file: Some(file),
        })
    }

    /// Creates a new temporary file at the specified path.
    ///
    /// The file is created with read and write permissions.
    ///
    /// # Arguments
    ///
    /// * `path` - The path at which to create the file. If a relative path is provided, it is resolved relative to the current working directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created.
    pub fn new_here<P: AsRef<Path>>(path: P) -> TempResult<TempFile> {
        if path.as_ref().is_relative() {
            Self::new(env::current_dir()?.join(path))
        } else {
            Self::new(path)
        }
    }

    /// Converts the `TempFile` into a permanent file.
    ///
    /// # Errors
    ///
    /// Returns an error if the inner file is `None`.
    pub fn persist(&mut self) -> TempResult<File> {
        self.path = None;
        self.file.take().ok_or(TempError::FileIsNone)
    }

    /// Renames the temporary file and then persists it.
    ///
    /// # Errors
    ///
    /// Returns an error if renaming or persisting the file fails.
    pub fn persist_name<S: AsRef<str>>(&mut self, name: S) -> TempResult<File> {
        self.rename(name.as_ref())?;
        self.persist()
    }

    /// Renames the temporary file (in the current directory) and persists it.
    ///
    /// # Errors
    ///
    /// Returns an error if renaming or persisting the file fails.
    pub fn persist_here<S: AsRef<str>>(&mut self, name: S) -> TempResult<File> {
        self.rename(format!("./{}", name.as_ref()))?;
        self.persist()
    }

    #[cfg(feature = "rand_gen")]
    /// Creates a new temporary file with a random name in the given directory.
    ///
    /// The file name is generated using random ASCII characters.
    ///
    /// # Arguments
    ///
    /// * `dir` - The directory in which to create the file. If `None`, the system temporary directory is used. If a relative directory is provided, it is resolved relative to the system temporary directory.
    ///
    /// # Errors
    ///
    /// Returns an error if a unique filename cannot be generated or if file creation fails.
    pub fn new_random<P: AsRef<Path>>(dir: Option<P>) -> TempResult<Self> {
        let dir_buf = if let Some(d) = dir {
            let d_ref = d.as_ref();
            if d_ref.is_absolute() {
                d_ref.to_path_buf()
            } else {
                env::temp_dir().join(d_ref)
            }
        } else {
            env::temp_dir()
        };
        let mut rng = rand::rng();
        for _ in 0..NUM_RETRY {
            let name: String = (0..RAND_FN_LEN)
                .map(|_| {
                    let idx = rng.random_range(0..VALID_CHARS.len());
                    VALID_CHARS[idx] as char
                })
                .collect();
            let full_path = dir_buf.join(&name);
            if !full_path.exists() {
                let file = Self::open(&full_path)?;
                return Ok(Self {
                    path: Some(full_path),
                    file: Some(file),
                });
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Could not generate a unique filename",
        )
        .into())
    }

    #[cfg(feature = "rand_gen")]
    /// Creates a new temporary file with a random name in the given directory.
    ///
    /// The file name is generated using random ASCII characters.
    ///
    /// # Arguments
    ///
    /// * `dir` - The directory in which to create the file. If `None`, the current working directory is used. If a relative directory is provided, it is resolved relative to the current directory.
    ///
    /// # Errors
    ///
    /// Returns an error if a unique filename cannot be generated or if file creation fails.
    pub fn new_random_here<P: AsRef<Path>>(dir: Option<P>) -> TempResult<Self> {
        if let Some(dir) = dir {
            if dir.as_ref().is_absolute() {
                Self::new_random(Some(dir))
            } else {
                Self::new_random(Some(env::current_dir()?.join(dir)))
            }
        } else {
            Self::new_random(Some("./"))
        }
    }

    /// Helper method to open file.
    fn open(path: &Path) -> TempResult<File> {
        OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(path)
            .map_err(Into::into)
    }

    /// Returns a mutable reference to the file handle.
    ///
    /// # Errors
    ///
    /// Returns `Err(TempError::FileIsNone)` if the file handle is not available.
    pub fn file_mut(&mut self) -> TempResult<&mut File> {
        self.file.as_mut().ok_or(TempError::FileIsNone)
    }

    /// Returns an immutable reference to the file handle.
    ///
    /// # Errors
    ///
    /// Returns `Err(TempError::FileIsNone)` if the file handle is not available.
    pub fn file(&self) -> TempResult<&File> {
        self.file.as_ref().ok_or(TempError::FileIsNone)
    }

    /// Returns the path to the temporary file.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Renames the temporary file.
    ///
    /// # Arguments
    ///
    /// * `new_path` - The new path for the file. If relative, its new path will be its old path's parent, followed by this. See [`rename_here`](TempFile::rename_here) for a method which renames relative paths to the current directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the rename operation fails or the old path has no parent and the new path is relative.
    pub fn rename<P: AsRef<Path>>(&mut self, new_path: P) -> TempResult<()> {
        let mut new_path = new_path.as_ref().to_path_buf();
        let pat = new_path.to_str().unwrap_or("");
        let mut mod_path = false;
        if !pat.contains('/') && !pat.contains('\\') {
            mod_path = true;
        }
        if let Some(ref old_path) = self.path {
            if mod_path {
                new_path = old_path
                    .parent()
                    .ok_or(TempError::IO(io::Error::new(
                        io::ErrorKind::NotFound,
                        "Old path parent not found",
                    )))?
                    .to_path_buf()
                    .join(new_path);
            }
            std::fs::copy(old_path, new_path.clone())?;
            std::fs::remove_file(old_path)?;
            self.path = Some(new_path);
        }
        Ok(())
    }

    /// Renames the temporary file using the current directory.
    ///
    /// # Arguments
    ///
    /// * `new_path` - The new path for the file. If relative, its new path will be the current directory, followed by this. See [`rename`](TempFile::rename) for a method which renames relative paths to the old directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the rename operation fails.
    pub fn rename_here<P: AsRef<Path>>(&mut self, new_path: P) -> TempResult<()> {
        let mut new_path = new_path.as_ref().to_path_buf();
        let pat = new_path.to_str().unwrap_or("");
        let mut mod_path = false;
        if !pat.contains('/') && !pat.contains('\\') {
            mod_path = true;
        }
        if let Some(ref old_path) = self.path {
            if mod_path {
                new_path = env::current_dir()?.join(new_path);
            }
            std::fs::copy(old_path, new_path.clone())?;
            std::fs::remove_file(old_path)?;
            self.path = Some(new_path);
        }
        Ok(())
    }

    /// Synchronizes the file’s state with the storage device.
    ///
    /// This is generally not needed. [`File::sync_all`](File::sync_all) for its purpose.
    ///
    /// # Errors
    ///
    /// Returns `Err(TempError::FileIsNone)` if the file handle is not available, or if syncing fails.
    pub fn sync_all(&self) -> TempResult<()> {
        self.file()?.sync_all().map_err(Into::into)
    }

    /// Flushes the file and disarms automatic deletion.
    ///
    /// After calling this method, the file will not be deleted when the `TempFile` is dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if flushing fails or if the file handle is not available.
    pub fn disarm(mut self) -> TempResult<()> {
        self.file_mut()?.flush().map_err(Into::<TempError>::into)?;
        self.path = None;
        Ok(())
    }

    /// Flushes and closes the file, disarming deletion.
    ///
    /// After calling this method, the file will not be deleted when the `TempFile` is dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if flushing fails or if the file handle is not available.
    pub fn close(mut self) -> TempResult<()> {
        self.file_mut()?.flush().map_err(Into::<TempError>::into)?;
        self.path = None;
        self.file = None;
        Ok(())
    }

    /// Consumes the `TempFile` and returns the inner file handle.
    ///
    /// This method disarms automatic deletion.
    ///
    /// # Errors
    ///
    /// Returns `Err(TempError::FileIsNone)` if the file handle has already been taken.
    pub fn into_inner(mut self) -> TempResult<File> {
        self.path = None;
        self.file.take().ok_or(TempError::FileIsNone)
    }

    /// Checks if the file is still active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.path.is_some()
    }

    /// Deletes the temporary file immediately.
    ///
    /// This method flushes the file, removes it from the filesystem, and disarms automatic deletion.
    ///
    /// # Errors
    ///
    /// Returns an error if flushing fails, if the file handle is not available, or if file removal fails.
    pub fn delete(mut self) -> TempResult<()> {
        self.file_mut()?.flush().map_err(Into::<TempError>::into)?;
        if let Some(ref path) = self.path {
            std::fs::remove_file(path)?;
            self.path = None;
        }
        Ok(())
    }

    /// Retrieves metadata of the file.
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata cannot be accessed or if the file has been closed.
    pub fn metadata(&self) -> TempResult<std::fs::Metadata> {
        if let Some(ref path) = self.path {
            std::fs::metadata(path).map_err(Into::into)
        } else {
            Err(Into::into(io::Error::new(
                io::ErrorKind::NotFound,
                "File has been closed",
            )))
        }
    }
}

#[cfg(feature = "mmap_support")]
impl TempFile {
    /// Creates a read-only memory map of the file.
    ///
    /// # Safety
    ///
    /// This operation is unsafe because it relies on the underlying file not changing unexpectedly.
    ///
    /// # Errors
    ///
    /// Returns an error if mapping the file fails.
    pub unsafe fn mmap(&self) -> TempResult<Mmap> {
        let file = self.file()?;
        unsafe { MmapOptions::new().map(file).map_err(Into::into) }
    }

    /// Creates a mutable memory map of the file.
    ///
    /// # Safety
    ///
    /// This operation is unsafe because it allows mutable access to the file's contents.
    ///
    /// # Errors
    ///
    /// Returns an error if mapping the file fails.
    pub unsafe fn mmap_mut(&mut self) -> TempResult<MmapMut> {
        let file = self.file_mut()?;
        unsafe {
            MmapOptions::new()
                .map_mut(Self::immut(file))
                .map_err(Into::into)
        }
    }

    /// Workaround for memmap2 API quirks... but seriously, why does it work like this?
    fn immut(file: &mut File) -> &File {
        Box::leak(Box::new(file))
    }
}

impl Write for TempFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(ref mut file) = self.file {
            file.write(buf)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                TempError::FileIsNone,
            ))
        }
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        if let Some(ref mut file) = self.file {
            file.write_vectored(bufs)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                TempError::FileIsNone,
            ))
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        if let Some(ref mut file) = self.file {
            file.flush()
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                TempError::FileIsNone,
            ))
        }
    }
}

impl Read for TempFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(ref mut file) = self.file {
            file.read(buf)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                TempError::FileIsNone,
            ))
        }
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        if let Some(ref mut file) = self.file {
            file.read_vectored(bufs)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                TempError::FileIsNone,
            ))
        }
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        if let Some(ref mut file) = self.file {
            file.read_to_end(buf)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                TempError::FileIsNone,
            ))
        }
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        if let Some(ref mut file) = self.file {
            file.read_to_string(buf)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                TempError::FileIsNone,
            ))
        }
    }
}

impl Seek for TempFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        if let Some(ref mut file) = self.file {
            file.seek(pos)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                TempError::FileIsNone,
            ))
        }
    }
}

impl Debug for TempFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TempFile")
            .field("path", &self.path)
            .field("file", &self.file)
            .finish()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Some(ref path) = self.path {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl AsRef<Path> for TempFile {
    fn as_ref(&self) -> &Path {
        // Instead of panicking if the path is None, we return an empty path.
        self.path.as_deref().unwrap_or_else(|| Path::new(""))
    }
}

#[cfg(unix)]
impl AsRawFd for TempFile {
    fn as_raw_fd(&self) -> RawFd {
        // Return -1 if the file is not available.
        self.file.as_ref().map_or(-1, AsRawFd::as_raw_fd)
    }
}

impl Deref for TempFile {
    type Target = File;
    fn deref(&self) -> &Self::Target {
        self.file.as_ref().expect("TempFile file is None")
    }
}

impl DerefMut for TempFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.file.as_mut().expect("TempFile file is None")
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsRawHandle for TempFile {
    fn as_raw_handle(&self) -> std::os::windows::io::RawHandle {
        // Return a null handle if the file is not available.
        self.file
            .as_ref()
            .map(|f| f.as_raw_handle())
            .unwrap_or(std::ptr::null_mut())
    }
}
