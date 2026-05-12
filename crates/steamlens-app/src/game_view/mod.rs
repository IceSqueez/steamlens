pub mod stats_panel;
pub mod types;
mod view;
pub mod widget;

pub use view::GameViewProps;
pub use view::achievement_search_id;

use std::collections::HashMap;

pub fn header_content<'a>(
    state: &'a GameViewState,
    theme: crate::ui::theme::AppTheme,
) -> crate::screen::AppHeaderContent<'a> {
    use crate::screen::{SegmentItem, SegmentedControlConfig};
    use std::borrow::Cow;
    use types::AchievementSort;

    let sort_items: Vec<SegmentItem<'_>> = AchievementSort::ALL
        .iter()
        .copied()
        .map(|s| SegmentItem {
            label: Cow::Borrowed(s.short_label()),
            tooltip: Some(s.tooltip()),
            selected: state.achievement_sort == s,
            on_press: crate::Message::GameSortChanged(s),
        })
        .collect();

    crate::screen::AppHeaderContent {
        search: Some(crate::screen::SearchConfig {
            placeholder: "Search achievements\u{2026}",
            value: state.search_query.as_str(),
            id: view::achievement_search_id(),
        }),
        segments: vec![SegmentedControlConfig {
            label: Some("SORT"),
            items: sort_items,
        }],
        screen_actions: vec![view::build_game_reload_button()],
        leading: Some(view::build_back_leading()),
        status_filter: Some(build_achievement_status_strip(state)),
        category_filter: Some(build_rarity_tier_strip(state)),
        theme,
    }
}

fn build_achievement_status_strip(state: &GameViewState) -> crate::screen::FilterStrip<'_> {
    use types::AchievementFilter;
    let buttons = [
        (AchievementFilter::All, "All"),
        (AchievementFilter::Unlocked, "Unlocked"),
        (AchievementFilter::Locked, "Locked"),
    ]
    .into_iter()
    .map(|(f, label)| crate::screen::FilterButton {
        label: std::borrow::Cow::Borrowed(label),
        selected: state.filter == f,
        on_press: crate::Message::GameView(GameViewMessage::FilterChanged(f)),
    })
    .collect();

    crate::screen::FilterStrip { buttons }
}

