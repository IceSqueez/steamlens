use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum CacheIoError {
    #[error("I/O error writing cache: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization failed: {0}")]
    #[allow(dead_code)]
    Serialize(String),
}

/// Atomically writes `bytes` to `path`.
///
/// The write goes to `<path>.tmp` in the same directory, the file is
/// `sync_data()`-d, then renamed over `path`. On POSIX this rename is atomic;
/// on Windows 10+ NTFS the move-file-ex replaces the destination atomically.
/// Keeping the `.tmp` file on the same filesystem as the target guarantees the
/// rename never crosses a mount boundary.
pub async fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), CacheIoError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let tmp_path = tmp_path_for(path);

    {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        file.write_all(bytes).await?;
        file.sync_data().await?;
    }

    tokio::fs::rename(&tmp_path, path).await?;

    Ok(())
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".tmp");
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "steamlens_cache_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[tokio::test]
    async fn atomic_write_creates_file_with_expected_bytes() {
        let dir = tempdir();
        let target = dir.join("test.bin");
        let payload = b"hello steamlens";

        atomic_write(&target, payload).await.expect("write");

        let read_back = std::fs::read(&target).expect("read");
        assert_eq!(read_back, payload);

        let tmp = dir.join("test.bin.tmp");
        assert!(!tmp.exists(), ".tmp file must not remain after rename");
    }

    #[tokio::test]
    async fn atomic_write_overwrites_existing_file() {
        let dir = tempdir();
        let target = dir.join("overwrite.bin");
        std::fs::write(&target, b"old content").expect("setup");

        atomic_write(&target, b"new content").await.expect("write");

        let read_back = std::fs::read(&target).expect("read");
        assert_eq!(read_back, b"new content");

        let tmp = dir.join("overwrite.bin.tmp");
        assert!(!tmp.exists(), ".tmp file must not remain");
    }

    #[tokio::test]
    async fn atomic_write_creates_parent_dirs() {
        let dir = tempdir();
        let target = dir.join("nested").join("deep").join("file.bin");

        atomic_write(&target, b"nested").await.expect("write");

        let read_back = std::fs::read(&target).expect("read");
        assert_eq!(read_back, b"nested");
    }
}
