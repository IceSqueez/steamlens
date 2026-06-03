use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::Mutex as AsyncMutex;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

static CACHE_WRITE_LOCKS: OnceLock<Mutex<HashMap<u32, Arc<AsyncMutex<()>>>>> = OnceLock::new();

pub(crate) fn cache_write_lock(app_id: u32) -> Arc<AsyncMutex<()>> {
    let map = CACHE_WRITE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .entry(app_id)
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

#[derive(Debug, thiserror::Error)]
pub enum CacheIoError {
    #[error("I/O error writing cache: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization failed: {0}")]
    Serialize(String),
}

pub async fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), CacheIoError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let tmp_path = tmp_path_for(path);

    {
        use tokio::io::AsyncWriteExt;
        #[cfg(unix)]
        let mut file = {
            use std::os::unix::fs::PermissionsExt;
            let f = tokio::fs::File::create(&tmp_path).await?;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))
                .await?;
            f
        };
        #[cfg(not(unix))]
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        file.write_all(bytes).await?;
        file.sync_data().await?;
    }

    tokio::fs::rename(&tmp_path, path).await?;

    Ok(())
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let pid = std::process::id();
    let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut s = path.as_os_str().to_owned();
    s.push(format!(".tmp.{pid}.{seq}"));
    PathBuf::from(s)
}
