use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SteamError {
    #[error("Steam client is not running. Please start Steam and try again.")]
    SteamNotRunning,

    #[error(
        "Could not locate steamclient.so. Searched: {}",
        format_paths(.searched)
    )]
    SteamInstallNotFound { searched: Vec<PathBuf> },

    #[error("Failed to load Steam library at {path}: {source}", path = .path.display())]
    LibraryLoadFailed {
        path: PathBuf,
        #[source]
        source: libloading::Error,
    },

    #[error("Failed to resolve symbol {symbol} in steamclient.so: {source}")]
    SymbolNotFound {
        symbol: &'static str,
        #[source]
        source: libloading::Error,
    },

    #[error("Steam interface version contained an interior NUL byte: {version:?}")]
    InvalidInterfaceVersion { version: String },

    #[error("Steam declined to vend interface {version:?} (CreateInterface returned null)")]
    InterfaceUnavailable { version: String },

    #[error("Failed to create Steam IPC pipe (SteamClient018::CreateSteamPipe returned 0)")]
    PipeCreationFailed,
}

fn format_paths(paths: &[PathBuf]) -> String {
    if paths.is_empty() {
        return "(no candidates)".to_owned();
    }
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}
