#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!(
    "steamlens-core: only linux x86_64 is supported in the current phase. \
     windows and macos backends will be implemented in future phases."
);

mod client;
mod error;
mod ffi;
mod raw_callback;
mod steam_callback;
mod user_stats;

pub use client::{Client, connect};
pub use error::SteamError;
pub use raw_callback::RawCallback;
pub use steam_callback::{SteamCallback, SteamResult};
pub use user_stats::UserStats;