fn build_rarity_tier_strip(state: &GameViewState) -> iced::Element<'_, crate::Message> {
    use crate::ui::widgets::pill::pill;
    use iced::widget::{Space, row, text};
    use iced::{Alignment, Color, Length};
    use types::{RarityTier, compute_tier_map};

    const TIER_PILL_RADIUS: f32 = 14.0;
    const TIER_PILL_PAD_H: u32 = 9;
    const TIER_PILL_PAD_V: u32 = 4;

    let tier_map = compute_tier_map(&state.achievements);
    let hidden_count = state
        .achievements
        .iter()
        .filter(|r| r.is_spoiler_hidden())
        .count();

    let any_selected = !state.rarity_tier_set.is_empty() || state.include_hidden;

    let mut chips: Vec<iced::Element<'_, crate::Message>> = Vec::new();
    for (tier, label) in [
        (RarityTier::Common, "Common"),
        (RarityTier::Uncommon, "Uncommon"),
        (RarityTier::Rare, "Rare"),
        (RarityTier::Mythical, "Mythical"),
        (RarityTier::Legendary, "Legendary"),
    ] {
        let count = tier_map.values().filter(|&&v| v == tier).count();
        let color = view::tier_color(tier);
        let inner = row![
            text(label).size(11).color(color),
            text(format!("{count}"))
                .size(11)
                .color(Color { a: 0.65, ..color }),
        ]
        .spacing(4)
        .align_y(Alignment::Center);

        let is_selected = state.rarity_tier_set.contains(&tier);
        let mut p = pill(inner, color)
            .radius(TIER_PILL_RADIUS)
            .padding(TIER_PILL_PAD_H, TIER_PILL_PAD_V)
            .reserve_dot_space(true)
            .selected(is_selected)
            .on_press(crate::Message::GameView(
                GameViewMessage::RarityTierToggled(tier),
            ));
        if !any_selected || is_selected {
            p = p.with_dot(color);
        }
        chips.push(p.into());
    }

    let hidden_color = crate::ui::theme::palette(crate::ui::theme::AppTheme::Dark).text_muted;
    let hidden_inner = row![
        text("Hidden").size(11).color(hidden_color),
        text(format!("{hidden_count}")).size(11).color(Color {
            a: 0.65,
            ..hidden_color
        }),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    let mut hidden_pill = pill(hidden_inner, hidden_color)
        .radius(TIER_PILL_RADIUS)
        .padding(TIER_PILL_PAD_H, TIER_PILL_PAD_V)
        .reserve_dot_space(true)
        .selected(state.include_hidden)
        .on_press(crate::Message::GameView(GameViewMessage::HiddenPillToggled));
    if !any_selected || state.include_hidden {
        hidden_pill = hidden_pill.with_dot(hidden_color);
    }
    chips.push(hidden_pill.into());

    if !state.rarity_tier_set.is_empty() || state.include_hidden {
        chips.push(Space::new().width(Length::Fill).into());
        let clear_label = text("Clear").size(11).color(hidden_color);
        chips.push(
            pill(clear_label, hidden_color)
                .radius(TIER_PILL_RADIUS)
                .padding(TIER_PILL_PAD_H, TIER_PILL_PAD_V)
                .selected(false)
                .on_press(crate::Message::GameView(
                    GameViewMessage::RarityFilterCleared,
                ))
                .into(),
        );
    }

    row(chips)
        .spacing(6)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}

use std::collections::{HashSet, VecDeque};

use iced::Task;

use crate::profile_view::types::ProfileViewState;
use crate::steam_worker::{SteamReply, SteamRequest};

use types::{
    AchievementFilter, AchievementRow, AchievementSort, Banner, BannerKind, BulkOp, RarityTier,
    StatRow, StatValue, build_apply_payload, compute_tier_map, dirty_count, has_stat_errors,
    visible_achievement_ids,
};

pub(crate) const MANAGER_FADE_DELTA: f32 = 0.2;

fn surface_connectivity_error(
    ctx: &mut crate::app_context::AppContext,
    err: crate::worker_subprocess::ConnectivityError,
) {
    use crate::worker_subprocess::ConnectivityError as CE;
    let msg = match err {
        CE::SteamNotRunning => "Steam is not running — cannot send command",
        CE::NotLoggedIn => "User is not signed in to Steam — cannot send command",
    };
    if !ctx.messaging.banners.iter().any(|b| b.body == msg) {
        ctx.messaging
            .push_banner(crate::messaging::BannerSeverity::Warning, msg, None, true);
    }
}

#[derive(Debug, Clone)]
pub enum GameViewMessage {
    AchievementToggled(String),
    FilterChanged(AchievementFilter),
    RarityTierToggled(RarityTier),
    HiddenPillToggled,
    RarityFilterCleared,
    AchievementSortChanged(AchievementSort),
    SearchChanged(String),
    StatsSearchChanged(String),
    StatsMaxAll,
    StatsResetAll,
    StatsMaxSingle(String),
    StatsResetSingle(String),
    BulkAction(BulkOp),
    ReloadRequested,
    ApplyClicked,
    ApplyConfirmInputChanged(String),
    ApplyConfirmed,
    ApplyCancelled,
    BannerDismissed,
    DiscardChanges,
    RevealHidden(String),
    SpinnerTick,
    RevealTick,
    GameViewFadeTick,
    RareGlowTick,
    RequestGoBack,
    AchievementsFullyLoaded,
    CapsuleLoaded {
        app_id: u32,
        size: crate::capsule_cache::CapsuleSize,
        handle: iced::widget::image::Handle,
        width: u32,
        height: u32,
    },
    CapsuleFailed {
        app_id: u32,
        size: crate::capsule_cache::CapsuleSize,
    },
    BarSliceHoverEnter(RarityTier),
    BarSliceHoverExit,
    InvalidateCacheClicked(u32),
}

