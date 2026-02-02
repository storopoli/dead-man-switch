use crate::error::HomeDirError;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::{process, thread};

use directories_next::BaseDirs;

/// Returns the name of the app
pub fn name() -> &'static str {
    "deadman"
}

/// Returns the path for the app dir (within an OS-agnostic home directory)
///
/// Under the hood uses the [`directories_next`] crate to find the
/// home directory.
///
/// # Errors
///
/// - Fails if the home directory cannot be found
/// - Fails if the app directory cannot be created
///
/// # Notes
///
/// This function handles testing and non-testing environments.
///
fn dir() -> Result<PathBuf, HomeDirError> {
    let base_dir = if cfg!(test) {
        // Use a temporary directory approach (for each test)
        let thread_id = format!("{:?}", thread::current().id());
        let test_dir = env::temp_dir()
            .join(format!("{}_test", name()))
            .join(format!(
                "{}-{}",
                process::id(),
                thread_id.replace([':', '(', ')'], "-")
            ));
        fs::create_dir_all(&test_dir).expect("failed to create test directory");
        test_dir
    } else {
        BaseDirs::new()
            .ok_or(HomeDirError::HomeDirNotFound)?
            .config_dir()
            .to_path_buf()
    };

    let dir = base_dir.join(name());

    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Returns the path for a file in the app directory
pub fn file_path(file_name: &str) -> Result<PathBuf, HomeDirError> {
    Ok(dir()?.join(file_name))
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::Path;

    // helper
    fn cleanup_test_dir(dir: &Path) {
        if let Some(parent) = dir.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    // helper
    fn cleanup_test_dir_parent(dir: &Path) {
        if let Some(parent) = dir.parent() {
            cleanup_test_dir(parent)
        }
    }

    #[test]
    fn file_path_in_test_mode() {
        // This test verifies that file_path() uses temp directory in test mode
        let file_name = "test.txt";
        let result = file_path(file_name);
        assert!(result.is_ok());

        let result = result.unwrap();
        let expected = format!("{}_test", name());
        assert!(result.to_string_lossy().contains(expected.as_str()));

        // It should also contain the actual file name
        let expected = Path::new(name()).join(file_name);
        assert!(result
            .to_string_lossy()
            .contains(expected.to_string_lossy().as_ref()));

        // Cleanup any created directories
        cleanup_test_dir_parent(&result);
    }
}
