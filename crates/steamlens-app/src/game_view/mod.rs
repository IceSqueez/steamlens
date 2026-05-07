pub mod types;
mod view;

pub use view::achievement_search_id;

use std::collections::{HashSet, VecDeque};

use iced::Task;

use crate::steam_worker::{SteamReply, SteamRequest, SteamWorker};

use types::{
    AchievementFilter, AchievementRow, AchievementSort, ActiveTab, Banner, BannerKind, BulkOp,
    RarityTier, ResetScope, StatRow, build_apply_payload, compute_tier_map, dirty_count,
    has_stat_errors, visible_achievement_ids,
};

pub(crate) const MANAGER_FADE_DELTA: f32 = 0.2;

#[derive(Debug, Clone)]
pub enum GameViewMessage {
    AchievementToggled(String),
    StatEdited(String, String),
    StatEditCommitted(String),
    FilterChanged(AchievementFilter),
    RarityTierToggled(RarityTier),
    RarityFilterCleared,
    HiddenPillToggled,
    AchievementSortChanged(AchievementSort),
    SearchChanged(String),
    TabChanged(ActiveTab),
    StatsConsentToggled(bool),
    BulkAction(BulkOp),
    ReloadRequested,
    ApplyChanges,
    ResetClicked,
    ResetScopeSelected(ResetScope),
    ResetConfirmInputChanged(String),
    ResetConfirmed,
    ResetCancelled,
    BannerDismissed,
    DiscardChanges,
    RevealHidden(String),
    SpinnerTick,
    RevealTick,
    GameViewFadeTick,
    RareGlowTick,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameViewPhase {
    Connecting,
    WaitingStats,
    LoadingData,
    Ready,
    Saving,
    Resetting,
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

    pub active_tab: ActiveTab,
    pub search_query: String,
    pub filter: AchievementFilter,
    pub achievement_sort: AchievementSort,
    pub rarity_tier_set: HashSet<RarityTier>,
    pub include_hidden: bool,
    pub stats_edit_consent: bool,

    pub reset_scope: ResetScope,
    pub reset_confirm_input: String,
    pub show_reset_modal: bool,

    pub banner: Option<Banner>,

    pub spinner_angle: f32,
    pub fade_in: f32,
    pub rare_glow_phase: f32,

    pub error_message: String,
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
            active_tab: ActiveTab::Achievements,
            search_query: String::new(),
            filter: AchievementFilter::All,
            achievement_sort: AchievementSort::UnlockChance,
            rarity_tier_set: HashSet::new(),
            include_hidden: false,
            stats_edit_consent: false,
            reset_scope: ResetScope::Pending,
            reset_confirm_input: String::new(),
            show_reset_modal: false,
            banner: None,
            spinner_angle: 0.0,
            fade_in: 0.0,
            rare_glow_phase: 0.0,
            error_message: String::new(),
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

    pub fn reset_confirm_matches(&self) -> bool {
        self.reset_scope != ResetScope::Pending
            && self
                .reset_confirm_input
                .trim()
                .eq_ignore_ascii_case(self.game_name.trim())
    }

    pub fn has_fading_cards(&self) -> bool {
        self.achievements
            .iter()
            .any(|r| r.appeared && r.card_opacity < 1.0)
    }
}

pub fn handle_steam_reply(state: &mut GameViewState, reply: SteamReply) -> Task<crate::Message> {
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
        SteamReply::StatsRequested => {
            state.phase = GameViewPhase::LoadingData;
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
            genre: _,
        } => {
            let prev_revealed: std::collections::HashSet<String> = state
                .achievements
                .iter()
                .filter(|r| r.revealed)
                .map(|r| r.data.id.clone())
                .collect();
            state.achievements = achievements
                .into_iter()
                .map(|data| {
                    let mut row = AchievementRow::from_data(data);
                    if prev_revealed.contains(&row.data.id) {
                        row.revealed = true;
                    }
                    row
                })
                .collect();
            state.stats = stats.into_iter().map(StatRow::from_data).collect();
            state.phase = GameViewPhase::Ready;
            state.fade_in = 0.0;

            state.reveal_queue = state
                .achievements
                .iter()
                .map(|r| r.data.id.clone())
                .collect();

            state.tier_breakdown = compute_tier_breakdown(&state.achievements);

            Task::none()
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
            Task::none()
        }
        SteamReply::SaveFailed(e) => {
            state.phase = GameViewPhase::Ready;
            state.banner = Some(Banner {
                kind: BannerKind::Error,
                message: format!("Failed to save: {e}"),
                dismissible: true,
            });
            Task::none()
        }
        SteamReply::ResetDone => {
            state.phase = GameViewPhase::LoadingData;
            Task::none()
        }
        SteamReply::ResetFailed(e) => {
            state.phase = GameViewPhase::Ready;
            state.banner = Some(Banner {
                kind: BannerKind::Error,
                message: format!("Reset failed: {e}"),
                dismissible: true,
            });
            Task::none()
        }
        SteamReply::IconUpdated { name, icon } => {
            if let Some(row) = state.achievements.iter_mut().find(|r| r.data.id == name) {
                row.data.icon = Some(icon);
            }
            Task::none()
        }
        SteamReply::Callback(cb) => {
            use steamlens_core::SteamCallback;
            if let SteamCallback::UserStatsReceived { result, .. } = &cb {
                if result.is_ok() && state.phase == GameViewPhase::WaitingStats {
                    state.phase = GameViewPhase::LoadingData;
                } else if !result.is_ok() && state.phase == GameViewPhase::WaitingStats {
                    state.phase = GameViewPhase::Error;
                    state.error_message =
                        format!("Steam returned error {} for RequestUserStats", result.raw());
                }
            }
            Task::none()
        }
        SteamReply::Disconnected => Task::none(),
        SteamReply::GlobalPercentagesReady(map) => {
            for row in &mut state.achievements {
                if let Some(&pct) = map.get(&row.data.id) {
                    row.rarity_percent = Some(pct);
                }
            }
            Task::none()
        }
        SteamReply::GlobalPercentagesFailed(_) => Task::none(),
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
    worker: &SteamWorker,
) -> Task<crate::Message> {
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
            Task::none()
        }
        GameViewMessage::StatEdited(id, text) => {
            if let Some(row) = state.stats.iter_mut().find(|r| r.data.id == id) {
                row.edit_text = text;
            }
            Task::none()
        }
        GameViewMessage::StatEditCommitted(id) => {
            if let Some(row) = state.stats.iter_mut().find(|r| r.data.id == id) {
                row.validate_and_parse();
            }
            Task::none()
        }
        GameViewMessage::FilterChanged(f) => {
            state.filter = f;
            Task::none()
        }
        GameViewMessage::RarityTierToggled(tier) => {
            if state.rarity_tier_set.contains(&tier) {
                state.rarity_tier_set.remove(&tier);
            } else {
                state.rarity_tier_set.insert(tier);
            }
            Task::none()
        }
        GameViewMessage::RarityFilterCleared => {
            state.rarity_tier_set.clear();
            state.include_hidden = false;
            Task::none()
        }
        GameViewMessage::HiddenPillToggled => {
            state.include_hidden = !state.include_hidden;
            Task::none()
        }
        GameViewMessage::AchievementSortChanged(s) => {
            state.achievement_sort = s;
            Task::none()
        }
        GameViewMessage::SearchChanged(q) => {
            state.search_query = q;
            Task::none()
        }
        GameViewMessage::TabChanged(tab) => {
            state.active_tab = tab;
            Task::none()
        }
        GameViewMessage::StatsConsentToggled(v) => {
            state.stats_edit_consent = v;
            if !v {
                for row in &mut state.stats {
                    if row.is_dirty {
                        row.edit_text = row.data.original_value.to_edit_string();
                        row.data.value = row.data.original_value;
                        row.is_dirty = false;
                        row.edit_error = None;
                    }
                }
            }
            Task::none()
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
            Task::none()
        }
        GameViewMessage::ReloadRequested => {
            state.phase = GameViewPhase::WaitingStats;
            state.achievements.clear();
            state.stats.clear();
            state.reveal_queue.clear();
            state.banner = None;
            worker.send(SteamRequest::RequestUserStats);
            worker.send(SteamRequest::RequestGlobalPercentages);
            Task::none()
        }
        GameViewMessage::ApplyChanges => {
            if state.dirty_count() == 0 || state.has_stat_errors() {
                return Task::none();
            }
            let payload = build_apply_payload(&state.achievements, &state.stats);
            state.phase = GameViewPhase::Saving;
            worker.send(SteamRequest::ApplyChanges {
                achievements_to_set: payload.achievements_to_set,
                achievements_to_clear: payload.achievements_to_clear,
                stats_int: payload.stats_int,
                stats_float: payload.stats_float,
            });
            Task::none()
        }
        GameViewMessage::ResetClicked => {
            state.reset_scope = ResetScope::StatsOnly;
            state.reset_confirm_input.clear();
            state.show_reset_modal = true;
            Task::none()
        }
        GameViewMessage::ResetScopeSelected(scope) => {
            state.reset_scope = scope;
            Task::none()
        }
        GameViewMessage::ResetConfirmInputChanged(text) => {
            state.reset_confirm_input = text;
            Task::none()
        }
        GameViewMessage::ResetConfirmed => {
            state.show_reset_modal = false;
            state.reset_confirm_input.clear();
            state.phase = GameViewPhase::Resetting;
            worker.send(SteamRequest::ResetAll {
                scope: state.reset_scope,
            });
            Task::none()
        }
        GameViewMessage::ResetCancelled => {
            state.show_reset_modal = false;
            state.reset_confirm_input.clear();
            Task::none()
        }
        GameViewMessage::BannerDismissed => {
            state.banner = None;
            Task::none()
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
            Task::none()
        }
        GameViewMessage::RevealHidden(id) => {
            if let Some(row) = state.achievements.iter_mut().find(|r| r.data.id == id) {
                row.revealed = true;
            }
            Task::none()
        }
        GameViewMessage::SpinnerTick => {
            state.spinner_angle = (state.spinner_angle + 6.0) % 360.0;
            if state.phase == GameViewPhase::Ready && state.fade_in < 1.0 {
                state.fade_in = (state.fade_in + 0.08).min(1.0);
            }
            Task::none()
        }
        GameViewMessage::RevealTick => {
            if let Some(id) = state.reveal_queue.pop_front()
                && let Some(row) = state.achievements.iter_mut().find(|r| r.data.id == id)
            {
                row.appeared = true;
            }
            Task::none()
        }

