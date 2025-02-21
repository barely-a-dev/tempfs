use std::io::Write;
use tempfs::{TempDir, TempError};

fn main() -> Result<(), TempError> {
    // Create a temporary directory with a random name.
    let mut temp_dir = TempDir::new_random::<std::path::PathBuf>(None)?;

    // Create a temporary file with a specific name.
    {
        let file = temp_dir.create_file("test1.txt")?;
        write!(file, "Content for test1")?;
    }

    // Create another temporary file with a random name.
    {
        let file = temp_dir.create_random_file()?;
        write!(file, "Random file content")?;
    }

    // List all the temporary files managed by the directory.
    for file_path in temp_dir.list_files() {
        println!("Managed temp file: {:?}", file_path);
    }

    // If the library was built with regex support, search for files matching a pattern.
    #[cfg(feature = "regex_support")]
    {
        let matching_files = temp_dir.find_files_by_pattern(r"^test\d\.txt$")?;
        for file in matching_files {
            if let Some(path) = file.path() {
                println!("Found file matching regex: {:?}", path);
            }
        }
    }

    // When `temp_dir` goes out of scope, the directory and all its managed files are automatically deleted.
    Ok(())
}