#[derive(Debug, Clone)]
pub enum GameViewEvent {
    None,
    GoBack,
    AchievementsFullyLoaded { app_id: u32 },
    InvalidateCache { app_id: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameViewPhase {
    Connecting,
    WaitingStats,
    Ready,
    Saving,
    Error,
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

    pub banner: Option<Banner>,

    pub spinner_angle: f32,
    pub fade_in: f32,
    pub rare_glow_phase: f32,

    pub error_message: String,

    pub prev_profile_state: Box<ProfileViewState>,

    pub pending_icons: HashMap<String, steamlens_core::AchievementIcon>,
    pub pending_rarity_percent: Option<HashMap<String, f32>>,

    pub capsule_handles: HashMap<
        (u32, crate::capsule_cache::CapsuleSize),
        crate::profile_view::types::StoredCapsule,
    >,

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
            banner: None,
            spinner_angle: 0.0,
            fade_in: 0.0,
            rare_glow_phase: 0.0,
            error_message: String::new(),
            prev_profile_state: Box::new(ProfileViewState::new()),
            pending_icons: HashMap::new(),
            pending_rarity_percent: None,
            capsule_handles: HashMap::new(),
            hovered_bar_slice: None,
            expected_total: 0,
            genre: None,
            playtime_minutes: None,
            cache_only: false,
        }
    }

