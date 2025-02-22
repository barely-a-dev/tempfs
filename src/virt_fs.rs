use std::fmt::Display;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::SystemTime;
use std::{fs, str};
use crate::error::FsError;

/// Splits a path string (e.g. "/a/b/c") into its non-empty components as string slices.
fn get_components(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// Splits a path string (e.g. "/a/b/c") into its non-empty components as owned Strings.
fn get_components_string(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Converts a `VirtPath` to a String (assuming valid UTF-8).
fn path_to_str(vp: &VirtPath) -> String {
    String::from_utf8_lossy(vp.bytes()).to_string()
}

/// Helper to canonicalize paths, eliminating components like "." or ".."
fn normalize_path(path: &str) -> String {
    let is_absolute = path.starts_with('/');
    let comps = path.split('/'); // will include empty strings for leading '/'
    let mut stack = Vec::new();
    for comp in comps {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp == ".." {
            if stack.pop().is_some() {
                // Pop the last component if available.
            } else if !is_absolute {
                // For relative paths, preserve leading ".." components.
                stack.push("..");
            }
        } else {
            stack.push(comp);
        }
    }
    if is_absolute {
        format!("/{}", stack.join("/"))
    } else {
        stack.join("/")
    }
}

/// Unix-like permissions stored as a mode bitmask.
#[derive(Clone, Debug)]
pub struct VirtPermissions {
    /// The permission mode of the fs entry parent.
    pub mode: u16,
}

impl VirtPermissions {
    /// User read permission bitmask.
    pub const S_IRUSR: u16 = 0o400;
    /// User write permission bitmask.
    pub const S_IWUSR: u16 = 0o200;
    /// User execute permission bitmask.
    pub const S_IXUSR: u16 = 0o100;
    /// Group read permission bitmask.
    pub const S_IRGRP: u16 = 0o040;
    /// Group write permission bitmask.
    pub const S_IWGRP: u16 = 0o020;
    /// Group execute permission bitmask.
    pub const S_IXGRP: u16 = 0o010;
    /// Others read permission bitmask.
    pub const S_IROTH: u16 = 0o004;
    /// Others write permission bitmask.
    pub const S_IWOTH: u16 = 0o002;
    /// Others execute permission bitmask.
    pub const S_IXOTH: u16 = 0o001;

    /// Create new permissions with the given mode.
    #[must_use]
    pub fn new(mode: u16) -> Self {
        VirtPermissions { mode }
    }
}

impl Display for VirtPermissions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::with_capacity(9);
        // The loop below uses the same mask shifting for owner, group, others.
        for &(bit, r, w, x) in &[
            (Self::S_IRUSR, 'r', 'w', 'x'),
            (Self::S_IRGRP, 'r', 'w', 'x'),
            (Self::S_IROTH, 'r', 'w', 'x'),
        ] {
            s.push(if self.mode & bit != 0 { r } else { '-' });
            s.push(if self.mode & (bit >> 1) != 0 { w } else { '-' });
            s.push(if self.mode & (bit >> 2) != 0 { x } else { '-' });
        }
        write!(f, "{s}")
    }
}

/// Metadata for files (and directories).
#[derive(Clone)]
pub struct VirtMetadata {
    /// The permissions of the fs entry parent.
    pub permissions: VirtPermissions,
    /// The owner of the fs entry parent.
    pub owner: String,
    /// The group of the fs entry parent.
    pub group: String,
    /// The time the fs entry parent was created.
    pub created: SystemTime,
    /// The last time the fs entry parent was modified.
    pub modified: SystemTime,
}

impl VirtMetadata {
    /// Create new metadata with a default mode and current timestamps.
    #[must_use]
    pub fn new(default_mode: u16) -> Self {
        let now = SystemTime::now();
        VirtMetadata {
            permissions: VirtPermissions::new(default_mode),
            owner: "root".to_string(),
            group: "root".to_string(),
            created: now,
            modified: now,
        }
    }
}

/// Virtual (in-memory) path representation.
#[derive(Clone)]
pub enum VirtPath {
    /// The path is relative, like "output.txt"
    Relative(Vec<u8>),
    /// The path is absolute, like "/home/user/output.txt"
    Absolute(Vec<u8>),
}

