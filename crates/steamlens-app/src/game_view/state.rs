use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use super::types::{
    AchievementFilter, AchievementRow, AchievementSort, RarityTier, StatRow, compute_tier_map,
    dirty_count, has_stat_errors, visible_achievement_indices,
};
use super::widget::compute_game_summary_with_tier_map;
use crate::ui::widgets::widget::WidgetSummary;

#[derive(Debug, Default)]
pub struct DerivedGameView {
    pub tier_map: Arc<HashMap<String, RarityTier>>,
    pub visible_indices: Vec<usize>,
    pub summary: WidgetSummary,
    pub has_legendary_visible: bool,
}

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
    pub reveal_accumulator: f32,

    pub error_message: String,

    pub pending_icons: HashMap<String, steamlens_core::AchievementIcon>,
    pub pending_rarity_percent: Option<HashMap<String, f32>>,

    pub hovered_bar_slice: Option<RarityTier>,

    pub expected_total: u32,

    pub genre: Option<String>,
    pub playtime_minutes: Option<u32>,

    pub cache_only: bool,

    pub achievement_grid_scroll_y: f32,

    pub icon_handles: HashMap<String, iced::widget::image::Handle>,

    pub derived: DerivedGameView,
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
            reveal_accumulator: 0.0,
            error_message: String::new(),
            pending_icons: HashMap::new(),
            pending_rarity_percent: None,
            hovered_bar_slice: None,
            expected_total: 0,
            genre: None,
            playtime_minutes: None,
            cache_only: false,
            achievement_grid_scroll_y: 0.0,
            icon_handles: HashMap::new(),
            derived: DerivedGameView::default(),
        }
    }

    pub fn recompute_derived(&mut self) {
        let tier_map = compute_tier_map(&self.achievements);
        let visible_indices = visible_achievement_indices(
            &self.achievements,
            &tier_map,
            self.filter,
            &self.search_query,
            self.achievement_sort,
            &self.rarity_tier_set,
            self.include_hidden,
        );
        let summary = compute_game_summary_with_tier_map(&self.achievements, &tier_map);
        let has_legendary_visible = self.achievements.iter().any(|r| {
            r.appeared
                && tier_map
                    .get(&r.data.id)
                    .is_some_and(|&t| t == RarityTier::Legendary)
        });
        self.derived = DerivedGameView {
            tier_map: Arc::new(tier_map),
            visible_indices,
            summary,
            has_legendary_visible,
        };
    }

    pub fn recompute_visible_only(&mut self) {
        let visible_indices = visible_achievement_indices(
            &self.achievements,
            &self.derived.tier_map,
            self.filter,
            &self.search_query,
            self.achievement_sort,
            &self.rarity_tier_set,
            self.include_hidden,
        );
        self.derived.visible_indices = visible_indices;
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

    pub fn tick_animations(&mut self, delta_secs: f32) {
        const SPINNER_DEG_PER_SEC: f32 = 180.0;
        const FADE_PER_SEC: f32 = 2.4;
        const CARD_OPACITY_PER_SEC: f32 = 6.0;
        const GLOW_RAD_PER_SEC: f32 = 3.0;
        const REVEAL_INTERVAL_SECS: f32 = 0.030;

        let busy = matches!(
            self.phase,
            GameViewPhase::Connecting | GameViewPhase::WaitingStats | GameViewPhase::Saving
        );
        let needs_spinner_or_fade = busy || self.phase == GameViewPhase::Ready;
        if needs_spinner_or_fade {
            self.spinner_angle = (self.spinner_angle + SPINNER_DEG_PER_SEC * delta_secs) % 360.0;
            if self.phase == GameViewPhase::Ready && self.fade_in < 1.0 {
                self.fade_in = (self.fade_in + FADE_PER_SEC * delta_secs).min(1.0);
            }
        }

        if self.has_fading_cards() {
            for row in &mut self.achievements {
                if row.appeared && row.card_opacity < 1.0 {
                    row.card_opacity =
                        (row.card_opacity + CARD_OPACITY_PER_SEC * delta_secs).min(1.0);
                }
            }
        }

        if self.derived.has_legendary_visible {
            self.rare_glow_phase = (self.rare_glow_phase + GLOW_RAD_PER_SEC * delta_secs)
                % (2.0 * std::f32::consts::PI);
        }

        if self.has_pending_reveals() {
            self.reveal_accumulator += delta_secs;
            let mut popped_any = false;
            while self.reveal_accumulator >= REVEAL_INTERVAL_SECS {
                self.reveal_accumulator -= REVEAL_INTERVAL_SECS;
                let Some(id) = self.reveal_queue.pop_front() else {
                    self.reveal_accumulator = 0.0;
                    break;
                };
                if let Some(row) = self.achievements.iter_mut().find(|r| r.data.id == id) {
                    row.appeared = true;
                    popped_any = true;
                }
            }
            if popped_any {
                self.recompute_derived();
            }
        } else {
            self.reveal_accumulator = 0.0;
        }
    }

    pub fn has_active_animations(&self) -> bool {
        matches!(
            self.phase,
            GameViewPhase::Connecting | GameViewPhase::WaitingStats | GameViewPhase::Saving
        ) || (self.phase == GameViewPhase::Ready && self.fade_in < 1.0)
            || self.has_pending_reveals()
            || self.has_fading_cards()
            || self.derived.has_legendary_visible
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
