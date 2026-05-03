pub mod types;
mod view;

use iced::Task;

use crate::steam_worker::{SteamReply, SteamRequest, SteamWorker};

use types::{
    AchievementFilter, AchievementRow, ActiveTab, Banner, BannerKind, BulkOp, ResetScope, StatRow,
    build_apply_payload, dirty_count, has_stat_errors, visible_achievement_ids,
};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ManagerMessage {
    StatsRequested,
    RequestStatsFailed(String),
    StatsReceived,
    StatsReceivedFailed(String),
    AchievementsLoaded(Vec<types::AchievementData>, Vec<types::StatData>),
    LoadFailed(String),

    AchievementToggled(String),
    StatEdited(String, String),
    StatEditCommitted(String),
    FilterChanged(AchievementFilter),
    SearchChanged(String),
    TabChanged(ActiveTab),
    StatsConsentToggled(bool),
    BulkAction(BulkOp),
    ReloadRequested,
    ApplyChanges,
    ChangesSaved,
    SaveFailed(String),
    ResetClicked,
    ResetScopeSelected(ResetScope),
    ResetConfirmed,
    ResetCancelled,
    ResetDone,
    ResetFailed(String),
    BannerDismissed,
    SpinnerTick,
    FadeInTick(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ManagerPhase {
    Connecting,
    WaitingStats,
    LoadingData,
    Ready,
    Saving,
    Resetting,
    Error,
}

pub struct ManagerState {
    pub app_id: u32,
    pub game_name: String,
    pub phase: ManagerPhase,

    pub achievements: Vec<AchievementRow>,
    pub stats: Vec<StatRow>,

    pub active_tab: ActiveTab,
    pub search_query: String,
    pub filter: AchievementFilter,
    pub stats_edit_consent: bool,

    pub reset_scope: ResetScope,
    pub show_reset_modal: bool,

    pub banner: Option<Banner>,

    pub spinner_angle: f32,
    pub fade_in: f32,

    pub error_message: String,
}

impl ManagerState {
    pub fn new(app_id: u32) -> Self {
        Self {
            app_id,
            game_name: format!("App {app_id}"),
            phase: ManagerPhase::Connecting,
            achievements: Vec::new(),
            stats: Vec::new(),
            active_tab: ActiveTab::Achievements,
            search_query: String::new(),
            filter: AchievementFilter::All,
            stats_edit_consent: false,
            reset_scope: ResetScope::Pending,
            show_reset_modal: false,
            banner: None,
            spinner_angle: 0.0,
            fade_in: 0.0,
            error_message: String::new(),
        }
    }

    pub fn dirty_count(&self) -> usize {
        dirty_count(&self.achievements, &self.stats)
    }

    pub fn has_stat_errors(&self) -> bool {
        has_stat_errors(&self.stats)
    }
}

pub fn handle_steam_reply(state: &mut ManagerState, reply: SteamReply) -> Task<crate::Message> {
    match reply {
        SteamReply::Connected { .. } => {
            state.phase = ManagerPhase::WaitingStats;
            Task::none()
        }
        SteamReply::ConnectFailed(e) => {
            state.phase = ManagerPhase::Error;
            state.error_message = e;
            Task::none()
        }
        SteamReply::StatsRequested => {
            state.phase = ManagerPhase::LoadingData;
            Task::none()
        }
        SteamReply::RequestStatsFailed(e) => {
            state.phase = ManagerPhase::Error;
            state.error_message = e;
            Task::none()
        }
        SteamReply::AchievementsAndStats {
            achievements,
            stats,
        } => {
            state.achievements = achievements
                .into_iter()
                .map(AchievementRow::from_data)
                .collect();
            state.stats = stats.into_iter().map(StatRow::from_data).collect();
            state.phase = ManagerPhase::Ready;
            state.fade_in = 0.0;
            Task::none()
        }
        SteamReply::LoadFailed(e) => {
            state.phase = ManagerPhase::Error;
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
            state.phase = ManagerPhase::Ready;
            state.banner = Some(Banner {
                kind: BannerKind::Success,
                message: "Changes saved to Steam.".to_owned(),
                dismissible: true,
            });
            Task::none()
        }
        SteamReply::SaveFailed(e) => {
            state.phase = ManagerPhase::Ready;
            state.banner = Some(Banner {
                kind: BannerKind::Error,
                message: format!("Failed to save: {e}"),
                dismissible: true,
            });
            Task::none()
        }
        SteamReply::ResetDone => {
            state.phase = ManagerPhase::LoadingData;
            Task::none()
        }
        SteamReply::ResetFailed(e) => {
            state.phase = ManagerPhase::Ready;
            state.banner = Some(Banner {
                kind: BannerKind::Error,
                message: format!("Reset failed: {e}"),
                dismissible: true,
            });
            Task::none()
        }
        SteamReply::Callback(cb) => {
            use steamlens_core::SteamCallback;
            if let SteamCallback::UserStatsReceived { result, .. } = &cb {
                if result.is_ok() && state.phase == ManagerPhase::WaitingStats {
                    state.phase = ManagerPhase::LoadingData;
                } else if !result.is_ok() && state.phase == ManagerPhase::WaitingStats {
                    state.phase = ManagerPhase::Error;
                    state.error_message =
                        format!("Steam returned error {} for RequestUserStats", result.raw());
                }
            }
            Task::none()
        }
        SteamReply::Disconnected => Task::none(),
    }
}

pub fn update(
    state: &mut ManagerState,
    message: ManagerMessage,
    worker: &SteamWorker,
) -> Task<crate::Message> {
    match message {
        ManagerMessage::StatsRequested => Task::none(),
        ManagerMessage::RequestStatsFailed(e) => {
            state.phase = ManagerPhase::Error;
            state.error_message = e;
            Task::none()
        }
        ManagerMessage::StatsReceived => Task::none(),
        ManagerMessage::StatsReceivedFailed(e) => {
            state.phase = ManagerPhase::Error;
            state.error_message = e;
            Task::none()
        }
        ManagerMessage::AchievementsLoaded(achievements, stats) => {
            state.achievements = achievements
                .into_iter()
                .map(AchievementRow::from_data)
                .collect();
            state.stats = stats.into_iter().map(StatRow::from_data).collect();
            state.phase = ManagerPhase::Ready;
            state.fade_in = 0.0;
            Task::none()
        }
        ManagerMessage::LoadFailed(e) => {
            state.phase = ManagerPhase::Error;
            state.error_message = e;
            Task::none()
        }
        ManagerMessage::AchievementToggled(id) => {
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
        ManagerMessage::StatEdited(id, text) => {
            if let Some(row) = state.stats.iter_mut().find(|r| r.data.id == id) {
                row.edit_text = text;
            }
            Task::none()
        }
        ManagerMessage::StatEditCommitted(id) => {
            if let Some(row) = state.stats.iter_mut().find(|r| r.data.id == id) {
                row.validate_and_parse();
            }
            Task::none()
        }
        ManagerMessage::FilterChanged(f) => {
            state.filter = f;
            Task::none()
        }
        ManagerMessage::SearchChanged(q) => {
            state.search_query = q;
            Task::none()
        }
        ManagerMessage::TabChanged(tab) => {
            state.active_tab = tab;
            Task::none()
        }
        ManagerMessage::StatsConsentToggled(v) => {
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
        ManagerMessage::BulkAction(op) => {
            let visible: std::collections::HashSet<String> =
                visible_achievement_ids(&state.achievements, state.filter, &state.search_query)
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
        ManagerMessage::ReloadRequested => {
            state.phase = ManagerPhase::WaitingStats;
            state.achievements.clear();
            state.stats.clear();
            state.banner = None;
            let steam_id_placeholder = 0u64;
            worker.send(SteamRequest::RequestUserStats);
            let _ = steam_id_placeholder;
            Task::none()
        }
        ManagerMessage::ApplyChanges => {
            if state.dirty_count() == 0 || state.has_stat_errors() {
                return Task::none();
            }
            let payload = build_apply_payload(&state.achievements, &state.stats);
            state.phase = ManagerPhase::Saving;
            worker.send(SteamRequest::ApplyChanges {
                achievements_to_set: payload.achievements_to_set,
                achievements_to_clear: payload.achievements_to_clear,
                stats_int: payload.stats_int,
                stats_float: payload.stats_float,
            });
            Task::none()
        }
        ManagerMessage::ChangesSaved => {
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
            state.phase = ManagerPhase::Ready;
            state.banner = Some(Banner {
                kind: BannerKind::Success,
                message: "Changes saved to Steam.".to_owned(),
                dismissible: true,
            });
            Task::none()
        }
        ManagerMessage::SaveFailed(e) => {
            state.phase = ManagerPhase::Ready;
            state.banner = Some(Banner {
                kind: BannerKind::Error,
                message: format!("Failed to save: {e}"),
                dismissible: true,
            });
            Task::none()
        }
        ManagerMessage::ResetClicked => {
            state.reset_scope = ResetScope::Pending;
            state.show_reset_modal = true;
            Task::none()
        }
        ManagerMessage::ResetScopeSelected(scope) => {
            state.reset_scope = scope;
            Task::none()
        }
        ManagerMessage::ResetConfirmed => {
            let achievements_too = state.reset_scope == ResetScope::StatsAndAchievements;
            state.show_reset_modal = false;
            state.phase = ManagerPhase::Resetting;
            worker.send(SteamRequest::ResetAll { achievements_too });
            Task::none()
        }
        ManagerMessage::ResetCancelled => {
            state.show_reset_modal = false;
            Task::none()
        }
        ManagerMessage::ResetDone => {
            state.phase = ManagerPhase::WaitingStats;
            state.achievements.clear();
            state.stats.clear();
            for row in &mut state.achievements {
                row.is_dirty = false;
            }
            for row in &mut state.stats {
                row.is_dirty = false;
                row.edit_error = None;
            }
            worker.send(SteamRequest::RequestUserStats);
            Task::none()
        }
        ManagerMessage::ResetFailed(e) => {
            state.phase = ManagerPhase::Ready;
            state.banner = Some(Banner {
                kind: BannerKind::Error,
                message: format!("Reset failed: {e}"),
                dismissible: true,
            });
            Task::none()
        }
        ManagerMessage::BannerDismissed => {
            state.banner = None;
            Task::none()
        }
        ManagerMessage::SpinnerTick => {
            state.spinner_angle = (state.spinner_angle + 6.0) % 360.0;
            if state.phase == ManagerPhase::Ready && state.fade_in < 1.0 {
                state.fade_in = (state.fade_in + 0.08).min(1.0);
            }
            Task::none()
        }
        ManagerMessage::FadeInTick(_) => Task::none(),
    }
}

pub fn view(state: &ManagerState) -> iced::Element<'_, crate::Message> {
    view::render(state)
}

pub fn subscription(state: &ManagerState) -> iced::Subscription<crate::Message> {
    use iced::time;

    let needs_spinner = matches!(
        state.phase,
        ManagerPhase::Connecting
            | ManagerPhase::WaitingStats
            | ManagerPhase::LoadingData
            | ManagerPhase::Saving
            | ManagerPhase::Resetting
    );

    let needs_tick = needs_spinner || (state.phase == ManagerPhase::Ready && state.fade_in < 1.0);

    if needs_tick {
        time::every(std::time::Duration::from_millis(33))
            .map(|_| crate::Message::Manager(ManagerMessage::SpinnerTick))
    } else {
        iced::Subscription::none()
    }
}