    pub fn with_prev_profile(mut self, prev: Box<ProfileViewState>) -> Self {
        self.prev_profile_state = prev;
        self
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

pub fn handle_steam_reply(state: &mut GameViewState, reply: SteamReply) -> Task<GameViewMessage> {
    match reply {
        SteamReply::Connected { app_name, .. } => {
            if let Some(name) = app_name {
                state.game_name = name;
            }
            state.phase = GameViewPhase::WaitingStats;
            Task::none()
        }
        SteamReply::ConnectFailed(e) => {
            state.phase = GameViewPhase::Error;
            state.error_message = e;
            Task::none()
        }
        SteamReply::RequestStatsFailed(e) => {
            state.phase = GameViewPhase::Error;
            state.error_message = e;
            Task::none()
        }
        SteamReply::AchievementsAndStats {
            achievements,
            stats,
        } => {
            let mut existing_icons: HashMap<String, steamlens_core::AchievementIcon> =
                HashMap::new();
            let mut existing_rarity_pct: HashMap<String, f32> = HashMap::new();
            let mut prev_revealed: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for row in state.achievements.drain(..) {
                if row.revealed {
                    prev_revealed.insert(row.data.id.clone());
                }
                if let Some(pct) = row.rarity_percent {
                    existing_rarity_pct.insert(row.data.id.clone(), pct);
                }
                if let Some(icon) = row.data.icon {
                    existing_icons.insert(row.data.id, icon);
                }
            }
            let pending_icons = std::mem::take(&mut state.pending_icons);
            let pending_pct = state.pending_rarity_percent.take();
            state.achievements = achievements
                .into_iter()
                .map(|mut data| {
                    if data.icon.is_none() {
                        data.icon = pending_icons
                            .get(&data.id)
                            .cloned()
                            .or_else(|| existing_icons.remove(&data.id));
                    }
                    let mut row = AchievementRow::from(data);
                    if prev_revealed.contains(&row.data.id) {
                        row.revealed = true;
                    }
                    row.rarity_percent = pending_pct
                        .as_ref()
                        .and_then(|m| m.get(&row.data.id).copied())
                        .or_else(|| existing_rarity_pct.get(&row.data.id).copied());
                    row
                })
                .collect();
            state.stats = stats.into_iter().map(StatRow::from).collect();
            state.phase = GameViewPhase::Ready;
            state.fade_in = 0.0;

            state.reveal_queue = state
                .achievements
                .iter()
                .map(|r| r.data.id.clone())
                .collect();

            state.tier_breakdown = compute_tier_breakdown(&state.achievements);

            Task::done(GameViewMessage::AchievementsFullyLoaded)
        }
        SteamReply::LoadFailed(e) => {
            state.phase = GameViewPhase::Error;
            state.error_message = e;
            Task::none()
        }
        SteamReply::ChangesSaved => {
            for row in &mut state.achievements {
                if row.is_dirty {
                    row.data.is_achieved = row.effective_achieved();
                    row.is_dirty = false;
                }
            }
            for row in &mut state.stats {
                if row.is_dirty {
                    row.data.original_value = row.data.value;
                    row.is_dirty = false;
                }
            }
            state.phase = GameViewPhase::Ready;
            state.banner = Some(Banner {
                kind: BannerKind::Success,
                message: "Changes saved to Steam.".to_owned(),
                dismissible: true,
            });
            Task::done(GameViewMessage::AchievementsFullyLoaded)
        }
        SteamReply::SaveFailed(e) => {
            state.phase = GameViewPhase::Ready;
            state.banner = Some(Banner {
                kind: BannerKind::Error,
                message: format!("Failed to save: {e}"),
                dismissible: true,
            });
            Task::done(GameViewMessage::AchievementsFullyLoaded)
        }
        SteamReply::IconUpdated { name, icon } => {
            if let Some(row) = state.achievements.iter_mut().find(|r| r.data.id == name) {
                row.data.icon = Some(icon);
            } else {
                state.pending_icons.insert(name, icon);
            }
            Task::none()
        }
        SteamReply::Disconnected => Task::none(),
        SteamReply::GlobalPercentagesReady(map) => {
            if state.achievements.is_empty() {
                state.pending_rarity_percent = Some(map);
                Task::none()
            } else {
                for row in &mut state.achievements {
                    if let Some(&pct) = map.get(&row.data.id) {
                        row.rarity_percent = Some(pct);
                    }
                }
                state.tier_breakdown = compute_tier_breakdown(&state.achievements);
                Task::done(GameViewMessage::AchievementsFullyLoaded)
            }
        }
        SteamReply::GlobalPercentagesFailed => Task::none(),
    }
}

pub(crate) fn compute_tier_breakdown(achievements: &[AchievementRow]) -> Vec<(RarityTier, u32)> {
    use std::collections::HashMap;
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

pub fn update(
    state: &mut GameViewState,
    message: GameViewMessage,
    ctx: &mut crate::app_context::AppContext,
) -> (Task<GameViewMessage>, GameViewEvent) {
    let worker = ctx.worker.as_ref();
    match message {
        GameViewMessage::AchievementToggled(id) => {
            if let Some(row) = state.achievements.iter_mut().find(|r| r.data.id == id) {
                if row.data.permission != 0 {
                    state.banner = Some(Banner {
                        kind: BannerKind::Warning,
                        message: "This achievement is protected and cannot be modified.".to_owned(),
                        dismissible: true,
                    });
                } else {
                    row.is_dirty = !row.is_dirty;
                }
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::FilterChanged(f) => {
            state.filter = f;
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::RarityTierToggled(tier) => {
            if state.rarity_tier_set.contains(&tier) {
                state.rarity_tier_set.remove(&tier);
            } else {
                state.rarity_tier_set.insert(tier);
            }
            let tiers: Vec<_> = state.rarity_tier_set.iter().copied().collect();
            let include_hidden = state.include_hidden;
            let _ = ctx.update_settings(|s| {
                s.manager.rarity_tiers = tiers;
                s.manager.include_hidden = include_hidden;
            });
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::HiddenPillToggled => {
            state.include_hidden = !state.include_hidden;
            let tiers: Vec<_> = state.rarity_tier_set.iter().copied().collect();
            let include_hidden = state.include_hidden;
            let _ = ctx.update_settings(|s| {
                s.manager.rarity_tiers = tiers;
                s.manager.include_hidden = include_hidden;
            });
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::RarityFilterCleared => {
            state.rarity_tier_set.clear();
            state.include_hidden = false;
            let _ = ctx.update_settings(|s| {
                s.manager.rarity_tiers = Vec::new();
                s.manager.include_hidden = false;
            });
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::AchievementSortChanged(s) => {
            let sort = s;
            let _ = ctx.update_settings(|s| s.manager.sort = sort);
            state.achievement_sort = s;
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::SearchChanged(q) => {
            state.search_query = q;
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::StatsSearchChanged(q) => {
            state.stats_search_query = q;
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::StatsMaxAll => {
            for stat in &mut state.stats {
                let Some(max) = stat.data.max_value else {
                    continue;
                };
                let new_value = match stat.data.value {
                    StatValue::Int(_) => StatValue::Int(max as i32),
                    StatValue::Float(_) => StatValue::Float(max as f32),
                };
                stat.data.value = new_value;
                stat.edit_text = new_value.to_edit_string();
                stat.is_dirty = new_value != stat.data.original_value;
                stat.edit_error = None;
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::StatsResetAll => {
            for stat in &mut state.stats {
                let default = stat.data.default_value.unwrap_or(0);
                let new_value = match stat.data.value {
                    StatValue::Int(_) => StatValue::Int(default as i32),
                    StatValue::Float(_) => StatValue::Float(default as f32),
                };
                stat.data.value = new_value;
                stat.edit_text = new_value.to_edit_string();
                stat.is_dirty = new_value != stat.data.original_value;
                stat.edit_error = None;
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::StatsResetSingle(id) => {
            if let Some(stat) = state.stats.iter_mut().find(|s| s.data.id == id) {
                let default = stat.data.default_value.unwrap_or(0);
                let new_value = match stat.data.value {
                    StatValue::Int(_) => StatValue::Int(default as i32),
                    StatValue::Float(_) => StatValue::Float(default as f32),
                };
                stat.data.value = new_value;
                stat.edit_text = new_value.to_edit_string();
                stat.is_dirty = new_value != stat.data.original_value;
                stat.edit_error = None;
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::StatsMaxSingle(id) => {
            if let Some(stat) = state.stats.iter_mut().find(|s| s.data.id == id)
                && let Some(max) = stat.data.max_value
            {
                let new_value = match stat.data.value {
                    StatValue::Int(_) => StatValue::Int(max as i32),
                    StatValue::Float(_) => StatValue::Float(max as f32),
                };
                stat.data.value = new_value;
                stat.edit_text = new_value.to_edit_string();
                stat.is_dirty = new_value != stat.data.original_value;
                stat.edit_error = None;
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::BulkAction(op) => {
            let visible: std::collections::HashSet<String> = visible_achievement_ids(
                &state.achievements,
                state.filter,
                &state.search_query,
                state.achievement_sort,
                &state.rarity_tier_set,
                state.include_hidden,
            )
            .into_iter()
            .map(|s| s.to_owned())
            .collect();

            for row in &mut state.achievements {
                if row.data.permission != 0 {
                    continue;
                }
                if !visible.contains(&row.data.id) {
                    continue;
                }
                match op {
                    BulkOp::Unlock => {
                        row.is_dirty = !row.data.is_achieved;
                    }
                    BulkOp::Lock => {
                        row.is_dirty = row.data.is_achieved;
                    }
                    BulkOp::Invert => {
                        row.is_dirty = !row.is_dirty;
                    }
                }
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::ReloadRequested => {
            state.phase = GameViewPhase::WaitingStats;
            state.achievements.clear();
            state.stats.clear();
            state.reveal_queue.clear();
            state.banner = None;
            if let Some(w) = worker {
                let steam_running = ctx.connectivity.steam_running.unwrap_or(false);
                let user_logged_in = ctx.connectivity.user_logged_in.unwrap_or(false);
                if let Err(e) = w.send_checked(
                    SteamRequest::RequestUserStats,
                    steam_running,
                    user_logged_in,
                ) {
                    surface_connectivity_error(ctx, e);
                    return (Task::none(), GameViewEvent::None);
                }
                if let Err(e) = w.send_checked(
                    SteamRequest::RequestGlobalPercentages,
                    steam_running,
                    user_logged_in,
                ) {
                    surface_connectivity_error(ctx, e);
                    return (Task::none(), GameViewEvent::None);
                }
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::ApplyClicked => {
            if state.cache_only || state.dirty_count() == 0 || state.has_stat_errors() {
                return (Task::none(), GameViewEvent::None);
            }
            state.apply_confirm_input.clear();
            state.show_apply_modal = true;
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::ApplyConfirmInputChanged(text) => {
            state.apply_confirm_input = text;
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::ApplyCancelled => {
            state.show_apply_modal = false;
            state.apply_confirm_input.clear();
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::ApplyConfirmed => {
            if !state.apply_confirm_matches() {
                return (Task::none(), GameViewEvent::None);
            }
            state.show_apply_modal = false;
            state.apply_confirm_input.clear();
            if state.dirty_count() == 0 || state.has_stat_errors() {
                return (Task::none(), GameViewEvent::None);
            }
            let payload = build_apply_payload(&state.achievements, &state.stats);
            state.phase = GameViewPhase::Saving;
            if let Some(w) = worker {
                let steam_running = ctx.connectivity.steam_running.unwrap_or(false);
                let user_logged_in = ctx.connectivity.user_logged_in.unwrap_or(false);
                if let Err(e) = w.send_checked(
                    SteamRequest::ApplyChanges {
                        achievements_to_set: payload.achievements_to_set,
                        achievements_to_clear: payload.achievements_to_clear,
                        stats_int: payload.stats_int,
                        stats_float: payload.stats_float,
                    },
                    steam_running,
                    user_logged_in,
                ) {
                    surface_connectivity_error(ctx, e);
                    state.phase = GameViewPhase::Ready;
                    return (
                        Task::none(),
                        GameViewEvent::AchievementsFullyLoaded {
                            app_id: state.app_id,
                        },
                    );
                }
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::BannerDismissed => {
            state.banner = None;
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::DiscardChanges => {
            for row in &mut state.achievements {
                row.is_dirty = false;
            }
            for row in &mut state.stats {
                row.data.value = row.data.original_value;
                row.edit_text = row.data.original_value.to_edit_string();
                row.is_dirty = false;
                row.edit_error = None;
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::RevealHidden(id) => {
            if let Some(row) = state.achievements.iter_mut().find(|r| r.data.id == id) {
                row.revealed = true;
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::SpinnerTick => {
            state.spinner_angle = (state.spinner_angle + 6.0) % 360.0;
            if state.phase == GameViewPhase::Ready && state.fade_in < 1.0 {
                state.fade_in = (state.fade_in + 0.08).min(1.0);
            }
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::RevealTick => {
            if let Some(id) = state.reveal_queue.pop_front()
                && let Some(row) = state.achievements.iter_mut().find(|r| r.data.id == id)
            {
                row.appeared = true;
            }
            (Task::none(), GameViewEvent::None)
        }

        GameViewMessage::GameViewFadeTick => {
            for row in &mut state.achievements {
                if row.appeared && row.card_opacity < 1.0 {
                    row.card_opacity = (row.card_opacity + MANAGER_FADE_DELTA).min(1.0);
                }
            }
            (Task::none(), GameViewEvent::None)
        }

        GameViewMessage::RareGlowTick => {
            state.rare_glow_phase = (state.rare_glow_phase + 0.12) % (2.0 * std::f32::consts::PI);
            (Task::none(), GameViewEvent::None)
        }

        GameViewMessage::AchievementsFullyLoaded => (
            Task::none(),
            GameViewEvent::AchievementsFullyLoaded {
                app_id: state.app_id,
            },
        ),
        GameViewMessage::RequestGoBack => (Task::none(), GameViewEvent::GoBack),
        GameViewMessage::CapsuleLoaded {
            app_id,
            size,
            handle,
            width,
            height,
        } => {
            state.capsule_handles.insert(
                (app_id, size),
                crate::profile_view::types::StoredCapsule {
                    handle,
                    width,
                    height,
                },
            );
            (Task::none(), GameViewEvent::None)
        }
        GameViewMessage::CapsuleFailed { app_id, size } => {
            crate::log!("game_view: capsule fetch failed for app_id={app_id} size={size:?}");
            (Task::none(), GameViewEvent::None)
        }

        GameViewMessage::BarSliceHoverEnter(tier) => {
            state.hovered_bar_slice = Some(tier);
            (Task::none(), GameViewEvent::None)
        }

        GameViewMessage::BarSliceHoverExit => {
            state.hovered_bar_slice = None;
            (Task::none(), GameViewEvent::None)
        }

        GameViewMessage::InvalidateCacheClicked(app_id) => {
            (Task::none(), GameViewEvent::InvalidateCache { app_id })
        }
    }
}

pub fn view(
    state: &GameViewState,
    props: view::GameViewProps,
) -> iced::Element<'_, GameViewMessage> {
    view::render(state, props)
}

pub fn subscription(state: &GameViewState) -> iced::Subscription<GameViewMessage> {
    use iced::time;

    let needs_spinner = matches!(
        state.phase,
        GameViewPhase::Connecting | GameViewPhase::WaitingStats | GameViewPhase::Saving
    );

    let needs_tick = needs_spinner
        || (state.phase == GameViewPhase::Ready && state.fade_in < 1.0)
        || (state.phase == GameViewPhase::Ready && state.has_pending_reveals())
        || (state.phase == GameViewPhase::Ready && state.has_fading_cards());

    let spinner_sub = if needs_tick {
        time::every(std::time::Duration::from_millis(33)).map(|_| GameViewMessage::SpinnerTick)
    } else {
        iced::Subscription::none()
    };

    let reveal_sub = if state.has_pending_reveals() {
        time::every(std::time::Duration::from_millis(30)).map(|_| GameViewMessage::RevealTick)
    } else {
        iced::Subscription::none()
    };

    let fade_sub = if state.has_fading_cards() {
        time::every(std::time::Duration::from_millis(33)).map(|_| GameViewMessage::GameViewFadeTick)
    } else {
        iced::Subscription::none()
    };

    let tier_map = compute_tier_map(&state.achievements);
    let has_legendary = state.phase == GameViewPhase::Ready
        && state.achievements.iter().any(|r| {
            r.appeared
                && tier_map
                    .get(&r.data.id)
                    .is_some_and(|&t| t == types::RarityTier::Legendary)
        });
    let glow_sub = if has_legendary {
        time::every(std::time::Duration::from_millis(40)).map(|_| GameViewMessage::RareGlowTick)
    } else {
        iced::Subscription::none()
    };

    iced::Subscription::batch([spinner_sub, reveal_sub, fade_sub, glow_sub])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_context::{AnimationState, AppContext, ConnectivityState};
    use crate::messaging::MessagingCenter;
    use crate::settings::Settings;
    use crate::steam_worker::SteamReply;
    use std::collections::{HashMap, VecDeque};
    use steamlens_core::AchievementIcon;
    use types::{AchievementData, AchievementRow};

    fn make_test_ctx() -> AppContext {
        AppContext {
            worker: None,
            worker_rx: None,
            settings: Settings::default(),
            settings_dirty_since: None,
            messaging: MessagingCenter::new(),
            cached_entries: HashMap::new(),
            pending_hit_queue: VecDeque::new(),
            steam_root: std::path::PathBuf::from("/tmp"),
            steamid3: 0,
            user_profile: None,
            profile_avatar_handle: None,
            connectivity: ConnectivityState {
                steam_running: Some(true),
                user_logged_in: Some(true),
            },
            steam_level: None,
            no_ach_cache: crate::cache::NoAchievementsCache::new(),
            animation: AnimationState {
                skeleton_phase: 0.0,
            },
        }
    }

    fn make_state_with_achievement(id: &str) -> GameViewState {
        let mut state = GameViewState::new(0);
        state.achievements = vec![AchievementRow::from(AchievementData {
            id: id.to_owned(),
            display_name: id.to_owned(),
            description: String::new(),
            is_hidden: false,
            is_achieved: false,
            unlock_time: None,
            permission: 0,
            icon: None,
        })];
        state
    }

    fn sample_icon() -> AchievementIcon {
        AchievementIcon {
            width: 2,
            height: 2,
            rgba: vec![255u8; 16],
        }
    }

    #[test]
    fn icon_updated_replaces_row_icon() {
        let mut state = make_state_with_achievement("ACH_FOO");
        assert!(state.achievements[0].data.icon.is_none());

        let reply = SteamReply::IconUpdated {
            name: "ACH_FOO".to_owned(),
            icon: sample_icon(),
        };
        let _task = handle_steam_reply(&mut state, reply);

        let icon = state.achievements[0]
            .data
            .icon
            .as_ref()
            .expect("icon should be set");
        assert_eq!(icon.width, 2);
        assert_eq!(icon.height, 2);
        assert_eq!(icon.rgba.len(), 16);
    }

    #[test]
    fn icon_updated_unknown_name_is_noop() {
        let mut state = make_state_with_achievement("ACH_FOO");

        let reply = SteamReply::IconUpdated {
            name: "ACH_NONEXISTENT".to_owned(),
            icon: sample_icon(),
        };
        let _task = handle_steam_reply(&mut state, reply);

        assert!(state.achievements[0].data.icon.is_none());
    }

    #[test]
    fn game_view_reveal_tick_pops_one_from_queue() {
        use std::collections::VecDeque;

        let mut state = GameViewState::new(0);
        for i in 1u8..=3 {
            let id = format!("ACH_{i}");
            state
                .achievements
                .push(types::AchievementRow::from(types::AchievementData {
                    id: id.clone(),
                    display_name: id.clone(),
                    description: String::new(),
                    is_hidden: false,
                    is_achieved: false,
                    unlock_time: None,
                    permission: 0,
                    icon: None,
                }));
        }
        state.reveal_queue =
            VecDeque::from(["ACH_1".to_owned(), "ACH_2".to_owned(), "ACH_3".to_owned()]);
        state.phase = GameViewPhase::Ready;

        assert!(state.has_pending_reveals(), "precondition: queue not empty");
        assert!(
            state.achievements.iter().all(|r| !r.appeared),
            "precondition: none appeared"
        );

        let mut ctx = make_test_ctx();
        for expected_remaining in [2usize, 1, 0] {
            let _task = update(&mut state, GameViewMessage::RevealTick, &mut ctx);
            assert_eq!(
                state.reveal_queue.len(),
                expected_remaining,
                "queue length after pop"
            );
        }

        assert!(
            state.achievements.iter().all(|r| r.appeared),
            "all 3 achievements must be appeared after 3 RevealTick calls"
        );
        assert!(
            !state.has_pending_reveals(),
            "has_pending_reveals must be false when queue is empty"
        );
    }

    #[test]
    fn game_subscription_constructs_with_default_state() {
        let state = GameViewState::new(0);
        let _: iced::Subscription<GameViewMessage> = subscription(&state);
    }
}
