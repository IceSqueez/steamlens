use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use memmap2::{Mmap, MmapMut};
use tempfile::{Builder, NamedTempFile};

#[derive(Debug, thiserror::Error)]
pub enum ShmError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("write of {wanted} bytes exceeds region capacity {capacity}")]
    Overflow { wanted: usize, capacity: usize },

    #[error("postcard serialize: {0}")]
    Serialize(String),

    #[error("postcard deserialize: {0}")]
    Deserialize(String),

    #[error("shm size mismatch: expected {expected} bytes, got {actual}")]
    SizeMismatch { expected: u64, actual: usize },
}

pub fn unlink_at(path: &Path) {
    if let Ok(reader) = ShmReader::open(path) {
        let _ = reader.unlink();
    }
}

pub fn write_payload<T: serde::Serialize>(value: &T) -> Result<(PathBuf, u64), ShmError> {
    let payload = postcard::to_allocvec(value).map_err(|e| ShmError::Serialize(e.to_string()))?;
    let mut writer = ShmWriter::create(payload.len())?;
    writer.write(&payload)?;
    let path = writer.into_path()?;
    Ok((path, payload.len() as u64))
}

/// Removes `/dev/shm/steamlens-*` (or fallback `temp_dir`) files older than
/// 60 seconds, picking up orphans from crashed sessions. Files newer than
/// the cutoff are left alone in case another concurrent SteamLens instance
/// is mid-flight. Call once at app startup before any worker spawns.
pub fn sweep_orphans() -> usize {
    sweep_orphans_in(&shm_dir(), Duration::from_secs(60))
}

fn sweep_orphans_in(dir: &Path, min_age: Duration) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let now = std::time::SystemTime::now();
    let mut count = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("steamlens-") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(mtime) = meta.modified() else {
            continue;
        };
        let Ok(age) = now.duration_since(mtime) else {
            continue;
        };
        if age < min_age {
            continue;
        }
        if std::fs::remove_file(entry.path()).is_ok() {
            count += 1;
        }
    }
    count
}

pub fn read_payload<T: serde::de::DeserializeOwned>(
    path: &Path,
    expected_bytes: u64,
) -> Result<T, ShmError> {
    let reader = ShmReader::open(path)?;
    let actual = reader.as_bytes().len();
    if actual != expected_bytes as usize {
        let _ = reader.unlink();
        return Err(ShmError::SizeMismatch {
            expected: expected_bytes,
            actual,
        });
    }
    let result =
        postcard::from_bytes(reader.as_bytes()).map_err(|e| ShmError::Deserialize(e.to_string()));
    if let Err(unlink_err) = reader.unlink() {
        eprintln!("[steamlens] shm unlink failed: {unlink_err}");
    }
    result
}

pub struct ShmWriter {
    file: NamedTempFile,
    mmap: MmapMut,
    capacity: usize,
}

