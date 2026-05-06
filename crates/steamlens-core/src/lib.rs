mod client;
mod error;
mod ffi;
pub mod ipc;
pub mod library;
pub mod probe;
mod process;
pub mod profile;
mod raw_callback;
mod stat_schema;
mod steam_callback;
pub mod steam_state;
mod user_stats;

pub use client::{Client, Image, connect};
pub use error::{LibraryError, SteamError};
pub use ipc::shm::{
    ShmError, ShmReader, ShmWriter, read_payload, sweep_orphans, unlink_at, write_payload,
};
pub use ipc::{
    AchievementCountPayload, AchievementData, AchievementIcon, AchievementsAndStatsPayload,
    CardOnlyAchievement, CardOnlyPayload, FrameError, ProbeResultPayload, StatData, StatValue,
    WorkerCommand, WorkerResponse,
};
pub use library::{GameSummary, enumerate_owned_games};
pub use probe::{ProbeError, ProbedProfile, probe_steam};
pub use process::{ChildLifetimeGuard, associate_kill_on_parent_exit};
pub use profile::{ProfileError, UserProfile, load_local_profile};
pub use raw_callback::RawCallback;
pub use stat_schema::{StatDescriptor, StatKind};
pub use steam_callback::{SteamCallback, SteamResult};
pub use steam_state::{ManifestState, read_all_last_played, read_last_played, read_manifest_state};
pub use user_stats::UserStats;
