use std::time::Duration;

pub const STEAM_CONNECT: Duration = Duration::from_secs(10);
pub const PROBE_STEAM_BOOT: Duration = Duration::from_secs(15);
pub const PROBE_STEAM_RECONNECT: Duration = Duration::from_secs(30);
pub const STAT_RECEIVED: Duration = Duration::from_secs(10);
pub const STORE_CONFIRMED: Duration = Duration::from_secs(5);
pub const GLOBAL_PERCENTAGES: Duration = Duration::from_secs(15);
pub const LIVE_LOAD: Duration = Duration::from_secs(15);
pub const COLD_SCAN_LOAD: Duration = Duration::from_secs(8);
pub const STAGING: Duration = Duration::from_secs(5);
pub const CHILD_KILL: Duration = Duration::from_secs(2);
pub const CHILD_DRAIN: Duration = Duration::from_secs(3);
pub const STDERR_DRAIN: Duration = Duration::from_secs(1);
pub const POLL_INTERVAL: Duration = Duration::from_millis(50);
