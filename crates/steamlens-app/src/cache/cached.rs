use std::path::PathBuf;

use serde::{Serialize, de::DeserializeOwned};

use crate::cache::store::{CacheIoError, atomic_write};

pub trait Cached: Serialize + DeserializeOwned + Sized {
    fn path() -> PathBuf;
}

pub async fn write<T: Cached>(value: &T) -> Result<(), CacheIoError> {
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|e| CacheIoError::Serialize(e.to_string()))?;
    atomic_write(&T::path(), &bytes).await
}
