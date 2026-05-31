//! Secure file utilities for SteelSeries GG.

use crate::{Error, Result};
use std::fs::{File, OpenOptions};
use std::path::Path;

/// Open a file securely, preventing symlink attacks on Unix.
pub fn secure_open<P: AsRef<Path>>(path: P, options: &OpenOptions) -> Result<File> {
    let path = path.as_ref();

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        // Refuse to operate on an existing symlink.
        if let Ok(metadata) = std::fs::symlink_metadata(path) {
            if metadata.file_type().is_symlink() {
                return Err(Error::FileSystemError(format!(
                    "Refusing to operate on {} because it is a symlink",
                    path.display()
                )));
            }
        }

        let mut secure_options = options.clone();
        secure_options.custom_flags(libc::O_NOFOLLOW);

        let file = match secure_options.open(path) {
            Ok(file) => file,
            Err(err) => {
                if err.raw_os_error() == Some(libc::ELOOP) {
                    return Err(Error::FileSystemError(format!(
                        "Refusing to operate on {} because it is (or contains) a symlink",
                        path.display()
                    )));
                }
                return Err(err.into());
            }
        };
        Ok(file)
    }

    #[cfg(not(unix))]
    {
        options.open(path).map_err(|e| e.into())
    }
}

/// Write data to a file securely, preventing symlink attacks on Unix.
pub fn secure_write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    let path = path.as_ref();
    let contents = contents.as_ref();

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = secure_open(path, &options)?;

    use std::io::Write;
    file.write_all(contents)?;

    Ok(())
}

/// Asynchronously write data to a file securely.
pub async fn secure_write_async<P: AsRef<Path> + Send + 'static, C: AsRef<[u8]> + Send + 'static>(
    path: P,
    contents: C,
) -> Result<()> {
    tokio::task::spawn_blocking(move || secure_write(path, contents))
        .await
        .map_err(|e| Error::Other(format!("Task join error: {}", e)))?
}
