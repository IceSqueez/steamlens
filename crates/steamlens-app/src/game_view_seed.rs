use iced::Task;

use crate::Message;
use crate::cache::{self, GameCacheEntry};
use crate::game_view::{self, GameViewMessage, GameViewState};
use crate::progress_scan;

pub(crate) fn recompute_tier_breakdown_if_missing(entry: &mut GameCacheEntry) {
    use game_view::compute_tier_breakdown;
    use game_view::types::{AchievementData, AchievementRow};

    if !entry.tier_breakdown.is_empty() || entry.achievements.is_empty() {
        return;
    }

    let rows: Vec<AchievementRow> = entry
        .achievements
        .iter()
        .map(|a| {
            let data = AchievementData {
                id: a.api_name.clone(),
                display_name: a.display_name.clone(),
                description: a.description.clone(),
                is_hidden: a.hidden,
                is_achieved: a.earned,
                unlock_time: a.earned_at.map(|t| t as u32),
                permission: 0,
                icon: None,
            };
            let mut row = AchievementRow::from(data);
            row.rarity_percent = a.global_percent.map(|p| p as f32);
            row
        })
        .collect();

    entry.tier_breakdown = compute_tier_breakdown(&rows);
}

pub(crate) fn compute_seed_from_cache(cached: &GameCacheEntry) -> game_view::SeededGameView {
    use game_view::SeededGameView;
    use game_view::types::{AchievementData, AchievementRow, StatData, StatRow, StatValue};

    let app_id = cached.app_id;
    let achievements: Vec<AchievementRow> = cached
        .achievements
        .iter()
        .map(|a| {
            let icon = cache::icons::load_blocking(app_id, &a.api_name);
            let data = AchievementData {
                id: a.api_name.clone(),
                display_name: a.display_name.clone(),
                description: a.description.clone(),
                is_hidden: a.hidden,
                is_achieved: a.earned,
                unlock_time: a.earned_at.map(|t| t as u32),
                permission: 0,
                icon,
            };
            let mut row = AchievementRow::from(data);
            row.appeared = true;
            row.card_opacity = 1.0;
            row.rarity_percent = a.global_percent.map(|p| p as f32);
            row
        })
        .collect();

    let stats: Vec<StatRow> = cached
        .stats
        .iter()
        .map(|s| {
            let value = match s.value {
                cache::types::CachedStatValue::Int(i) => StatValue::Int(i as i32),
                cache::types::CachedStatValue::Float(f) => StatValue::Float(f as f32),
            };
            let data = StatData {
                id: s.api_name.clone(),
                display_name: s.display_name.clone(),
                value,
                original_value: value,
                max_value: s.max_value,
                min_value: s.min_value,
                default_value: s.default_value,
                is_increment_only: s.is_increment_only,
                permission: s.permission,
            };
            StatRow::from(data)
        })
        .collect();

    SeededGameView {
        game_name: cached.name.clone(),
        achievements,
        stats,
    }
}

pub(crate) fn spawn_seed_task(app_id: u32, cached: GameCacheEntry) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || compute_seed_from_cache(&cached))
                .await
                .unwrap_or_else(|e| {
                    tracing::error!(error = %e, app_id, "seed task panicked");
                    game_view::SeededGameView {
                        game_name: String::new(),
                        achievements: Vec::new(),
                        stats: Vec::new(),
                    }
                })
        },
        move |seeded| {
            Message::GameView(GameViewMessage::CacheSeeded {
                app_id,
                seeded: Box::new(seeded),
            })
        },
    )
}

