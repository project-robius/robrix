use std::{path::{Path, PathBuf}, sync::OnceLock, time::{Duration, SystemTime}};
use makepad_widgets::error;
use crate::cache_dir;

/// Subdirectory of the app cache dir used for short-lived scratch files.
const TEMP_SUBDIR: &str = "temp";

/// How long to wait after startup before running the temp dir cleanup task.
const CLEANUP_DELAY: Duration = Duration::from_secs(60);

/// Creates and returns the path to an app-local temp directory,
/// a subdirectory within the platform-designated cache dir for this app.
///
/// This is cheap to repeatedly call, it only does the directory work once.
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

/// Schedules a task to clear leftover temp files from previous app runs.
///
/// We do it later to avoid interfering with important operations at startup, like sync.
pub fn schedule_temp_dir_cleanup() {
    let path = cache_dir().join(TEMP_SUBDIR);
    let cutoff = SystemTime::now();
    std::thread::spawn(move || {
        std::thread::sleep(CLEANUP_DELAY);
        remove_files_older_than(&path, cutoff);
    });
}

/// Recursively deletes files last modified before `cutoff`, then any empty dirs.
fn remove_files_older_than(dir: &Path, cutoff: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            remove_files_older_than(&path, cutoff);
            let _ = std::fs::remove_dir(&path); // only succeeds once it's empty
        } else if meta.modified().is_ok_and(|m| m < cutoff) {
            let _ = std::fs::remove_file(&path);
        }
    }
}
