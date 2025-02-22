use std::io::{Seek, SeekFrom, Write};
use tempfs::{TempError, TempFile};

fn main() -> Result<(), TempError> {
    // Create a temporary file at a given path.
    let mut temp_file = TempFile::new("mmap_example.txt")?;
    write!(temp_file, "This is a memory-mapped file example")?;
    temp_file.seek(SeekFrom::Start(0))?;

    // Create a read-only memory mapping.
    #[cfg(feature = "mmap_support")]
    unsafe {
        let mmap = temp_file.mmap()?;
        let content = std::str::from_utf8(&mmap)
            .unwrap_or("Invalid UTF-8 sequence");
        println!("Memory-mapped content: {content}");
    }

    Ok(())
}
