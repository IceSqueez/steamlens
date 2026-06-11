use serde::{Deserialize, Serialize};

use crate::game_view::types::RarityTier;

pub const CURRENT_SCHEMA_VERSION: u32 = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameCacheEntry {
    pub schema_version: u32,
    pub app_id: u32,
    pub name: String,
    pub cached_change_number: u32,
    pub steam_last_played: u64,
    pub cached_at: u64,
    pub achievements: Vec<CachedAchievement>,
    pub stats: Vec<CachedStat>,
    pub progress: CachedProgress,
    pub tier_breakdown: Vec<(RarityTier, u32)>,
    pub genre: Option<String>,
    pub playtime_minutes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAchievement {
    pub api_name: String,
    pub display_name: String,
    pub description: String,
    #[serde(rename = "hidden")]
    pub is_hidden: bool,
    pub icon_path: Option<String>,
    pub icon_locked_path: Option<String>,
    #[serde(rename = "earned")]
    pub is_achieved: bool,
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
    pub max_value: Option<u64>,
    pub min_value: Option<i64>,
    pub default_value: Option<i64>,
    pub is_increment_only: bool,
    pub permission: u32,
}

pub use steamlens_core::AchievementsCountPayload as CachedProgress;

#[derive(Debug, Clone)]
pub struct CacheHit {
    pub app_id: u32,
    pub entry: GameCacheEntry,
}
