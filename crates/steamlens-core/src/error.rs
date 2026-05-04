use std::ffi::NulError;
use std::io;
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
}

/// Errors that can occur while scanning the local Steam library for installed
/// games.  Per-game failures (bad `.acf` files, missing schema) are silently
/// swallowed; only catalogue-level failures propagate here.
#[derive(Debug, Error)]
pub enum LibraryScanError {
    /// Reading `libraryfolders.vdf` failed with an I/O error AND the fallback
    /// default-root path also failed.
    #[error("Failed to read Steam library folders file: {0}")]
    LibraryFoldersIo(#[source] io::Error),

    /// `libraryfolders.vdf` was parsed successfully but contained no library
    /// paths — this should not happen with a valid Steam installation.
    #[error("No Steam library paths found in libraryfolders.vdf")]
    NoLibrariesFound,
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
