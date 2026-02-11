use std::path::Path;
use anyhow::{Context, Result};
use rand::Rng;
use tokio::io::AsyncWriteExt;

/// Writes the given `content` to the file at `path` in a secure and atomic manner.
///
/// This function:
/// 1. Creates a temporary file in the same directory as the target `path`.
/// 2. Sets the file permissions to `0o600` (read/write only for the owner) on Unix systems.
/// 3. Writes the content to the temporary file.
/// 4. Atomically renames the temporary file to the target `path`.
///
/// This prevents partial writes (corruption) and ensures that sensitive data is not readable by other users.
pub async fn write_file_securely(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));

    // Create a temporary file name with a random suffix.
    let random_suffix: u32 = {
        let mut rng = rand::thread_rng();
        rng.r#gen()
    };
    let temp_filename = format!(
        "{}.tmp.{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        random_suffix
    );
    let temp_path = parent.join(temp_filename);

    // Open the file with restrictive permissions on creation if possible.
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);

    #[cfg(unix)]
    {
        options.mode(0o600);
    }

    let mut file = options.open(&temp_path).await
        .with_context(|| format!("Failed to create temp file {}", temp_path.display()))?;

    file.write_all(content.as_ref()).await
        .with_context(|| format!("Failed to write to temp file {}", temp_path.display()))?;

    file.flush().await
        .with_context(|| format!("Failed to flush temp file {}", temp_path.display()))?;

    // Ensure the file is closed before renaming (on Windows especially).
    // Dropping `file` closes it, but async close is better if supported.
    // Tokio `File` creates an async wrapper around std `File` (blocking).
    // Dropping it closes the underlying fd.
    drop(file);

    // Rename (atomic replace).
    tokio::fs::rename(&temp_path, path).await
        .with_context(|| format!("Failed to rename {} to {}", temp_path.display(), path.display()))?;

    Ok(())
}