        GameViewMessage::GameViewFadeTick => {
            for row in &mut state.achievements {
                if row.appeared && row.card_opacity < 1.0 {
                    row.card_opacity = (row.card_opacity + MANAGER_FADE_DELTA).min(1.0);
                }
            }
            Task::none()
        }

        GameViewMessage::RareGlowTick => {
            state.rare_glow_phase = (state.rare_glow_phase + 0.12) % (2.0 * std::f32::consts::PI);
            Task::none()
        }
    }
}

pub fn view(state: &GameViewState, skeleton_phase: f32) -> iced::Element<'_, crate::Message> {
    view::render(state, skeleton_phase)
}

pub fn subscription(state: &GameViewState) -> iced::Subscription<crate::Message> {
    use iced::time;

    let needs_spinner = matches!(
        state.phase,
        GameViewPhase::Connecting
            | GameViewPhase::WaitingStats
            | GameViewPhase::LoadingData
            | GameViewPhase::Saving
            | GameViewPhase::Resetting
    );

    let needs_tick = needs_spinner
        || (state.phase == GameViewPhase::Ready && state.fade_in < 1.0)
        || (state.phase == GameViewPhase::Ready && state.has_pending_reveals())
        || (state.phase == GameViewPhase::Ready && state.has_fading_cards());

    let spinner_sub = if needs_tick {
        time::every(std::time::Duration::from_millis(33))
            .map(|_| crate::Message::GameView(GameViewMessage::SpinnerTick))
    } else {
        iced::Subscription::none()
    };

    let reveal_sub = if state.has_pending_reveals() {
        time::every(std::time::Duration::from_millis(30))
            .map(|_| crate::Message::GameView(GameViewMessage::RevealTick))
    } else {
        iced::Subscription::none()
    };

    let fade_sub = if state.has_fading_cards() {
        time::every(std::time::Duration::from_millis(33))
            .map(|_| crate::Message::GameView(GameViewMessage::GameViewFadeTick))
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
        time::every(std::time::Duration::from_millis(40))
            .map(|_| crate::Message::GameView(GameViewMessage::RareGlowTick))
    } else {
        iced::Subscription::none()
    };

    iced::Subscription::batch([spinner_sub, reveal_sub, fade_sub, glow_sub])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steam_worker::SteamReply;
    use steamlens_core::AchievementIcon;
    use types::{AchievementData, AchievementRow};

    fn make_state_with_achievement(id: &str) -> GameViewState {
        let mut state = GameViewState::new(0);
        state.achievements = vec![AchievementRow::from_data(AchievementData {
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
                .push(types::AchievementRow::from_data(types::AchievementData {
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

        let worker = SteamWorker::new_disconnected();
        for expected_remaining in [2usize, 1, 0] {
            let _task = update(&mut state, GameViewMessage::RevealTick, &worker);
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
}
