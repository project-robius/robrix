use std::{path::{Path, PathBuf}, sync::OnceLock, time::{Duration, SystemTime}};
use makepad_widgets::error;
use crate::cache_dir;

/// Subdirectory of the app cache dir used for short-lived scratch files.
const TEMP_SUBDIR: &str = "temp";

/// How long to wait after startup before clearing the temp dir, so cleanup
/// doesn't compete with sync and other startup work.
const CLEANUP_DELAY: Duration = Duration::from_secs(60);

/// Creates and returns the path to an app-scoped temp directory for scratch files
/// (e.g. attachments staged for the native share sheet, which needs a real path).
///
/// Efficient to call repeatedly: the directory is created once and the path cached.
pub fn get_temp_dir_path() -> &'static PathBuf {
    static TEMP_DIR_PATH: OnceLock<PathBuf> = OnceLock::new();
    TEMP_DIR_PATH.get_or_init(|| {
        let path = cache_dir().join(TEMP_SUBDIR);
        if let Err(e) = std::fs::create_dir_all(&path) {
            error!("Failed to create temp dir {}: {e}", path.display());
        }
        path
    })
}

/// Clears leftover temp files from previous runs, `CLEANUP_DELAY` after startup so it
/// stays off the busy startup path. Files staged for sharing this run are kept.
pub fn schedule_temp_dir_cleanup() {
    let path = cache_dir().join(TEMP_SUBDIR);
    let cutoff = SystemTime::now();
    std::thread::spawn(move || {
        std::thread::sleep(CLEANUP_DELAY);
        remove_files_before(&path, cutoff);
    });
}

/// Recursively deletes files last modified before `cutoff`, then any dirs left empty.
fn remove_files_before(dir: &Path, cutoff: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            remove_files_before(&path, cutoff);
            let _ = std::fs::remove_dir(&path); // only succeeds once it's empty
        } else if meta.modified().is_ok_and(|m| m < cutoff) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_pre_cutoff_files() {
        let base = std::env::temp_dir().join(format!("robrix_cleanup_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (old, new) = (base.join("old_mxc"), base.join("new_mxc"));

        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("a.bin"), b"old").unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        let cutoff = SystemTime::now();
        std::thread::sleep(Duration::from_millis(1100));
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("b.bin"), b"new").unwrap();

        remove_files_before(&base, cutoff);

        assert!(!old.exists(), "stale subdir should be removed");
        assert!(new.join("b.bin").exists(), "file created this run should be kept");
        let _ = std::fs::remove_dir_all(&base);
    }
}
