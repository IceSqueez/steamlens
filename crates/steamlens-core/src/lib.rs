mod client;
mod error;
mod ffi;
pub mod genre;
pub mod ipc;
pub mod library;
pub mod paths;
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
pub use genre::primary_genre_name;
pub use ipc::shm::{
    ShmError, ShmReader, ShmWriter, read_payload, sweep_orphans, unlink_at, write_payload,
};
pub use ipc::{
    AchievementCountPayload, AchievementData, AchievementIcon, AchievementsAndStatsPayload,
    CardOnlyAchievement, CardOnlyPayload, FrameError, ProbeResultPayload, StatData, StatValue,
    WorkerCommand, WorkerErrorKind, WorkerResponse,
};
pub use library::GameSummary;
pub use paths::{
    appcache_stats_dir, steam_install_root_candidates, steamclient_lib_candidates, user_data_dir,
};
pub use probe::{ProbeError, ProbedProfile, probe_steam};
pub use process::{ChildLifetimeGuard, associate_kill_on_parent_exit, current_exe_resilient};
pub use profile::{ProfileError, STEAMID64_INDIVIDUAL_MIN, UserProfile, load_local_profile};
pub use raw_callback::RawCallback;
pub use stat_schema::{StatDescriptor, StatKind};
pub use steam_callback::{
    CALLBACK_ID_GLOBAL_ACHIEVEMENT_PERCENTAGES_READY, CALLBACK_ID_USER_ACHIEVEMENT_ICON_FETCHED,
    CALLBACK_ID_USER_STATS_RECEIVED, CALLBACK_ID_USER_STATS_STORED, STEAM_RESULT_NO_STATS_SCHEMA,
    STEAM_RESULT_OK, SteamCallback, SteamResult,
};
pub use steam_state::read_last_played;
pub use user_stats::UserStats;
