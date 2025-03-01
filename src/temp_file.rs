#[cfg(feature = "rand_gen")]
use crate::global_consts::{num_retry, rand_fn_len, valid_chars};
#[cfg(feature = "mmap_support")]
use memmap2::{Mmap, MmapMut, MmapOptions};
#[cfg(feature = "rand_gen")]
use rand::Rng;
#[cfg(feature = "display_files")]
use std::fmt::Display;
use std::fmt::{Debug, Formatter};
#[cfg(unix)]
use std::fs::Permissions;
use std::fs::{File, OpenOptions};
use std::io::{self, IoSlice, IoSliceMut, Read, Seek, SeekFrom, Write};
use std::ops::{Deref, DerefMut};
#[cfg(unix)]
use std::os::fd::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::{env, fs};

use crate::error::{TempError, TempResult};
use crate::helpers::normalize_path;

/// A temporary file that is automatically deleted when dropped unless explicitly closed.
///
/// The file is opened with read and write permissions. When the instance is dropped,
/// the underlying file is removed unless deletion is disarmed (for example, by calling
/// [`close`](TempFile::close) or [`into_inner`](TempFile::into_inner)).
#[derive(Debug)]
pub struct TempFile {
    /// The full path to the temporary file.
    pub(crate) path: Option<PathBuf>,
    /// The underlying file handle.
    file: Option<File>,
    /// Directories created to hold the temporary file that did not exist.
    created_parent: Option<PathBuf>,
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
        let path_ref = normalize_path(path.as_ref());
        let path_buf = if path_ref.is_absolute() {
            path_ref
        } else {
            env::temp_dir().join(path_ref)
        };
        let (created, file) = Self::open(&path_buf)?;
        Ok(Self {
            path: Some(path_buf),
            file: Some(file),
            created_parent: created,
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
        let path_ref = normalize_path(path.as_ref());
        let path_buf = if path_ref.is_absolute() {
            path_ref
        } else {
            env::current_dir()?.join(path_ref)
        };
        let (created, file) = Self::open(&path_buf)?;
        Ok(Self {
            path: Some(path_buf),
            file: Some(file),
            created_parent: created,
        })
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
        self.rename(env::current_dir()?.join(name.as_ref()))?;
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
            let path_ref = normalize_path(d.as_ref());
            if path_ref.is_absolute() {
                path_ref
            } else {
                env::temp_dir().join(path_ref)
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
            let full_path = dir_buf.join(&name);
            if !full_path.exists() {
                let (created, file) = Self::open(&full_path)?;
                return Ok(Self {
                    path: Some(full_path),
                    file: Some(file),
                    created_parent: created,
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
            let d_ref = normalize_path(dir.as_ref());
            if d_ref.is_absolute() {
                Self::new_random(Some(d_ref))
            } else {
                Self::new_random(Some(env::current_dir()?.join(d_ref)))
            }
        } else {
            Self::new_random(Some(&env::current_dir()?))
        }
    }

    /// Opens a new file at the specified path, creating any missing parent directories if necessary.
    ///
    /// If the file already exists, an error is returned. On success, this function returns a tuple containing:
    /// - An `Option<PathBuf>` representing the created directory (if any),
    /// - The newly created file handle.
    fn open(path: &Path) -> TempResult<(Option<PathBuf>, File)> {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;
        let mut created = None;
        let par = path.parent();
        if path.exists() {
            return Err(TempError::PathExists(path.to_path_buf()));
        } else if let Some(c) = crate::helpers::first_missing_directory_component(path) {
            fs::create_dir_all(par.unwrap())?;
            created = Some(c);
        }
        let file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(path)
            .map_err(Into::into);
        if file.is_err() && created.is_some() {
            fs::remove_dir_all(created.clone().unwrap())?;
        }
        #[cfg(unix)]
        fs::set_permissions(path, Permissions::from_mode(0o700))?;
        file.map(|file| (created, file))
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

    /// Copies the temporary file to a new path and deletes the original file, "renaming" it.
    ///
    /// # Arguments
    ///
    /// * `new_path` - The new path for the file. If `new_path` is relative, it is appended to the old file's parent directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file copy or deletion operation fails, or if the old file's parent directory cannot be determined when `new_path` is relative.
    pub fn rename<P: AsRef<Path>>(&mut self, new_path: P) -> TempResult<()> {
        let mut new_path = normalize_path(new_path.as_ref());
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
                    .join(new_path);
            }
            fs::copy(old_path, new_path.clone())?;
            fs::remove_file(old_path)?;
            self.path = Some(new_path);
        }
        Ok(())
    }

    /// Copies the temporary file to a new path in the current directory and deletes the original file, "renaming" it.
    ///
    /// # Arguments
    ///
    /// * `new_path` - The new path for the file. If `new_path` is relative, it is resolved relative to the current working directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file copy or deletion operation fails.
    pub fn rename_here<P: AsRef<Path>>(&mut self, new_path: P) -> TempResult<()> {
        let mut new_path = normalize_path(new_path.as_ref());
        let pat = new_path.to_str().unwrap_or("");
        let mut mod_path = false;
        if !pat.contains('/') && !pat.contains('\\') {
            mod_path = true;
        }
        if let Some(ref old_path) = self.path {
            if mod_path {
                new_path = env::current_dir()?.join(new_path);
            }
            fs::copy(old_path, new_path.clone())?;
            fs::remove_file(old_path)?;
            self.path = Some(new_path);
        }
        Ok(())
    }

    /// Synchronizes the file’s state with the storage device.
    ///
    /// This is generally not needed. See [`File::sync_all`] for its purpose.
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
            fs::remove_file(path)?;
            self.path = None;
        }
        Ok(())
    }

    /// Retrieves metadata of the file.
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata cannot be accessed or if the file has been closed.
    pub fn metadata(&self) -> TempResult<fs::Metadata> {
        if let Some(ref path) = self.path {
            fs::metadata(path).map_err(Into::into)
        } else {
            Err(Into::into(io::Error::new(
                io::ErrorKind::NotFound,
                "File has been closed",
            )))
        }
    }

    /// A function to convert a normal File and its path into a `TempFile`.
    ///
    /// # Errors
    ///
    /// Returns an error if the Path and File do not point to the same file.
    #[cfg(unix)]
    pub fn from_fp<P: AsRef<Path>>(file: File, path: P) -> TempResult<Self> {
        if !Self::are_same_file(path.as_ref(), &file)? {
            return Err(TempError::InvalidFileOrPath);
        }
        Ok(Self {
            path: Some(path.as_ref().to_path_buf()),
            file: Some(file),
            created_parent: None,
        })
    }

    /// Helper function to validate that a given &Path and File both point to the same file.
    #[cfg(unix)]
    fn are_same_file(path: &Path, file: &File) -> io::Result<bool> {
        use std::fs::metadata;
        use std::os::unix::fs::MetadataExt;
        let path_metadata = metadata(path)?;
        let file_metadata = file.metadata()?;

        #[cfg(unix)]
        {
            // Compare device and inode
            Ok(path_metadata.dev() == file_metadata.dev()
                && path_metadata.ino() == file_metadata.ino())
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

    // Workaround for memmap2 API quirks... but seriously, why does it work like this?
    /// Converts a mutable file reference into a static immutable reference for use with memory mapping.
    ///
    /// # Safety
    ///
    /// This function intentionally leaks the provided file reference to extend its lifetime, satisfying the API requirements of the memory mapping library.
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

impl Drop for TempFile {
    fn drop(&mut self) {
        match (self.path.take(), self.created_parent.take()) {
            (Some(p), None) => {
                let _ = fs::remove_file(p);
            }
            (Some(_), Some(d)) => {
                let _ = fs::remove_dir_all(d);
            }
            _ => {}
        }
    }
}

impl AsRef<Path> for TempFile {
    fn as_ref(&self) -> &Path {
        // Instead of panicking if the path is None, we return an empty path.
        self.path.as_deref().unwrap_or_else(|| Path::new(""))
    }
}

impl AsRef<File> for TempFile {
    fn as_ref(&self) -> &File {
        self.file().expect("TempFile inner File is None")
    }
}

impl AsMut<File> for TempFile {
    fn as_mut(&mut self) -> &mut File {
        self.file_mut().expect("TempFile inner File is None")
    }
}

impl Deref for TempFile {
    type Target = File;
    fn deref(&self) -> &Self::Target {
        self.file().expect("TempFile inner File is None")
    }
}

impl DerefMut for TempFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.file_mut().expect("TempFile inner File is None")
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

#[cfg(unix)]
impl AsRawFd for TempFile {
    fn as_raw_fd(&self) -> RawFd {
        // Return -1 if the file is not available.
        self.file.as_ref().map_or(-1, AsRawFd::as_raw_fd)
    }
}

#[cfg(feature = "display_files")]
impl Display for TempFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.file {
            None => writeln!(f, "No file"),
            Some(ref file) => {
                let mut buf = Vec::new();
                file.try_clone()
                    .expect("Failed to get new file handle")
                    .read_to_end(&mut buf)
                    .expect("Failed to read from file");
                writeln!(f, "{}", sew::infallible::InfallibleString::from(buf))
            }
        }
    }
}