impl From<&str> for VirtPath {
    fn from(s: &str) -> Self {
        if s.starts_with('/') {
            VirtPath::Absolute(s.as_bytes().to_vec())
        } else {
            VirtPath::Relative(s.as_bytes().to_vec())
        }
    }
}

impl From<String> for VirtPath {
    fn from(s: String) -> Self {
        if s.starts_with('/') {
            VirtPath::Absolute(s.as_bytes().to_vec())
        } else {
            VirtPath::Relative(s.as_bytes().to_vec())
        }
    }
}

impl AsRef<VirtPath> for VirtPath {
    fn as_ref(&self) -> &VirtPath {
        self
    }
}

impl VirtPath {
    /// Return the internal byte representation of the virtual path.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        match self {
            VirtPath::Absolute(ref v) | VirtPath::Relative(ref v) => v.as_slice(),
        }
    }

    /// Return a mutable reference to the internal byte vector of the virtual path.
    pub fn bytes_mut(&mut self) -> &mut Vec<u8> {
        match self {
            VirtPath::Absolute(ref mut v) | VirtPath::Relative(ref mut v) => v,
        }
    }

    /// Navigate from self by appending a relative path component.
    ///
    /// If the current path does not end with '/', one is inserted before appending.
    #[must_use]
    pub fn nav_rel<P: Into<VirtPath>>(&self, rhs: P) -> VirtPath {
        let rhs = rhs.into();
        let mut base = self.clone();
        if !base.bytes().ends_with(b"/") {
            base.bytes_mut().push(b'/');
        }
        base.bytes_mut().extend_from_slice(rhs.bytes());
        base
    }
}

/// A virtual in-memory filesystem that supports Unix-like file operations.
pub struct VirtFS {
    /// The root directory.
    root: VirtDir,
    /// The current working directory.
    current_dir: VirtPath,
}

#[derive(Clone)]
/// A virtual directory in the in-memory filesystem.
pub struct VirtDir {
    /// The path to the directory.
    pub path: VirtPath,
    /// The files in the directory.
    pub files: Vec<VirtFile>,
    /// The subdirectories in the directory.
    pub dirs: Vec<VirtDir>,
    /// The metadata of the directory.
    pub metadata: VirtMetadata,
}

/// A virtual file. In addition to the metadata and content,
/// it maintains an internal cursor so that it implements
/// the standard I/O traits (Read, Write, Seek).
#[derive(Clone)]
pub struct VirtFile {
    /// The path to the file.
    pub path: VirtPath,
    /// The raw content of the file in bytes.
    pub content: Vec<u8>,
    /// The metadata of the file.
    pub metadata: VirtMetadata,
    /// Current cursor position in the file.
    cursor: usize,
}

impl VirtFile {
    /// Create a new file with an initial empty content and a zero cursor.
    pub fn new<P: Into<VirtPath>>(path: P, metadata: VirtMetadata) -> Self {
        VirtFile {
            path: path.into().clone(),
            content: Vec::new(),
            metadata,
            cursor: 0,
        }
    }

    /// Reset the internal cursor (e.g. to rewind the file).
    pub fn reset_cursor(&mut self) {
        self.cursor = 0;
    }

    /// Attempt to create a virtual file from a real file path, reading its content.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the file from the given real path fails.
    pub fn try_from_real_path<P: AsRef<Path>, VP: Into<VirtPath>>(
        path: P,
        new_path: VP,
    ) -> std::io::Result<Self> {
        let path = path.as_ref();
        match fs::read(path) {
            Ok(b) => Ok(Self {
                path: new_path.into().clone(),
                content: b,
                metadata: VirtMetadata::new(0o755),
                cursor: 0,
            }),
            Err(e) => Err(e),
        }
    }

    /// Attempt to create a virtual file from an open real file, reading its content.
    ///
    /// # Errors
    ///
    /// Returns an error if seeking or reading from the file fails.
    pub fn try_from_real<VP: Into<VirtPath>>(
        file: &mut File,
        new_path: VP,
    ) -> std::io::Result<Self> {
        let mut buf = Vec::new();
        file.seek(SeekFrom::Start(0))?;
        file.read_to_end(&mut buf)?;
        Ok(Self {
            path: new_path.into().clone(),
            content: buf,
            metadata: VirtMetadata::new(0o755),
            cursor: 0,
        })
    }
}

