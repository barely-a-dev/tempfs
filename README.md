# tempfs

`tempfs` is a lightweight Rust crate that provides utilities for managing temporary files and directories. It makes working with temporary resources easier by automatically cleaning up files and directories when they go out of scope. The crate offers a flexible API with optional support for features such as random name generation, memory mapping, and regex-based file filtering.

## Features

- **Temporary Directory (`TempDir`):**  
  Create and manage a temporary directory whose contents are automatically removed when the directory is dropped.
  
- **Temporary File (`TempFile`):**  
  Create temporary files with support for writing, reading, renaming, persisting, and even memory mapping (if enabled).

- **Optional Feature Flags:**
  - **`rand_gen`**: Enables random name generation for temporary files and directories. *(Requires the `rand` dependency.)*
  - **`mmap_support`**: Enables memory mapping of temporary files via the `memmap2` crate.
  - **`regex_support`**: Enables regex-based filtering and searching of temporary files using the `regex` crate.
  - **`full`**: Activates all optional features at once.

## Installation

Add `tempfs` to your `Cargo.toml` manually or use `cargo add tempfs [-F <feature>]*`.

## Usage

Below is a simple example demonstrating how to create a temporary directory and file:

```rust
use tempfs::{TempDir, TempFile};
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory at the specified path.
    let mut temp_dir = TempDir::new("temp_directory")?;

    // Create a temporary file within the directory.
    let mut temp_file = temp_dir.create_file("example.txt")?;

    // Write data to the temporary file.
    writeln!(temp_file, "Hello, tempfs!")?;

    // Optionally, persist the file to prevent deletion.
    let _persisted_file = temp_file.close()?;

    // The temporary directory will clean up any remaining temporary files on drop.
    Ok(())
}
```

## Advanced Usage

- **Random Naming:**  
  If you enable the `rand_gen` feature, you can use methods like `TempDir::random` and `TempFile::new_random` to create temporary resources with random names.

- **Regex-Based Filtering:**  
  When the `regex_support` feature is enabled, you can filter temporary files using `TempDir::find_files_by_pattern` or its mutable counterpart.

- **Memory Mapping:**  
  With the `mmap_support` feature enabled, you can create memory maps of temporary files using `TempFile::mmap` and `TempFile::mmap_mut`.

## Documentation

Full API documentation is available on [docs.rs](https://docs.rs/tempfs).

## License

This project is dual licensed under the MIT and Apache 2.0 Licenses. See the [MIT license](LICENSE-MIT) and [Apache license](LICENSE-APACHE-2.0) files for details.

## Contributing

Contributions, issues, and feature requests are welcome! Please check the [issues page](https://github.com/barely-a-dev/tempfs/issues) for existing issues before creating new ones. Pull requests are also welcome.