impl ShmWriter {
    pub fn create(size: usize) -> Result<Self, ShmError> {
        let file = Builder::new()
            .prefix("steamlens-")
            .tempfile_in(shm_dir())?;
        file.as_file().set_len(size as u64)?;
        // SAFETY: see RFC-004 §Phase A. NamedTempFile is owned exclusively by
        // this ShmWriter; the path has not been exposed to any other process,
        // so the file cannot be modified out-of-band during the mapping.
        let mmap = unsafe { MmapMut::map_mut(file.as_file())? };
        Ok(Self {
            file,
            mmap,
            capacity: size,
        })
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), ShmError> {
        if bytes.len() > self.capacity {
            return Err(ShmError::Overflow {
                wanted: bytes.len(),
                capacity: self.capacity,
            });
        }
        self.mmap[..bytes.len()].copy_from_slice(bytes);
        self.mmap.flush()?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    /// Persists the temp file (disables auto-delete on drop) and returns its
    /// path. Caller is responsible for sending the path to the reader and
    /// for reader-side cleanup via [`ShmReader::unlink`].
    pub fn into_path(self) -> Result<PathBuf, ShmError> {
        let ShmWriter { file, mmap, .. } = self;
        drop(mmap);
        file.keep()
            .map(|(_, path)| path)
            .map_err(|e| ShmError::Io(e.error))
    }
}

pub struct ShmReader {
    file: File,
    mmap: Mmap,
    path: PathBuf,
}

impl ShmReader {
    pub fn open(path: &Path) -> Result<Self, ShmError> {
        let file = File::open(path)?;
        // SAFETY: see RFC-004 §Phase A. The producer has flushed and dropped
        // its mmap before sending us this path; no other process is expected
        // to mutate the file during our read.
        let mmap = unsafe { Mmap::map(&file)? };
        Ok(Self {
            file,
            mmap,
            path: path.to_path_buf(),
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap
    }

    /// Drops the mapping, closes the fd, removes the underlying file. One-shot:
    /// consumes `self` so the caller cannot accidentally double-unlink.
    pub fn unlink(self) -> Result<(), ShmError> {
        let ShmReader { file, mmap, path } = self;
        drop(mmap);
        drop(file);
        std::fs::remove_file(path)?;
        Ok(())
    }
}

fn shm_dir() -> PathBuf {
    let dev_shm = Path::new("/dev/shm");
    if dev_shm.is_dir() {
        dev_shm.to_path_buf()
    } else {
        std::env::temp_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_bytes(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn roundtrip_1_byte() {
        let mut writer = ShmWriter::create(1).expect("create");
        writer.write(b"X").expect("write");
        let path = writer.into_path().expect("into_path");

        let reader = ShmReader::open(&path).expect("open");
        assert_eq!(reader.as_bytes(), b"X");
        reader.unlink().expect("unlink");

        assert!(!path.exists(), "unlink must remove the file");
    }

    #[test]
    fn roundtrip_1_mib() {
        let data = synthetic_bytes(1024 * 1024);
        let mut writer = ShmWriter::create(data.len()).expect("create");
        writer.write(&data).expect("write");
        let path = writer.into_path().expect("into_path");

        let reader = ShmReader::open(&path).expect("open");
        assert_eq!(reader.as_bytes(), data.as_slice());
        reader.unlink().expect("unlink");
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn roundtrip_512_mib() {
        if std::env::var("STEAMLENS_LARGE_TESTS").is_err() {
            return;
        }
        let size = 512 * 1024 * 1024;
        let data = synthetic_bytes(size);
        let mut writer = ShmWriter::create(size).expect("create");
        writer.write(&data).expect("write");
        let path = writer.into_path().expect("into_path");

        let reader = ShmReader::open(&path).expect("open");
        let bytes = reader.as_bytes();
        assert_eq!(bytes.len(), size);
        assert_eq!(&bytes[..1024], &data[..1024]);
        assert_eq!(&bytes[size - 1024..], &data[size - 1024..]);
        reader.unlink().expect("unlink");
    }

    #[test]
    fn write_overflow_returns_error() {
        let mut writer = ShmWriter::create(100).expect("create");
        let result = writer.write(&[0u8; 200]);
        assert!(matches!(
            result,
            Err(ShmError::Overflow {
                wanted: 200,
                capacity: 100
            })
        ));
    }

    #[test]
    fn open_missing_path_returns_error() {
        let result = ShmReader::open(Path::new(
            "/dev/shm/sl-nonexistent-test-9c7f2a8e1b3d4f5a6c7e8d9f0a1b2c3d",
        ));
        assert!(matches!(result, Err(ShmError::Io(_))));
    }

    #[test]
    fn unlink_after_read_removes_file() {
        let mut writer = ShmWriter::create(16).expect("create");
        writer.write(b"hello, steamlens").expect("write");
        let path = writer.into_path().expect("into_path");

        assert!(path.exists(), "file must persist after into_path");
        let reader = ShmReader::open(&path).expect("open");
        reader.unlink().expect("unlink");
        assert!(!path.exists(), "unlink must remove the file");
    }

    #[test]
    fn writer_drop_without_into_path_auto_deletes() {
        let path: PathBuf;
        {
            let writer = ShmWriter::create(16).expect("create");
            path = writer.path().to_path_buf();
            assert!(path.exists(), "tempfile must exist while writer alive");
        }
        assert!(
            !path.exists(),
            "writer drop without into_path must auto-delete"
        );
    }

    #[test]
    fn sweep_orphans_removes_matching_prefix_only() {
        let dir = std::env::temp_dir().join(format!(
            "steamlens-sweep-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("steamlens-orphan-1"), b"x").unwrap();
        std::fs::write(dir.join("steamlens-orphan-2"), b"y").unwrap();
        std::fs::write(dir.join("not-ours-tmpfile"), b"z").unwrap();

        let removed = sweep_orphans_in(&dir, Duration::ZERO);
        assert_eq!(removed, 2, "must sweep only steamlens-prefixed files");
        assert!(!dir.join("steamlens-orphan-1").exists());
        assert!(!dir.join("steamlens-orphan-2").exists());
        assert!(
            dir.join("not-ours-tmpfile").exists(),
            "non-prefixed files must remain"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sweep_orphans_respects_min_age() {
        let dir = std::env::temp_dir().join(format!(
            "steamlens-sweep-age-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("steamlens-fresh"), b"x").unwrap();

        let removed = sweep_orphans_in(&dir, Duration::from_secs(3600));
        assert_eq!(removed, 0, "fresh file must not be swept");
        assert!(dir.join("steamlens-fresh").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn shm_dir_routes_to_dev_shm_when_available() {
        let dir = shm_dir();
        if Path::new("/dev/shm").is_dir() {
            assert_eq!(dir, Path::new("/dev/shm"));
        } else {
            assert_eq!(dir, std::env::temp_dir());
        }
    }
}
