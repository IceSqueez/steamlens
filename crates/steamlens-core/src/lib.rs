#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!(
    "steamlens-core: only linux x86_64 is supported in the current phase. \
     windows and macos backends will be implemented in future phases."
);

mod client;
mod error;
mod ffi;
pub mod ipc;
pub mod library;
pub mod profile;
mod raw_callback;
mod stat_schema;
mod steam_callback;
pub mod steam_state;
mod user_stats;

pub use client::{Client, Image, connect};
pub use error::{LibraryScanError, SteamError};
pub use ipc::{
    AchievementData, AchievementIcon, FrameError, StatData, StatValue, WorkerCommand,
    WorkerResponse,
};
pub use library::{GameSummary, scan_installed_games};
pub use profile::{ProfileError, UserProfile, load_local_profile};
pub use raw_callback::RawCallback;
pub use stat_schema::{StatDescriptor, StatKind};
pub use steam_callback::{SteamCallback, SteamResult};
pub use steam_state::{ManifestState, read_all_last_played, read_last_played, read_manifest_state};
pub use user_stats::UserStats;
