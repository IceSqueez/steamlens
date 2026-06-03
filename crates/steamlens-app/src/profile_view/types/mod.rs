mod entries;
mod filters;
mod messages;
mod sort;
mod state;

pub use entries::{CapsuleAsset, GameEntry, StoredCapsule, TopEntry};
pub use filters::{GameStatusFilter, LibrarySort};
pub use messages::{ProfileEvent, ProfileViewMessage};
pub(crate) use sort::sort_games_in_place;
#[cfg(test)]
pub use state::LoaderPhase;
pub use state::{ProfileViewPhase, ProfileViewState, SharedProgressRx};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capsule_cache::CapsuleSize;
    use crate::progress_scan::ProgressData;
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};

    fn make_entry(app_id: u32, name: &str, last_played: Option<u32>) -> GameEntry {
        GameEntry {
            app_id,
            change_number: 0,
            last_played,
            name: Some(name.to_owned()),
            capsule: CapsuleAsset::Pending,
            progress: None,
            genre: None,
        }
    }

    fn make_state_with_games(games: Vec<GameEntry>) -> ProfileViewState {
        ProfileViewState {
            phase: ProfileViewPhase::Loaded,
            games,
            search: String::new(),
            sort: LibrarySort::LastPlayed,
            capsule_size: CapsuleSize::default(),
            spinner_angle: 0.0,
            progress_scanner: None,
            progress_rx: Arc::new(Mutex::new(None)),
            scan_generation: 0,
            failed_app_ids: HashSet::new(),
            library_name_map: HashMap::new(),
            loader_pulse_phase: 0.0,
            loader_hiding_since: None,
            hovered_card: None,
            hovered_bar_slice: None,
            hovered_card_tier: None,
            status_filter: GameStatusFilter::default(),
            genre_filter: HashSet::new(),
            available_genres: Vec::new(),
            grid_scroll_y: 0.0,
            last_scan_completed_at: None,
            scan_started_at: None,
            scan_target_count: 0,
            derived: Default::default(),
        }
    }

    fn make_entry_with_capsule(capsule: CapsuleAsset, progress: Option<ProgressData>) -> GameEntry {
        GameEntry {
            app_id: 1,
            change_number: 0,
            last_played: None,
            name: Some("TestGame".to_owned()),
            capsule,
            progress,
            genre: None,
        }
    }

    #[test]
    fn is_hydrated_requires_progress_and_terminal_capsule() {
        let progress = Some(ProgressData {
            earned: 5,
            total: 10,
        });

        assert!(
            !make_entry_with_capsule(CapsuleAsset::Pending, None).is_hydrated(),
            "Pending + None -> not hydrated"
        );
        assert!(
            !make_entry_with_capsule(CapsuleAsset::Pending, progress).is_hydrated(),
            "Pending + Some(progress) -> not hydrated"
        );
        assert!(
            !make_entry_with_capsule(CapsuleAsset::Unavailable, None).is_hydrated(),
            "Unavailable + None -> not hydrated"
        );
        assert!(
            make_entry_with_capsule(CapsuleAsset::Unavailable, progress).is_hydrated(),
            "Unavailable + Some(progress) -> hydrated"
        );

        let dummy_handle = iced::widget::image::Handle::from_rgba(1, 1, vec![0u8, 0, 0, 255]);
        assert!(
            !make_entry_with_capsule(
                CapsuleAsset::Loaded {
                    handle: dummy_handle.clone(),
                    width: 1,
                    height: 1,
                },
                None
            )
            .is_hydrated(),
            "Loaded + None -> not hydrated"
        );
        assert!(
            make_entry_with_capsule(
                CapsuleAsset::Loaded {
                    handle: dummy_handle,
                    width: 1,
                    height: 1,
                },
                progress
            )
            .is_hydrated(),
            "Loaded + Some(progress) -> hydrated"
        );
    }

    #[test]
    fn sort_last_played_descending_with_none_at_end() {
        let mut state = make_state_with_games(vec![
            make_entry(1, "Alpha", None),
            make_entry(2, "Beta", Some(1000)),
            make_entry(3, "Gamma", Some(2000)),
            make_entry(4, "Delta", Some(500)),
        ]);
        state.sort = LibrarySort::LastPlayed;

        let visible = state.visible_games(&[]);
        let names: Vec<&str> = visible
            .iter()
            .map(|g| g.name.as_deref().unwrap_or(""))
            .collect();

        assert_eq!(names[0], "Gamma", "most recent first");
        assert_eq!(names[1], "Beta");
        assert_eq!(names[2], "Delta");
        assert_eq!(names[3], "Alpha", "never-played at end");
    }

    #[test]
    fn sort_name_ascending_case_insensitive() {
        let mut state = make_state_with_games(vec![
            make_entry(1, "cherry", None),
            make_entry(2, "Apple", None),
            make_entry(3, "banana", None),
        ]);
        state.sort = LibrarySort::NameAsc;

        let visible = state.visible_games(&[]);
        let names: Vec<&str> = visible
            .iter()
            .map(|g| g.name.as_deref().unwrap_or(""))
            .collect();

        assert_eq!(names[0], "Apple");
        assert_eq!(names[1], "banana");
        assert_eq!(names[2], "cherry");
    }

    fn make_entry_with_progress(app_id: u32, name: &str, earned: u32, total: u32) -> GameEntry {
        GameEntry {
            app_id,
            change_number: 0,
            last_played: None,
            name: Some(name.to_owned()),
            capsule: CapsuleAsset::Pending,
            progress: Some(ProgressData { earned, total }),
            genre: None,
        }
    }

    #[test]
    fn sort_completion_descending_highest_pct_first() {
        let mut state = make_state_with_games(vec![
            make_entry_with_progress(1, "Half", 50, 100),
            make_entry_with_progress(2, "Done", 99, 100),
            make_entry_with_progress(3, "Tiny", 1, 100),
            make_entry_with_progress(4, "Mid", 75, 100),
        ]);
        state.sort = LibrarySort::Completion;

        let visible = state.visible_games(&[]);
        let names: Vec<&str> = visible
            .iter()
            .map(|g| g.name.as_deref().unwrap_or(""))
            .collect();

        assert_eq!(names, vec!["Done", "Mid", "Half", "Tiny"]);
    }

    #[test]
    fn sort_completion_games_without_progress_sort_to_end() {
        let mut state = make_state_with_games(vec![
            make_entry_with_progress(1, "Real", 50, 100),
            make_entry(2, "NoData", None),
            make_entry_with_progress(3, "Hot", 90, 100),
        ]);
        state.sort = LibrarySort::Completion;

        let visible = state.visible_games(&[]);
        let names: Vec<&str> = visible
            .iter()
            .map(|g| g.name.as_deref().unwrap_or(""))
            .collect();

        assert_eq!(names, vec!["Hot", "Real", "NoData"]);
    }

    #[test]
    fn sort_completion_ties_break_by_name_ascending() {
        let mut state = make_state_with_games(vec![
            make_entry_with_progress(1, "Bravo", 50, 100),
            make_entry_with_progress(2, "Alpha", 50, 100),
            make_entry_with_progress(3, "charlie", 50, 100),
        ]);
        state.sort = LibrarySort::Completion;

        let visible = state.visible_games(&[]);
        let names: Vec<&str> = visible
            .iter()
            .map(|g| g.name.as_deref().unwrap_or(""))
            .collect();

        assert_eq!(names, vec!["Alpha", "Bravo", "charlie"]);
    }

    #[test]
    fn search_filters_case_insensitive_substring() {
        let mut state = make_state_with_games(vec![
            make_entry(1, "Terraria", Some(100)),
            make_entry(2, "Portal 2", Some(200)),
            make_entry(3, "terra Battle", Some(50)),
        ]);
        state.search = "terra".to_owned();

        let visible = state.visible_games(&[]);
        assert_eq!(visible.len(), 2);
        let names: Vec<&str> = visible
            .iter()
            .map(|g| g.name.as_deref().unwrap_or(""))
            .collect();
        assert!(names.contains(&"Terraria"));
        assert!(names.contains(&"terra Battle"));
    }

    #[test]
    fn loader_phase_beta_when_partial_progress() {
        let mut state = make_state_with_games(vec![
            GameEntry {
                app_id: 1,
                change_number: 0,
                last_played: None,
                name: Some("A".to_owned()),
                capsule: CapsuleAsset::Unavailable,
                progress: Some(crate::progress_scan::ProgressData {
                    earned: 5,
                    total: 10,
                }),
                genre: None,
            },
            GameEntry {
                app_id: 2,
                change_number: 0,
                last_played: None,
                name: Some("B".to_owned()),
                capsule: CapsuleAsset::Unavailable,
                progress: None,
                genre: None,
            },
        ]);
        state.phase = ProfileViewPhase::Loaded;
        assert_eq!(
            state.loader_phase(None),
            LoaderPhase::Beta {
                loaded: 1,
                total: 2
            }
        );
    }

    #[test]
    fn loader_phase_gamma_when_all_have_progress() {
        let mut state = make_state_with_games(vec![GameEntry {
            app_id: 1,
            change_number: 0,
            last_played: None,
            name: Some("A".to_owned()),
            capsule: CapsuleAsset::Unavailable,
            progress: Some(crate::progress_scan::ProgressData {
                earned: 5,
                total: 10,
            }),
            genre: None,
        }]);
        state.phase = ProfileViewPhase::Loaded;
        assert_eq!(state.loader_phase(None), LoaderPhase::Gamma);
    }

    #[test]
    fn pinned_first_sort_preserves_pin_order_regardless_of_active_sort() {
        let state = make_state_with_games(vec![
            make_entry(1, "Alpha", Some(3000)),
            make_entry(2, "Beta", Some(2000)),
            make_entry(3, "Gamma", Some(1000)),
            make_entry(4, "Delta", None),
        ]);
        let pinned = [3u32, 1u32];
        let visible = state.visible_games(&pinned);
        let ids: Vec<u32> = visible.iter().map(|g| g.app_id).collect();
        assert_eq!(ids[0], 3, "first pinned game must be first");
        assert_eq!(ids[1], 1, "second pinned game must be second");
        assert_eq!(ids[2], 2, "rest sorted by last_played descending");
        assert_eq!(ids[3], 4, "never-played last");
    }

    #[test]
    fn empty_pinned_list_falls_back_to_sort() {
        let state = make_state_with_games(vec![
            make_entry(1, "Zebra", Some(100)),
            make_entry(2, "Alpha", Some(200)),
        ]);
        let visible = state.visible_games(&[]);
        let ids: Vec<u32> = visible.iter().map(|g| g.app_id).collect();
        assert_eq!(ids[0], 2, "most recently played first");
        assert_eq!(ids[1], 1);
    }

    #[test]
    fn visible_games_filters_by_status_completed() {
        let mut state = make_state_with_games(vec![
            make_entry_with_progress(100, "Alpha", 10, 10),
            make_entry_with_progress(200, "Beta", 5, 10),
            make_entry_with_progress(300, "Gamma", 0, 10),
        ]);
        state.status_filter = GameStatusFilter::Completed;

        let visible = state.visible_games(&[]);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].app_id, 100, "only fully completed game returned");
    }

    #[test]
    fn visible_games_filters_by_status_not_started_treats_missing_progress_as_not_started() {
        let mut state = make_state_with_games(vec![
            make_entry_with_progress(100, "Started", 3, 10),
            make_entry_with_progress(200, "Zero", 0, 10),
            make_entry(300, "NoData", None),
        ]);
        state.status_filter = GameStatusFilter::NotStarted;

        let visible = state.visible_games(&[]);
        assert_eq!(
            visible.len(),
            2,
            "zero-progress and no-data games both qualify"
        );
        let ids: Vec<u32> = visible.iter().map(|g| g.app_id).collect();
        assert!(ids.contains(&200));
        assert!(ids.contains(&300));
    }

    #[test]
    fn visible_games_filters_by_genre_multi_select_intersection() {
        let state = make_state_with_games(vec![
            GameEntry {
                app_id: 100,
                change_number: 0,
                last_played: None,
                name: Some("Action Game".to_owned()),
                capsule: CapsuleAsset::Pending,
                progress: None,
                genre: Some("Action".to_owned()),
            },
            GameEntry {
                app_id: 200,
                change_number: 0,
                last_played: None,
                name: Some("RPG Game".to_owned()),
                capsule: CapsuleAsset::Pending,
                progress: None,
                genre: Some("RPG".to_owned()),
            },
            GameEntry {
                app_id: 300,
                change_number: 0,
                last_played: None,
                name: Some("Strategy Game".to_owned()),
                capsule: CapsuleAsset::Pending,
                progress: None,
                genre: Some("Strategy".to_owned()),
            },
        ]);
        let mut filtered = state;
        filtered.genre_filter.insert("Action".to_owned());
        filtered.genre_filter.insert("RPG".to_owned());

        let visible = filtered.visible_games(&[]);
        assert_eq!(visible.len(), 2, "only Action and RPG games returned");
        let ids: Vec<u32> = visible.iter().map(|g| g.app_id).collect();
        assert!(ids.contains(&100));
        assert!(ids.contains(&200));
        assert!(!ids.contains(&300));
    }

    #[test]
    fn loaded_games_sort_above_skeletons_regardless_of_user_sort() {
        let mut by_name = make_state_with_games(vec![
            GameEntry {
                app_id: 100,
                change_number: 0,
                last_played: None,
                name: Some("Zebra".to_owned()),
                capsule: CapsuleAsset::Pending,
                progress: Some(ProgressData {
                    earned: 1,
                    total: 10,
                }),
                genre: None,
            },
            GameEntry {
                app_id: 200,
                change_number: 0,
                last_played: None,
                name: Some("Apple".to_owned()),
                capsule: CapsuleAsset::Pending,
                progress: None,
                genre: None,
            },
            GameEntry {
                app_id: 300,
                change_number: 0,
                last_played: None,
                name: Some("Banana".to_owned()),
                capsule: CapsuleAsset::Pending,
                progress: Some(ProgressData {
                    earned: 5,
                    total: 10,
                }),
                genre: None,
            },
        ]);
        by_name.sort = LibrarySort::NameAsc;
        let visible = by_name.visible_games(&[]);
        let ids: Vec<u32> = visible.iter().map(|g| g.app_id).collect();
        assert_eq!(
            ids,
            vec![300, 100, 200],
            "loaded (Banana, Zebra) before skeleton (Apple), inside groups name-sorted"
        );
    }
}
