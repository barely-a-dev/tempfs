use std::env;
use std::path::{Component, Path, PathBuf};

/// A helper function to normalize a path without touching the filesystem.
/// It removes redundant `.` components and resolves `..` without following symlinks.
pub fn normalize_path<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    let mut normalized = PathBuf::new();
    for (i, comp) in path.components().enumerate() {
        match comp {
            Component::CurDir => {
                if i == 0 {
                    normalized.push(env::current_dir().unwrap());
                }
            }
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(comp.as_os_str()),
        }
    }
    normalized
}

/// Returns the path up to the first nonexistent directory component, or None if it all exists except the final component.
pub fn first_missing_directory_component(path: &Path) -> Option<PathBuf> {
    // Get the parent directory (ignore the final component)
    let parent = path.parent()?;
    let mut cumulative = PathBuf::new();

    // Iterate over each component in the parent path.
    for component in parent.components() {
        cumulative.push(component.as_os_str());
        if !cumulative.exists() {
            return Some(cumulative);
        }
    }
    None
}
