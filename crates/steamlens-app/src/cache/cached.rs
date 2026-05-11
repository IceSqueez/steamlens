use std::path::{Path, PathBuf};

use serde::{Serialize, de::DeserializeOwned};

use crate::cache::store::{CacheIoError, atomic_write};

pub trait Cached: Serialize + DeserializeOwned + Sized {
    const NAME: &'static str;
    const CURRENT_SCHEMA: u32;
    fn schema_version(&self) -> u32;
    fn path() -> PathBuf;
}

pub async fn write<T: Cached>(value: &T) -> Result<(), CacheIoError> {
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|e| CacheIoError::Serialize(e.to_string()))?;
    atomic_write(&T::path(), &bytes).await
}

pub async fn load<T: Cached>() -> Option<T> {
    load_from_path::<T>(&T::path()).await
}

pub(crate) async fn load_from_path<T: Cached>(path: &Path) -> Option<T> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let entry: T = serde_json::from_slice(&bytes)
        .map_err(|e| {
            crate::log!("{} cache: parse error at {}: {e}", T::NAME, path.display());
        })
        .ok()?;
    if entry.schema_version() != T::CURRENT_SCHEMA {
        crate::log!(
            "{} cache: schema {} != expected {}; treating as miss",
            T::NAME,
            entry.schema_version(),
            T::CURRENT_SCHEMA
        );
        return None;
    }
    Some(entry)
}
