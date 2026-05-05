use std::ffi::NulError;
use std::io;
use std::path::PathBuf;

use thiserror::Error;

use steamlens_vdf::PackageInfoError;

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

    #[error("Achievement or stat name contains an interior NUL byte: {source}")]
    InvalidString {
        #[source]
        source: NulError,
    },

    #[error("Steam returned false for {method}")]
    CallFailed { method: &'static str },

    #[error("Achievement {name:?} not found or returned null from Steam")]
    AchievementNotFound { name: String },

    /// The schema cache file exists but could not be parsed.
    ///
    /// A missing file is not an error — `stat_descriptors` returns an empty
    /// `Vec` in that case. This variant fires only when the file is present but
    /// the binary KeyValue data is truncated or otherwise corrupt.
    #[error("Failed to parse Steam schema cache: {source}")]
    SchemaParseError {
        #[source]
        source: steamlens_vdf::VdfError,
    },

    /// `ISteamUser012::GetUserDataFolder` returned `false` or an empty path.
    #[error("Steam GetUserDataFolder returned false or an empty path")]
    UserDataFolderUnavailable,

    /// `GetUserDataFolder` returned a path that does not end with
    /// `userdata/<steamid3>` — the steam root cannot be derived from it.
    #[error(
        "Cannot derive Steam root from user data folder path: {observed}",
        observed = .observed.display()
    )]
    MalformedUserDataPath { observed: PathBuf },
}

/// Errors that can occur while enumerating the owned Steam game library.
///
/// Per-game failures (name lookup returning empty, missing localconfig) are
/// silently swallowed; only catalogue-level failures propagate here.
#[derive(Debug, Error)]
pub enum LibraryError {
    /// Deriving the Steam root from the pipe failed.
    #[error("Could not determine Steam root: {0}")]
    SteamRoot(#[source] SteamError),

    /// Reading `appcache/packageinfo.vdf` failed with an I/O error.
    #[error("Failed to read packageinfo.vdf: {0}")]
    PackageInfoIo(#[source] io::Error),

    /// `packageinfo.vdf` was read but could not be parsed.
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
