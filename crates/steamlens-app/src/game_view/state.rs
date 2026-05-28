use std::collections::{HashMap, HashSet, VecDeque};

use super::types::{
    AchievementFilter, AchievementRow, AchievementSort, RarityTier, StatRow, compute_tier_map,
    dirty_count, has_stat_errors,
};

#[derive(Debug, Clone)]
pub struct SeededGameView {
    pub game_name: String,
    pub achievements: Vec<AchievementRow>,
    pub stats: Vec<StatRow>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameViewPhase {
    Connecting,
    WaitingStats,
    Ready,
    Saving,
    Error,
}

impl std::fmt::Debug for GameViewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameViewState")
            .field("app_id", &self.app_id)
            .field("phase", &self.phase)
            .finish()
    }
}

pub struct GameViewState {
    pub app_id: u32,
    pub game_name: String,
    pub phase: GameViewPhase,

    pub achievements: Vec<AchievementRow>,
    pub stats: Vec<StatRow>,
    pub reveal_queue: VecDeque<String>,
    pub tier_breakdown: Vec<(RarityTier, u32)>,

    pub search_query: String,
    pub stats_search_query: String,
    pub filter: AchievementFilter,
    pub achievement_sort: AchievementSort,
    pub rarity_tier_set: HashSet<RarityTier>,
    pub include_hidden: bool,

    pub apply_confirm_input: String,
    pub show_apply_modal: bool,

    pub spinner_angle: f32,
    pub fade_in: f32,
    pub rare_glow_phase: f32,

    pub error_message: String,

    pub pending_icons: HashMap<String, steamlens_core::AchievementIcon>,
    pub pending_rarity_percent: Option<HashMap<String, f32>>,

    pub capsule_handles: HashMap<
        (u32, crate::capsule_cache::CapsuleSize),
        crate::profile_view::types::StoredCapsule,
    >,

    pub capsule_unavailable: HashSet<(u32, crate::capsule_cache::CapsuleSize)>,

    pub hovered_bar_slice: Option<RarityTier>,

    pub expected_total: u32,

    pub genre: Option<String>,
    pub playtime_minutes: Option<u32>,

    pub cache_only: bool,
}

impl GameViewState {
    pub fn new(app_id: u32) -> Self {
        Self {
            app_id,
            game_name: format!("App {app_id}"),
            phase: GameViewPhase::Connecting,
            achievements: Vec::new(),
            stats: Vec::new(),
            reveal_queue: VecDeque::new(),
            tier_breakdown: Vec::new(),
            search_query: String::new(),
            stats_search_query: String::new(),
            filter: AchievementFilter::All,
            achievement_sort: AchievementSort::UnlockChance,
            rarity_tier_set: HashSet::new(),
            include_hidden: false,
            apply_confirm_input: String::new(),
            show_apply_modal: false,
            spinner_angle: 0.0,
            fade_in: 0.0,
            rare_glow_phase: 0.0,
            error_message: String::new(),
            pending_icons: HashMap::new(),
            pending_rarity_percent: None,
            capsule_handles: HashMap::new(),
            capsule_unavailable: HashSet::new(),
            hovered_bar_slice: None,
            expected_total: 0,
            genre: None,
            playtime_minutes: None,
            cache_only: false,
        }
    }

    pub fn dirty_count(&self) -> usize {
        dirty_count(&self.achievements, &self.stats)
    }

    pub fn has_stat_errors(&self) -> bool {
        has_stat_errors(&self.stats)
    }

    pub fn has_pending_reveals(&self) -> bool {
        !self.reveal_queue.is_empty()
    }

    pub fn apply_confirm_matches(&self) -> bool {
        self.apply_confirm_input
            .trim()
            .eq_ignore_ascii_case("confirmed")
    }

    pub fn has_fading_cards(&self) -> bool {
        self.achievements
            .iter()
            .any(|r| r.appeared && r.card_opacity < 1.0)
    }
}

pub(crate) fn compute_tier_breakdown(achievements: &[AchievementRow]) -> Vec<(RarityTier, u32)> {
    let tier_map = compute_tier_map(achievements);
    let mut counts: HashMap<RarityTier, u32> = HashMap::new();
    for row in achievements {
        if row.effective_achieved()
            && let Some(&tier) = tier_map.get(&row.data.id)
        {
            *counts.entry(tier).or_insert(0) += 1;
        }
    }
    [
        RarityTier::Common,
        RarityTier::Uncommon,
        RarityTier::Rare,
        RarityTier::Mythical,
        RarityTier::Legendary,
    ]
    .iter()
    .filter_map(|&t| counts.get(&t).map(|&c| (t, c)))
    .collect()
}