impl Default for VirtFS {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtFS {
    /// Create a new file system with root at "/".
    #[must_use]
    pub fn new() -> VirtFS {
        let root_dir = VirtDir {
            path: VirtPath::Absolute(b"/".to_vec()),
            files: Vec::new(),
            dirs: Vec::new(),
            metadata: VirtMetadata::new(0o755),
        };
        VirtFS {
            root: root_dir,
            current_dir: VirtPath::Absolute(b"/".to_vec()),
        }
    }

    /// Resolve a given path (absolute or relative) to an absolute, normalized virtual path.
    ///
    /// Relative paths are joined with the current working directory and any "." or ".." components are resolved.
    fn resolve_path<P: Into<VirtPath>>(&self, path: P) -> VirtPath {
        let p = path_to_str(&path.into());
        if p.starts_with('/') {
            let norm = normalize_path(&p);
            VirtPath::Absolute(norm.into_bytes())
        } else {
            let cur = path_to_str(&self.current_dir);
            let joined = if cur.ends_with('/') {
                format!("{cur}{p}")
            } else {
                format!("{cur}/{p}")
            };
            let norm = normalize_path(&joined);
            VirtPath::Absolute(norm.into_bytes())
        }
    }

    /// Change directory. Absolute paths replace the current directory;
    /// relative ones are joined to the current directory.
    pub fn cd<P: Into<VirtPath>>(&mut self, path: P) {
        let path = path.into();
        match path {
            VirtPath::Absolute(_) => {
                self.current_dir = path.clone();
            }
            VirtPath::Relative(_) => {
                self.current_dir = self.current_dir.nav_rel(path);
            }
        }
        // Normalize the current directory after change.
        let normalized = normalize_path(&path_to_str(&self.current_dir));
        self.current_dir = VirtPath::Absolute(normalized.into_bytes());
    }

    /// Return the current working directory as a string.
    #[must_use]
    pub fn pwd(&self) -> String {
        path_to_str(&self.current_dir)
    }

    /// Recursively create directories given a (absolute or relative) path.
    /// If intermediate directories do not exist, an error is returned.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is invalid or a required directory is not found.
    ///
    /// # Panics
    ///
    /// Panics if file name extraction via `unwrap()` fails.
    pub fn mkdir<P: Into<VirtPath>>(&mut self, path: P) -> Result<(), FsError> {
        let abs = self.resolve_path(path);
        let comps = get_components_string(&path_to_str(&abs));
        let mut current = &mut self.root;
        let mut current_path = String::from("/");
        for comp in comps {
            if let Some(dir) = current.find_dir(&comp) {
                *current = dir.clone();
            } else {
                if current_path != "/" {
                    current_path.push('/');
                }
                current_path.push_str(&comp);
                let new_dir = VirtDir {
                    path: VirtPath::Absolute(current_path.as_bytes().to_vec()),
                    files: Vec::new(),
                    dirs: Vec::new(),
                    metadata: VirtMetadata::new(0o755),
                };
                current.dirs.push(new_dir);
                current = current.find_dir_mut(&comp).unwrap();
            }
        }
        Ok(())
    }

    /// Create an empty file at the specified path (touch).
    /// If intermediate directories do not exist, an error is returned.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is invalid or a required directory is not found.
    ///
    /// # Panics
    ///
    /// Panics if file name extraction via `unwrap()` fails.
    pub fn touch<P: Into<VirtPath>>(&mut self, path: P) -> Result<(), FsError> {
        let abs = self.resolve_path(path);
        let comps = get_components_string(&path_to_str(&abs));
        if comps.is_empty() {
            return Err(FsError::InvalidPath("Empty file name".to_string()));
        }
        let file_name = comps.last().unwrap();
        let dir_path = if comps.len() == 1 {
            "/".to_string()
        } else {
            format!("/{}", comps[..comps.len() - 1].join("/"))
        };
        let dir_comps = get_components(&dir_path);
        let mut current = &mut self.root;
        for comp in dir_comps {
            current = current
                .find_dir_mut(comp)
                .ok_or_else(|| FsError::NotFound(format!("Directory {comp} not found")))?;
        }
        // If the file already exists, simply return.
        if current.find_file_mut(file_name).is_some() {
            return Ok(());
        }
        let file_full_path = if dir_path == "/" {
            format!("/{file_name}")
        } else {
            format!("{dir_path}/{file_name}")
        };
        let new_file = VirtFile::new(
            VirtPath::Absolute(file_full_path.as_bytes().to_vec()),
            VirtMetadata::new(0o644),
        );
        current.files.push(new_file);
        Ok(())
    }

