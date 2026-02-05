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
        let parent = path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        // Create a temporary file in the same directory to ensure atomic move/persist works.
        // `tempfile::Builder` defaults to secure permissions (0600 on Unix).
        let mut temp = tempfile::Builder::new()
            .prefix(".tmp")
            .tempfile_in(parent)?;

        use std::io::Write;
        temp.write_all(&content)?;

        // Persist the temporary file to the final destination.
        // On Windows, `persist` (which uses `fs::rename`) fails if the target exists.
        // We attempt to remove the target file first if persistence fails due to existence.
        // This makes the operation slightly less atomic on Windows but ensures it succeeds.
        match temp.persist(&path) {
            Ok(_) => Ok(()),
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists ||
                      // Windows specific error code for "Cannot create a file when that file already exists"
                      // or similar errors that might manifest as PermissionDenied or Other on some setups
                      (cfg!(windows) && e.error.kind() == std::io::ErrorKind::PermissionDenied) => {
                // If the target exists, we must remove it first on Windows.
                // We do this check regardless of platform if the error suggests existence conflict,
                // though strictly `fs::rename` is atomic overwrite on POSIX.
                // However, `persist` implementation details might vary.

                // If we are here, we still own the temp file (in `e.file`).
                let temp = e.file;

                // Remove the target file.
                if let Err(rm_err) = std::fs::remove_file(&path) {
                    if rm_err.kind() != std::io::ErrorKind::NotFound {
                        return Err(rm_err);
                    }
                }

                // Try persisting again.
                temp.persist(&path).map_err(|e| e.error)?;
                Ok(())
            },
            Err(e) => Err(e.error),
        }
    }).await.map_err(std::io::Error::other)?
}
