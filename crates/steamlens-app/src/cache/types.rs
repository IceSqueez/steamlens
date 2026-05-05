use serde::{Deserialize, Serialize};

use crate::game_view::types::RarityTier;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameCacheEntry {
    pub schema_version: u32,
    pub app_id: u32,
    pub name: String,
    pub steam_last_updated: u64,
    pub steam_last_played: u64,
    pub cached_at: u64,
    pub achievements: Vec<CachedAchievement>,
    pub stats: Vec<CachedStat>,
    pub progress: CachedProgress,
    #[serde(default)]
    pub tier_breakdown: Vec<(RarityTier, u32)>,
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

/// `value_int` and `value_float` are mutually exclusive — Steam tags
/// each stat as INT or FLOAT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedStat {
    pub api_name: String,
    pub display_name: String,
    pub value_int: Option<i64>,
    pub value_float: Option<f64>,
    pub default_value: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CachedProgress {
    pub earned: u32,
    pub total: u32,
}

#[derive(Debug, Clone)]
pub struct CacheHit {
    pub app_id: u32,
    pub entry: GameCacheEntry,
}