    /// Open (or create) a file for reading/writing. This method returns
    /// a mutable reference to the `VirtFile` so that users can call its
    /// read, write, and seek methods.
    ///
    /// # Errors
    ///
    /// Returns an error if the file or its parent directory cannot be found or created.
    pub fn open<P: Into<VirtPath> + Clone>(&mut self, path: P) -> Result<&mut VirtFile, FsError> {
        // Create the file if it does not exist.
        self.touch(path.clone().into())?;
        self.open_file_mut(path)
    }

    /// Retrieve a mutable reference to a file given its path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file or its parent directory cannot be found.
    ///
    /// # Panics
    ///
    /// Panics if file name extraction via `unwrap()` fails.
    pub fn open_file_mut<P: Into<VirtPath>>(&mut self, path: P) -> Result<&mut VirtFile, FsError> {
        let abs = self.resolve_path(path);
        let comps = get_components_string(&path_to_str(&abs));
        if comps.is_empty() {
            return Err(FsError::InvalidPath("Empty file name".to_string()));
        }
        let file_name = comps.last().unwrap();
        let dir_path = if comps.len() == 1 {
            "/".to_string()
        } else {
            format!("/{}", comps[..comps.len() - 1].join("/"))
        };
        let dir_comps = get_components(&dir_path);
        let mut current = &mut self.root;
        for comp in dir_comps {
            current = current
                .find_dir_mut(comp)
                .ok_or_else(|| FsError::NotFound(format!("Directory {comp} not found")))?;
        }
        current
            .find_file_mut(file_name)
            .ok_or_else(|| FsError::NotFound(format!("File {file_name} not found")))
    }

    /// List the contents (directories and files) of the given path,
    /// or the current directory if None is provided.
    ///
    /// # Errors
    ///
    /// Returns an error if the target directory cannot be found.
    pub fn ls<P: Into<VirtPath>>(&self, path: Option<P>) -> Result<Vec<String>, FsError> {
        let target_path = if let Some(p) = path {
            self.resolve_path(p)
        } else {
            self.current_dir.clone()
        };
        let comps = get_components_string(&path_to_str(&target_path));
        let mut current = &self.root;
        for comp in comps {
            current = current
                .find_dir(&comp)
                .ok_or_else(|| FsError::NotFound(format!("Directory {comp} not found")))?;
        }
        let mut entries = Vec::new();
        for d in &current.dirs {
            entries.push(format!("{}/", d.name()));
        }
        for f in &current.files {
            let full = path_to_str(&f.path);
            let comps = get_components(&full);
            if let Some(name) = comps.last() {
                entries.push((*name).to_string());
            }
        }
        Ok(entries)
    }

    /// Remove a file at the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file or its parent directory cannot be found, or if the file is not found.
    ///
    /// # Panics
    ///
    /// Panics if file name extraction via `unwrap()` fails.
    pub fn rm<P: Into<VirtPath>>(&mut self, path: P) -> Result<(), FsError> {
        let abs = self.resolve_path(path);
        let comps = get_components_string(&path_to_str(&abs));
        if comps.is_empty() {
            return Err(FsError::InvalidPath("Empty file name".to_string()));
        }
        let file_name = comps.last().unwrap();
        let dir_path = if comps.len() == 1 {
            "/".to_string()
        } else {
            format!("/{}", comps[..comps.len() - 1].join("/"))
        };
        let dir_comps = get_components(&dir_path);
        let mut current = &mut self.root;
        for comp in dir_comps {
            current = current
                .find_dir_mut(comp)
                .ok_or_else(|| FsError::NotFound(format!("Directory {comp} not found")))?;
        }
        let initial_len = current.files.len();
        current.files.retain(|f| {
            let full = path_to_str(&f.path);
            let comps = get_components(&full);
            comps.last().is_none_or(|s| *s != *file_name)
        });
        if current.files.len() == initial_len {
            return Err(FsError::NotFound(format!("File {file_name} not found")));
        }
        Ok(())
    }

