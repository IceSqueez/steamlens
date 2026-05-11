use serde::{Deserialize, Serialize};

use crate::game_view::types::RarityTier;

pub const CURRENT_SCHEMA_VERSION: u32 = 3;

#[allow(dead_code, reason = "consumers land in subsequent migration chunks")]
pub const SUMMARY_SCHEMA_VERSION: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameCacheEntry {
    pub schema_version: u32,
    pub app_id: u32,
    pub name: String,
    pub steam_last_played: u64,
    pub cached_at: u64,
    pub achievements: Vec<CachedAchievement>,
    pub stats: Vec<CachedStat>,
    pub progress: CachedProgress,
    #[serde(default)]
    pub tier_breakdown: Vec<(RarityTier, u32)>,
    #[serde(default)]
    pub genre: Option<String>,
}

#[allow(dead_code, reason = "consumers land in subsequent migration chunks")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSummaryCache {
    pub schema_version: u32,
    pub app_id: u32,
    pub name: String,
    pub cached_change_number: u32,
    pub cached_at: u64,
    pub progress: CachedProgress,
    pub tier_breakdown: Vec<(RarityTier, u32)>,
    pub genre: Option<String>,
}

#[allow(dead_code, reason = "consumers land in subsequent migration chunks")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameAchievementsCache {
    pub schema_version: u32,
    pub app_id: u32,
    pub cached_at: u64,
    pub achievements: Vec<CachedAchievement>,
    pub stats: Vec<CachedStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAchievement {
    pub api_name: String,
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    pub icon_path: Option<String>,
    pub icon_locked_path: Option<String>,
    pub earned: bool,
    pub earned_at: Option<u64>,
    pub global_percent: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CachedStatValue {
    Int(i64),
    Float(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedStat {
    pub api_name: String,
    pub display_name: String,
    pub value: CachedStatValue,
}

pub use steamlens_core::AchievementCountPayload as CachedProgress;

#[derive(Debug, Clone)]
pub struct CacheHit {
    pub app_id: u32,
    pub entry: GameCacheEntry,
}
