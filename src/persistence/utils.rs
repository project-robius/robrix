use std::path::Path;
use tokio::io::AsyncWriteExt;

/// Writes the given content to a file at the given path, ensuring that the file
/// is only readable/writable by the current user (permission 0o600 on Unix).
///
/// On non-Unix platforms, this falls back to the default file permissions
/// (usually inherited from the directory or user profile).
pub async fn write_to_file_securely<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    content: C,
) -> std::io::Result<()> {
    let path = path.as_ref();
    let content = content.as_ref();

    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);

    #[cfg(unix)]
    {
        // On Unix, `tokio::fs::OpenOptions` has an inherent `mode` method
        // in recent versions, which makes this trait import unused.
        // However, we import it to ensure compatibility if the inherent method is missing,
        // and allow unused_imports to suppress warnings if it is present.
        #[allow(unused_imports)]
        use std::os::unix::fs::OpenOptionsExt;

        // We set it to 0o600 (read/write for owner only).
        options.mode(0o600);
    }

    let mut file = options.open(path).await?;

    #[cfg(unix)]
    {
        // Explicitly set permissions to handle cases where the file already existed
        // with insecure permissions (OpenOptions::mode only applies to creation).
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        file.set_permissions(perms).await?;
    }

    file.write_all(content).await?;
    Ok(())
}
