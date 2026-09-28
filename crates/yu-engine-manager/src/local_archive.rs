//! Explicit offline input for the existing verified installer; never a network fallback.
use crate::{DEFAULT_MAX_DOWNLOAD_BYTES, Downloader, ManagerError};
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

pub struct LocalArchiveDownloader {
    source: PathBuf,
    expected_url: String,
    max_bytes: u64,
}

impl LocalArchiveDownloader {
    /// Bind an explicitly supplied archive to the selected manifest package.
    /// EngineInstaller still verifies the staged bytes against the manifest SHA-256.
    pub fn new(source: impl Into<PathBuf>, expected_url: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            expected_url: expected_url.into(),
            max_bytes: DEFAULT_MAX_DOWNLOAD_BYTES,
        }
    }
}

impl Downloader for LocalArchiveDownloader {
    fn download(&self, url: &str, destination: &Path) -> Result<u64, ManagerError> {
        if url != self.expected_url {
            return Err(ManagerError::Download(
                "local archive does not match the selected package URL".into(),
            ));
        }
        let metadata = fs::symlink_metadata(&self.source).map_err(|e| {
            ManagerError::Io(format!(
                "cannot inspect local archive {}: {e}",
                self.source.display()
            ))
        })?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(ManagerError::Download(
                "local archive must be a regular, non-symlink file".into(),
            ));
        }
        let source = fs::File::open(&self.source)
            .map_err(|e| ManagerError::Io(format!("cannot open local archive: {e}")))?;
        let metadata = source
            .metadata()
            .map_err(|e| ManagerError::Io(e.to_string()))?;
        if !metadata.is_file() || metadata.len() > self.max_bytes {
            return Err(ManagerError::Download(format!(
                "local archive exceeds {} bytes or is not regular",
                self.max_bytes
            )));
        }
        // Never reuse a caller's existing staging file. Bound the stream as well as stat,
        // so growth during the copy cannot bypass the normal download-size budget.
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|e| ManagerError::Io(format!("cannot create local archive staging: {e}")))?;
        let count = io::copy(&mut source.take(self.max_bytes.saturating_add(1)), &mut out)
            .map_err(|e| ManagerError::Io(format!("cannot stage local archive: {e}")))?;
        if count > self.max_bytes {
            return Err(ManagerError::Download(format!(
                "local archive exceeds {} bytes",
                self.max_bytes
            )));
        }
        out.sync_all()
            .map_err(|e| ManagerError::Io(format!("cannot sync local archive staging: {e}")))?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "yu-local-archive-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn local_archive_is_bounded_and_never_overwrites_staging() {
        let root = Root::new();
        let source = root.0.join("source");
        let out = root.0.join("staged");
        fs::write(&source, b"1234").unwrap();
        let downloader = LocalArchiveDownloader {
            source: source.clone(),
            expected_url: "https://example.invalid/a".into(),
            max_bytes: 4,
        };
        assert_eq!(
            downloader.download(&downloader.expected_url, &out).unwrap(),
            4
        );
        assert_eq!(fs::read(&source).unwrap(), b"1234");
        assert!(downloader.download(&downloader.expected_url, &out).is_err());
        assert_eq!(fs::read(&out).unwrap(), b"1234");
        fs::write(&source, b"12345").unwrap();
        assert!(
            downloader
                .download(&downloader.expected_url, &root.0.join("oversized"))
                .is_err()
        );
        assert!(!root.0.join("oversized").exists());
    }

    #[test]
    fn local_archive_rejects_wrong_binding_and_non_files() {
        let root = Root::new();
        let out = root.0.join("staged");
        let downloader = LocalArchiveDownloader::new(&root.0, "https://example.invalid/a");
        assert!(
            downloader
                .download("https://example.invalid/b", &out)
                .is_err()
        );
        assert!(downloader.download(&downloader.expected_url, &out).is_err());
        assert!(!out.exists());
    }

    #[cfg(unix)]
    #[test]
    fn local_archive_rejects_symlinks() {
        let root = Root::new();
        let source = root.0.join("source");
        let link = root.0.join("link");
        fs::write(&source, b"1234").unwrap();
        std::os::unix::fs::symlink(&source, &link).unwrap();
        let downloader = LocalArchiveDownloader::new(link, "https://example.invalid/a");
        assert!(
            downloader
                .download(&downloader.expected_url, &root.0.join("staged"))
                .is_err()
        );
    }
}
