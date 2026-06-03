use std::ffi::NulError;
use std::io;
use std::path::PathBuf;

use thiserror::Error;

use steamlens_vdf::PackageInfoError;

#[derive(Debug, Error)]
pub enum SteamError {
    #[error("Steam client is not running. Please start Steam and try again.")]
    SteamNotRunning,

    #[error("Steam is running but no user is signed in")]
    NotLoggedIn,

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

    #[error("Achievement or stat name contains an interior NUL byte: {source}")]
    InvalidString {
        #[source]
        source: NulError,
    },

    #[error("Steam returned false for {method}")]
    CallFailed { method: &'static str },

    #[error("Achievement {name:?} not found or returned null from Steam")]
    AchievementNotFound { name: String },

    #[error("Failed to parse Steam schema cache: {source}")]
    SchemaParseError {
        #[source]
        source: steamlens_vdf::VdfError,
    },

    #[error("Steam GetUserDataFolder returned false or an empty path")]
    UserDataFolderUnavailable,

    #[error(
        "Cannot derive Steam root from user data folder path: {observed}",
        observed = .observed.display()
    )]
    MalformedUserDataPath { observed: PathBuf },
}

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("Could not determine Steam root: {0}")]
    SteamRoot(#[source] SteamError),

    #[error("Failed to read packageinfo.vdf: {0}")]
    PackageInfoIo(#[source] io::Error),

    #[error("Failed to parse packageinfo.vdf: {0}")]
    PackageInfoParse(#[source] PackageInfoError),
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