pub(crate) fn build_cache_entry_from_scan(
    scanned: &progress_scan::ScannedGameData,
    app_id: u32,
    entry_name: Option<&str>,
    steam_state: &std::collections::HashMap<u32, steamlens_core::SteamAppState>,
) -> GameCacheEntry {
    use cache::types::{CachedAchievement, CachedProgress, CachedStat};
    use game_view::compute_tier_breakdown;
    use game_view::types::{AchievementData, AchievementRow};
    use steamlens_core::StatValue;

    let stats: Vec<CachedStat> = scanned
        .stats
        .iter()
        .map(|s| {
            let value = match s.value {
                StatValue::Int(i) => cache::types::CachedStatValue::Int(i as i64),
                StatValue::Float(f) => cache::types::CachedStatValue::Float(f as f64),
            };
            CachedStat {
                api_name: s.id.clone(),
                display_name: s.display_name.clone(),
                value,
                max_value: s.max_value,
                min_value: s.min_value,
                default_value: s.default_value,
                is_increment_only: s.is_increment_only,
                permission: s.permission,
            }
        })
        .collect();

    let earned = scanned
        .achievements
        .iter()
        .filter(|a| a.is_achieved)
        .count() as u32;
    let total = scanned.achievements.len() as u32;

    let tier_rows: Vec<AchievementRow> = scanned
        .achievements
        .iter()
        .map(|a| {
            let data = AchievementData {
                id: a.id.clone(),
                display_name: String::new(),
                description: String::new(),
                is_hidden: false,
                is_achieved: a.is_achieved,
                unlock_time: None,
                permission: 0,
                icon: None,
            };
            let mut row = AchievementRow::from(data);
            row.rarity_percent = scanned.global_percentages.get(&a.id).copied();
            row
        })
        .collect();
    let tier_breakdown = compute_tier_breakdown(&tier_rows);

    let state_entry = steam_state.get(&app_id).copied().unwrap_or_default();
    let steam_last_played = state_entry.last_played.unwrap_or(0) as u64;
    let playtime_minutes = state_entry.playtime_minutes;
    let cached_at = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let name = scanned
        .app_name
        .clone()
        .or_else(|| entry_name.map(|s| s.to_owned()))
        .unwrap_or_else(|| format!("App {app_id}"));

    let achievements: Vec<CachedAchievement> = scanned
        .achievements
        .iter()
        .map(|a| CachedAchievement {
            api_name: a.id.clone(),
            display_name: String::new(),
            description: String::new(),
            hidden: false,
            icon_path: None,
            icon_locked_path: None,
            earned: a.is_achieved,
            earned_at: None,
            global_percent: scanned.global_percentages.get(&a.id).map(|p| *p as f64),
        })
        .collect();

    GameCacheEntry {
        schema_version: cache::CURRENT_SCHEMA_VERSION,
        app_id,
        name,
        steam_last_played,
        cached_at,
        achievements,
        stats,
        progress: CachedProgress { earned, total },
        tier_breakdown,
        genre: scanned.genre.clone(),
        playtime_minutes,
    }
}

pub(crate) fn build_game_view_cache_entry(
    state: &GameViewState,
    app_id: u32,
    steam_state: &std::collections::HashMap<u32, steamlens_core::SteamAppState>,
) -> GameCacheEntry {
    use cache::types::{CachedAchievement, CachedProgress, CachedStat};

    let earned = state
        .achievements
        .iter()
        .filter(|r| r.data.is_achieved)
        .count() as u32;
    let total = state.achievements.len() as u32;

    let achievements = state
        .achievements
        .iter()
        .map(|r| CachedAchievement {
            api_name: r.data.id.clone(),
            display_name: r.data.display_name.clone(),
            description: r.data.description.clone(),
            hidden: r.data.is_hidden,
            icon_path: None,
            icon_locked_path: None,
            earned: r.data.is_achieved,
            earned_at: r.data.unlock_time.map(|t| t as u64),
            global_percent: r.rarity_percent.map(|p| p as f64),
        })
        .collect();

    let stats = state
        .stats
        .iter()
        .map(|r| {
            use steamlens_core::StatValue;
            let value = match r.data.value {
                StatValue::Int(i) => cache::types::CachedStatValue::Int(i as i64),
                StatValue::Float(f) => cache::types::CachedStatValue::Float(f as f64),
            };
            CachedStat {
                api_name: r.data.id.clone(),
                display_name: r.data.display_name.clone(),
                value,
                max_value: r.data.max_value,
                min_value: r.data.min_value,
                default_value: r.data.default_value,
                is_increment_only: r.data.is_increment_only,
                permission: r.data.permission,
            }
        })
        .collect();

    let state_entry = steam_state.get(&app_id).copied().unwrap_or_default();
    let steam_last_played = state_entry.last_played.unwrap_or(0) as u64;
    let playtime_minutes = state_entry.playtime_minutes;

    let cached_at = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    GameCacheEntry {
        schema_version: cache::CURRENT_SCHEMA_VERSION,
        app_id,
        name: state.game_name.clone(),
        steam_last_played,
        cached_at,
        achievements,
        stats,
        progress: CachedProgress { earned, total },
        tier_breakdown: state.tier_breakdown.clone(),
        genre: None,
        playtime_minutes,
    }
}
