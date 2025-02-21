use std::io::{Read, Seek, SeekFrom, Write};
use tempfs::{TempFile, TempError};

fn main() -> Result<(), TempError> {
    // Create a temporary file with a random name in the system's temp directory.
    #[cfg(feature = "rand_gen")]
    let mut temp_file = TempFile::new_random::<std::path::PathBuf>(None)?;
    #[cfg(not(feature = "rand_gen"))]
    let mut temp_file = TempFile::new("./tmp/hello.txt")?;

    // Write some data to the temporary file.
    write!(temp_file, "Hello, temporary world!")?;

    // Move back to the start of the file before reading.
    temp_file.seek(SeekFrom::Start(0))?;

    // Read the file content into a string.
    let mut content = String::new();
    temp_file.read_to_string(&mut content)?;
    println!("Temp file content: {content}");

    // Rename the file (for example, to "output.txt") and persist it,
    // so that it is not deleted when `temp_file` is dropped.
    let _permanent_file = temp_file.persist_here("output.txt")?;
    println!("Temporary file persisted as output.txt");

    Ok(())
}
