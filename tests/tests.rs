#[cfg(test)]
mod tests {
    use tempfs::temp_dir::TempDir;
    use tempfs::temp_file::TempFile;
    use std::env;
    use std::fs;
    use std::io::{Read, Seek, SeekFrom, Write};

    #[test]
    fn test_temp_file_create_write_read() {
        // Create a temporary file in the system temp directory.
        let temp_path = env::temp_dir().join("test_temp_file.txt");
        {
            let mut temp_file = TempFile::new(&temp_path).expect("Failed to create TempFile");
            let data = b"Hello, TempFile!";
            temp_file.write_all(data).expect("Failed to write data");
            // Sync the file (optional)
            temp_file.sync_all().expect("Failed to sync data");
            // Navigate back to the beginning of the file before reading.
            temp_file.seek(SeekFrom::Start(0)).expect("Failed to seek");
            let mut content = Vec::new();
            let am = temp_file
                .read_to_end(&mut content)
                .expect("Failed to read data");
            println!("am: {am}");
            assert_eq!(data, &content[..]);
            // Persist the file so it won’t be auto-deleted.
            let file = temp_file.persist().expect("Failed to persist file");
            drop(file);
        }
        // Verify that the file exists and then remove it.
        assert!(temp_path.exists());
        fs::remove_file(&temp_path).expect("Failed to remove persistent file");
    }

    #[test]
    fn test_temp_file_rename() {
        let temp_path = env::temp_dir().join("test_temp_file_rename.txt");
        let new_path = env::temp_dir().join("test_temp_file_rename_new.txt");
        {
            let mut temp_file = TempFile::new(&temp_path).expect("Failed to create TempFile");
            temp_file.rename(new_path.to_str().unwrap()).expect("Rename failed");
            assert_eq!(temp_file.path().unwrap(), new_path.as_path());
            let file = temp_file.persist().expect("Persist failed");
            drop(file);
        }
        assert!(new_path.exists());
        fs::remove_file(&new_path).expect("Failed to remove renamed file");
    }

    #[test]
    fn test_temp_dir_create_and_file() {
        let temp_dir_path = env::temp_dir().join("test_temp_dir");
        {
            let mut temp_dir = TempDir::new(&temp_dir_path).expect("Failed to create TempDir");
            {
                let file = temp_dir.create_file("file1.txt").expect("Failed to create file");
                file.write_all(b"Content").expect("Failed to write to file");
            }
            let files = temp_dir.list_files();
            assert_eq!(files.len(), 1);
            let file_ref = temp_dir.get_file("file1.txt").expect("File not found");
            assert!(file_ref.path().is_some());
        }
        // After drop, the directory should no longer exist.
        assert!(!temp_dir_path.exists());
    }

    #[cfg(feature = "regex_support")]
    #[test]
    fn test_temp_dir_find_files_by_pattern() {
        let temp_dir_path = env::temp_dir().join("test_temp_dir_regex");
        {
            let mut temp_dir = TempDir::new(&temp_dir_path).expect("Failed to create TempDir");
            temp_dir.create_file("match.txt").expect("Failed to create file");
            temp_dir.create_file("nomatch.log").expect("Failed to create file");
            let matches = temp_dir.find_files_by_pattern(".*\\.txt").expect("Regex error");
            assert_eq!(matches.len(), 1);
            let file_name = matches[0]
                .path()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap();
            assert_eq!(file_name, "match.txt");
        }
        assert!(!temp_dir_path.exists());
    }
}