    /// Remove an empty directory at the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be found or is not empty.
    ///
    /// # Panics
    ///
    /// Panics if target directory extraction via `unwrap()` fails.
    pub fn rmdir<P: Into<VirtPath>>(&mut self, path: P) -> Result<(), FsError> {
        let abs = self.resolve_path(path);
        let comps = get_components_string(&path_to_str(&abs));
        if comps.is_empty() {
            return Err(FsError::InvalidPath("Cannot remove root".to_string()));
        }
        let target_dir = comps.last().unwrap();
        let parent_path = format!("/{}", comps[..comps.len() - 1].join("/"));
        let parent_comps = get_components(&parent_path);
        let mut parent = &mut self.root;
        for comp in parent_comps {
            parent = parent
                .find_dir_mut(comp)
                .ok_or_else(|| FsError::NotFound(format!("Directory {comp} not found")))?;
        }
        // Ensure directory is empty.
        if let Some(dir) = parent.find_dir(target_dir) {
            if !dir.files.is_empty() || !dir.dirs.is_empty() {
                return Err(FsError::AlreadyExists(format!(
                    "Directory {target_dir} is not empty"
                )));
            }
        }
        let initial_len = parent.dirs.len();
        parent.dirs.retain(|d| d.name() != *target_dir);
        if parent.dirs.len() == initial_len {
            return Err(FsError::NotFound(format!(
                "Directory {target_dir} not found"
            )));
        }
        Ok(())
    }

    /// Change the permission bits of a file or directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file or directory cannot be found.
    ///
    /// # Panics
    ///
    /// Panics if extraction of the entry name via `unwrap()` fails.
    pub fn chmod<P: Into<VirtPath>>(&mut self, path: P, mode: u16) -> Result<(), FsError> {
        // Try as file first.
        let abs = self.resolve_path(path);
        let comps = get_components_string(&path_to_str(&abs));
        if comps.is_empty() {
            return Err(FsError::InvalidPath("Empty path".to_string()));
        }
        let name = comps.last().unwrap();
        let dir_path = if comps.len() == 1 {
            "/".to_string()
        } else {
            format!("/{}", comps[..comps.len() - 1].join("/"))
        };
        let dir_comps = get_components(&dir_path);
        let mut current = &mut self.root;
        for comp in dir_comps {
            current = current
                .find_dir_mut(comp)
                .ok_or_else(|| FsError::NotFound(format!("Directory {comp} not found")))?;
        }
        if let Some(file) = current.find_file_mut(name) {
            file.metadata.permissions.mode = mode;
            file.metadata.modified = SystemTime::now();
            return Ok(());
        }
        if let Some(dir) = current.find_dir_mut(name) {
            dir.metadata.permissions.mode = mode;
            dir.metadata.modified = SystemTime::now();
            return Ok(());
        }
        Err(FsError::NotFound(format!("Entry {name} not found",)))
    }

    /// Change the owner and group of a file or directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file or directory cannot be found.
    ///
    /// # Panics
    ///
    /// Panics if extraction of the entry name via `unwrap()` fails.
    pub fn chown<P: Into<VirtPath>>(
        &mut self,
        path: P,
        owner: &str,
        group: &str,
    ) -> Result<(), FsError> {
        let abs = self.resolve_path(path);
        let comps = get_components_string(&path_to_str(&abs));
        if comps.is_empty() {
            return Err(FsError::InvalidPath("Empty path".to_string()));
        }
        let name = comps.last().unwrap();
        let dir_path = if comps.len() == 1 {
            "/".to_string()
        } else {
            format!("/{}", comps[..comps.len() - 1].join("/"))
        };
        let dir_comps = get_components(&dir_path);
        let mut current = &mut self.root;
        for comp in dir_comps {
            current = current
                .find_dir_mut(comp)
                .ok_or_else(|| FsError::NotFound(format!("Directory {comp} not found")))?;
        }
        if let Some(file) = current.find_file_mut(name) {
            file.metadata.owner = owner.to_string();
            file.metadata.group = group.to_string();
            file.metadata.modified = SystemTime::now();
            return Ok(());
        }
        if let Some(dir) = current.find_dir_mut(name) {
            dir.metadata.owner = owner.to_string();
            dir.metadata.group = group.to_string();
            dir.metadata.modified = SystemTime::now();
            return Ok(());
        }
        Err(FsError::NotFound(format!("Entry {name} not found")))
    }

