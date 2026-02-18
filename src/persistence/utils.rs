use std::path::Path;
use anyhow::{Context, Result};
use rand::{distributions::Alphanumeric, Rng};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

/// Writes data to a file securely and atomically.
///
/// This function:
/// 1. Creates a temporary file in the same directory as the target file.
/// 2. Sets restrictive permissions (0600 on Unix) on the temporary file.
/// 3. Writes the data to the temporary file.
/// 4. Atomically renames the temporary file to the target file.
///
/// This ensures that the file is never partially written or accessible to other users
/// during the write process.
pub async fn write_file_securely(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));

    // Create parent directory if it doesn't exist
    if !parent.exists() {
        fs::create_dir_all(parent).await.context("Failed to create parent directory")?;
    }

    // Generate a random temporary filename
    let temp_name: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    let temp_path = parent.join(format!(".{}.tmp", temp_name));

    // Configure OpenOptions for secure creation
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);

    #[cfg(unix)]
    options.mode(0o600); // Read/write only for owner

    // Write content to temporary file
    let mut file = options.open(&temp_path).await.context("Failed to open temp file")?;
    file.write_all(content.as_ref()).await.context("Failed to write to temp file")?;
    file.flush().await.context("Failed to flush temp file")?;

    // Ensure data is synced to disk
    file.sync_all().await.context("Failed to sync temp file")?;

    // Atomically rename
    fs::rename(&temp_path, path).await.context("Failed to rename temp file to target")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_write_file_securely() -> Result<()> {
        let mut dir = env::temp_dir();
        // Use a random dir name to avoid conflicts
        let dir_name: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(10)
            .map(char::from)
            .collect();
        dir.push(format!("robrix_test_{}", dir_name));

        let _ = tokio::fs::create_dir_all(&dir).await;

        let file_path = dir.join("secret.txt");
        let content = b"secret data";

        write_file_securely(&file_path, content).await?;

        assert!(file_path.exists());
        let read_content = tokio::fs::read(&file_path).await?;
        assert_eq!(read_content, content);

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = std::fs::metadata(&file_path)?;
            let mode = metadata.mode();
            assert_eq!(mode & 0o777, 0o600, "File permissions should be 0600");
        }

        // Cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
        let _ = tokio::fs::remove_dir(&dir).await;

        Ok(())
    }
}
