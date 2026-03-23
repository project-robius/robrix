use std::{sync::OnceLock, path::PathBuf};

/// Creates and returns the path to a temp directory for storage.
///
/// This is very efficient to call multiple times because the result is cached
/// after the first call creates the temp directory.
pub fn get_temp_dir_path() -> &'static PathBuf {
    const TEMP_DIR_NAME: &str = "robrix_temp";
    static TEMP_DIR_PATH: OnceLock<PathBuf> = OnceLock::new();

    TEMP_DIR_PATH.get_or_init(|| {
        let mut path = std::env::temp_dir();
        path.push(TEMP_DIR_NAME);
        std::fs::create_dir_all(&path).expect("Failed to create temp dir: {path}");
        path
    })
}