    /// Get a clone of the metadata (stat) for a file or directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the target entry cannot be found.
    ///
    /// # Panics
    ///
    /// Panics if extraction of the entry name via `unwrap()` fails.
    pub fn stat<P: Into<VirtPath>>(&self, path: P) -> Result<VirtMetadata, FsError> {
        let abs = self.resolve_path(path);
        let comps = get_components_string(&path_to_str(&abs));
        if comps.is_empty() {
            return Err(FsError::InvalidPath("Empty path".to_string()));
        }
        let name = comps.last().unwrap();
        let dir_path = if comps.len() == 1 {
            "/".to_string()
        } else {
            format!("/{}", comps[..comps.len() - 1].join("/"))
        };
        let dir_comps = get_components(&dir_path);
        let mut current = &self.root;
        for comp in dir_comps {
            current = current
                .find_dir(comp)
                .ok_or_else(|| FsError::NotFound(format!("Directory {comp} not found")))?;
        }
        if let Some(file) = current.find_file(name) {
            return Ok(file.metadata.clone());
        }
        if let Some(dir) = current.find_dir(name) {
            return Ok(dir.metadata.clone());
        }
        Err(FsError::NotFound(format!("Entry {name} not found")))
    }

    /// Rename (or move) a file or directory from `src` to `dst`.
    /// This method updates the entry’s internal path and moves it from its original parent
    /// to the destination’s parent directory.
    ///
    /// # Errors
    ///
    /// Returns an error if either the source or destination directory cannot be found, or if the source entry does not exist.
    ///
    /// # Panics
    ///
    /// Panics if extraction of the source or destination file/directory name via `unwrap()` fails.
    pub fn rename<P: Into<VirtPath>, P2: Into<VirtPath>>(
        &mut self,
        src: P,
        dst: P2,
    ) -> Result<(), FsError> {
        let src_abs = self.resolve_path(src);
        let dst_abs = self.resolve_path(dst);

        let src_comps = get_components_string(&path_to_str(&src_abs));
        let dst_comps = get_components_string(&path_to_str(&dst_abs));

        if src_comps.is_empty() || dst_comps.is_empty() {
            return Err(FsError::InvalidPath("Empty path".to_string()));
        }

        // Locate parent directory of source.
        let src_file_name = src_comps.last().unwrap();
        let src_parent_path = if src_comps.len() == 1 {
            "/".to_string()
        } else {
            format!("/{}", src_comps[..src_comps.len() - 1].join("/"))
        };
        let src_parent_comps = get_components(&src_parent_path);
        let mut src_parent = &mut self.root;
        for comp in src_parent_comps {
            src_parent = src_parent
                .find_dir_mut(comp)
                .ok_or_else(|| FsError::NotFound(format!("Directory {comp} not found")))?;
        }

        // Check if the source is a file.
        if let Some(pos) = src_parent.files.iter().position(|f| {
            let full = path_to_str(&f.path);
            let comps = get_components(&full);
            comps.last().is_some_and(|s| *s == *src_file_name)
        }) {
            let mut file = src_parent.files.remove(pos);
            // Update file path.
            file.path = VirtPath::Absolute(path_to_str(&dst_abs).as_bytes().to_vec());
            // Insert into destination's parent.
            let dst_parent_path = if dst_comps.len() == 1 {
                "/".to_string()
            } else {
                format!("/{}", dst_comps[..dst_comps.len() - 1].join("/"))
            };
            let dst_parent_comps = get_components(&dst_parent_path);
            let mut dst_parent = &mut self.root;
            for comp in dst_parent_comps {
                dst_parent = dst_parent
                    .find_dir_mut(comp)
                    .ok_or_else(|| FsError::NotFound(format!("Directory {comp} not found")))?;
            }
            dst_parent.files.push(file);
            return Ok(());
        }

        // Else, check if the source is a directory.
        if let Some(pos) = src_parent
            .dirs
            .iter()
            .position(|d| d.name() == *src_file_name)
        {
            let mut dir = src_parent.dirs.remove(pos);
            // Update directory path recursively.
            dir.update_path::<String>(path_to_str(&dst_abs));
            let dst_parent_path = if dst_comps.len() == 1 {
                "/".to_string()
            } else {
                format!("/{}", dst_comps[..dst_comps.len() - 1].join("/"))
            };
            let dst_parent_comps = get_components(&dst_parent_path);
            let mut dst_parent = &mut self.root;
            for comp in dst_parent_comps {
                dst_parent = dst_parent
                    .find_dir_mut(comp)
                    .ok_or_else(|| FsError::NotFound(format!("Directory {comp} not found")))?;
            }
            dst_parent.dirs.push(dir);
            return Ok(());
        }
        Err(FsError::NotFound("Source entry not found".to_string()))
    }
}

impl VirtDir {
    /// Get the “name” of this directory (the last component of its path).
    #[must_use]
    pub fn name(&self) -> String {
        let full = path_to_str(&self.path);
        if full == "/" {
            "/".to_string()
        } else {
            let comps = get_components(&full);
            (*comps.last().unwrap_or(&full.as_str())).to_string()
        }
    }

