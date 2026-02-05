use std::path::Path;

/// Writes the given content to a file at the given path, ensuring that the file
/// is only readable/writable by the current user (permission 0o600 on Unix).
///
/// This uses the `tempfile` crate to perform an atomic write:
/// 1. A temporary file is created in the same directory (inheriting secure defaults, e.g. 0600 on Unix).
/// 2. Content is written to the temporary file.
/// 3. The temporary file is atomically renamed to the target path.
///
/// This implementation is platform-agnostic and avoids platform-specific `cfg` blocks.
pub async fn write_to_file_securely<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    content: C,
) -> std::io::Result<()> {
    let path = path.as_ref().to_owned();
    let content = content.as_ref().to_vec();

    tokio::task::spawn_blocking(move || {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        // Create a temporary file in the same directory to ensure atomic move/persist works.
        // `tempfile::Builder` defaults to secure permissions (0600 on Unix).
        let mut temp = tempfile::Builder::new()
            .prefix(".tmp")
            .tempfile_in(parent)?;

        use std::io::Write;
        temp.write_all(&content)?;

        // Persist the temporary file to the final destination.
        // This performs an atomic rename.
        temp.persist(path).map_err(|e| e.error)?;

        Ok(())
    }).await.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
}
