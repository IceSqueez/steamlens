mod header;
mod messages;
mod state;
pub mod stats_panel;
pub mod types;
mod update;
mod view;
pub mod widget;

pub use header::header_content;
pub use messages::{GameViewEvent, GameViewMessage};
pub(crate) use state::compute_tier_breakdown;
pub use state::{GameViewPhase, GameViewState, SeededGameView};
pub use update::{handle_steam_reply, update};
pub use view::{GameViewProps, achievement_search_id};

pub fn view(
    state: &GameViewState,
    props: view::GameViewProps,
) -> iced::Element<'_, GameViewMessage> {
    view::render(state, props)
}

pub fn subscription(_state: &GameViewState) -> iced::Subscription<GameViewMessage> {
    iced::Subscription::none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_context::{AnimationState, AppContext, ConnectivityState};
    use crate::messaging::MessagingCenter;
    use crate::settings::Settings;
    use crate::steam_worker::SteamReply;
    use std::collections::{HashMap, VecDeque};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use steamlens_core::AchievementIcon;
    use tokio::sync::mpsc;
    use types::{AchievementData, AchievementRow};

    fn make_test_ctx() -> AppContext {
        let (reply_tx, reply_rx) = mpsc::unbounded_channel();
        AppContext {
            worker: None,
            worker_reply_tx: reply_tx,
            worker_reply_rx: Arc::new(Mutex::new(Some(reply_rx))),
            settings: Settings::default(),
            settings_dirty_since: None,
            messaging: MessagingCenter::new(),
            cached_entries: HashMap::new(),
            pending_hit_queue: VecDeque::new(),
            steam_root: PathBuf::from("/tmp"),
            steamid3: 0,
            user_profile: None,
            profile_avatar_handle: None,
            connectivity: ConnectivityState {
                steam_running: Some(true),
                user_logged_in: Some(true),
            },
            steam_level: None,
            no_ach_cache: crate::cache::NoAchievementsCache::new(),
            steam_state: HashMap::new(),
            steam_state_mtime: None,
            app_assets: HashMap::new(),
            animation: AnimationState::new(),
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
        let _task = handle_steam_reply(&mut state, reply, &mut make_test_ctx());

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
        let _task = handle_steam_reply(&mut state, reply, &mut make_test_ctx());

        assert!(state.achievements[0].data.icon.is_none());
    }

    #[test]
    fn tick_animations_drains_reveal_queue_via_accumulator() {
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

        // One 100ms frame = ceil(100 / 30) = 3 pops via accumulator.
        state.tick_animations(0.100);

        assert!(
            !state.has_pending_reveals(),
            "all 3 reveals must have popped in a single 100ms frame"
        );
        assert!(
            state.achievements.iter().all(|r| r.appeared),
            "all 3 achievements must be appeared after accumulator drains"
        );
    }

    #[test]
    fn game_subscription_constructs_with_default_state() {
        let state = GameViewState::new(0);
        let _: iced::Subscription<GameViewMessage> = subscription(&state);
    }
}