    /// Find a mutable subdirectory with the given name.
    pub fn find_dir_mut(&mut self, name: &str) -> Option<&mut VirtDir> {
        self.dirs.iter_mut().find(|d| d.name() == name)
    }

    /// Find an immutable subdirectory with the given name.
    #[must_use]
    pub fn find_dir(&self, name: &str) -> Option<&VirtDir> {
        self.dirs.iter().find(|d| d.name() == name)
    }

    /// Find a mutable file with the given name.
    pub fn find_file_mut(&mut self, name: &str) -> Option<&mut VirtFile> {
        self.files.iter_mut().find(|f| {
            let full = path_to_str(&f.path);
            let comps = get_components(&full);
            comps.last().is_some_and(|s| *s == name)
        })
    }

    /// Find an immutable file with the given name.
    #[must_use]
    pub fn find_file(&self, name: &str) -> Option<&VirtFile> {
        self.files.iter().find(|f| {
            let full = path_to_str(&f.path);
            let comps = get_components(&full);
            comps.last().is_some_and(|s| *s == name)
        })
    }

    /// Insert a new file into the directory.
    pub fn insert_file(&mut self, file: VirtFile) {
        self.files.push(file);
    }

    /// Recursively update the path of this directory and all its children.
    pub fn update_path<P: Into<VirtPath>>(&mut self, new_path: P) {
        let new_path = new_path.into();
        self.path = new_path.clone();
        for f in &mut self.files {
            // Append the file name to the new directory path.
            let comps = get_components_string(&path_to_str(&f.path));
            if let Some(name) = comps.last() {
                let full = if path_to_str(&new_path) == "/" {
                    format!("/{name}")
                } else {
                    format!("{}/{}", path_to_str(&new_path), name)
                };
                f.path = VirtPath::Absolute(full.as_bytes().to_vec());
            }
        }
        for d in &mut self.dirs {
            let comps = get_components_string(&path_to_str(&d.path));
            if let Some(name) = comps.last() {
                let full = if path_to_str(&new_path) == "/" {
                    format!("/{name}")
                } else {
                    format!("{}/{name}", path_to_str(&new_path))
                };
                d.update_path(VirtPath::Absolute(full.as_bytes().to_vec()));
            }
        }
    }
}

impl Read for VirtFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.cursor >= self.content.len() {
            return Ok(0); // EOF
        }
        let available = self.content.len() - self.cursor;
        let to_read = available.min(buf.len());
        buf[..to_read].copy_from_slice(&self.content[self.cursor..self.cursor + to_read]);
        self.cursor += to_read;
        Ok(to_read)
    }
}

impl Write for VirtFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // If the cursor is beyond current content, pad with zeros.
        if self.cursor > self.content.len() {
            self.content.resize(self.cursor, 0);
        }
        let end = self.cursor + buf.len();
        if end > self.content.len() {
            self.content.resize(end, 0);
        }
        self.content[self.cursor..end].copy_from_slice(buf);
        self.cursor = end;
        // Update the modified timestamp.
        self.metadata.modified = SystemTime::now();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Seek for VirtFile {
    #[allow(clippy::cast_possible_wrap)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => self.cursor as i64 + offset,
            SeekFrom::End(offset) => self.content.len() as i64 + offset,
        };
        if new_pos < 0 {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid seek",
            ))
        } else {
            self.cursor = new_pos as usize;
            Ok(self.cursor as u64)
        }
    }
}
