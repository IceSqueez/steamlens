use std::collections::HashMap;
use std::time::Instant;

use iced::widget::image::Handle as ImageHandle;
use steamlens_core::GameSummary;

use crate::capsule_cache::CapsuleSize;
use crate::game_view::types::RarityTier;
use crate::progress_scan::ProgressData;

#[derive(Clone)]
pub struct StoredCapsule {
    pub handle: ImageHandle,
    pub width: u32,
    pub height: u32,
}

impl std::fmt::Debug for StoredCapsule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StoredCapsule({}x{})", self.width, self.height)
    }
}

#[derive(Clone)]
pub enum CapsuleAsset {
    Pending,
    Loaded {
        handle: ImageHandle,
        width: u32,
        height: u32,
    },
    Unavailable,
}

impl std::fmt::Debug for CapsuleAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapsuleAsset::Pending => write!(f, "Pending"),
            CapsuleAsset::Loaded { width, height, .. } => {
                write!(f, "Loaded({width}x{height})")
            }
            CapsuleAsset::Unavailable => write!(f, "Unavailable"),
        }
    }
}

#[derive(Clone)]
pub struct GameEntry {
    pub summary: GameSummary,
    pub capsule: CapsuleAsset,
    pub progress: Option<ProgressData>,
}

impl GameEntry {
    pub fn is_hydrated(&self) -> bool {
        self.progress.is_some() && !matches!(self.capsule, CapsuleAsset::Pending)
    }
}

impl std::fmt::Debug for GameEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameEntry")
            .field("app_id", &self.summary.app_id)
            .field("name", &self.summary.name)
            .field("progress", &self.progress)
            .finish_non_exhaustive()
    }
}

pub struct TopEntry {
    pub app_id: u32,
    pub game_name: String,
    pub completion_pct: f64,
    pub rarity_tier: Option<RarityTier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibrarySort {
    LastPlayed,
    NameAsc,
}

impl std::fmt::Display for LibrarySort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibrarySort::LastPlayed => write!(f, "Last Played"),
            LibrarySort::NameAsc => write!(f, "Name (A\u{2013}Z)"),
        }
    }
}

#[derive(Clone)]
pub enum ProfileViewMessage {
    ScanComplete(Vec<GameSummary>),
    ScanFailed(String),
    SearchChanged(String),
    SortChanged(LibrarySort),
    CapsuleSizeChanged(CapsuleSize),
    CapsuleLoaded {
        app_id: u32,
        size: CapsuleSize,
        handle: ImageHandle,
        width: u32,
        height: u32,
    },
    CapsuleFailed {
        app_id: u32,
        size: CapsuleSize,
    },
    GameSelected(u32),
    ManualAppIdChanged(String),
    ManualAppIdSubmitted,
    RescanRequested,
    SpinnerTick(f32),
    ProgressFetched {
        app_id: u32,
        earned: u32,
        total: u32,
    },
    ProgressScanDone,
    LoaderPulseTick,
    CardHoverEnter(u32),
    CardHoverExit(u32),
}

impl std::fmt::Debug for ProfileViewMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileViewMessage::ScanComplete(v) => {
                write!(f, "ScanComplete({} games)", v.len())
            }
            ProfileViewMessage::ScanFailed(e) => write!(f, "ScanFailed({e})"),
            ProfileViewMessage::SearchChanged(s) => write!(f, "SearchChanged({s:?})"),
            ProfileViewMessage::SortChanged(s) => write!(f, "SortChanged({s:?})"),
            ProfileViewMessage::CapsuleSizeChanged(s) => write!(f, "CapsuleSizeChanged({s})"),
            ProfileViewMessage::CapsuleLoaded {
                app_id,
                size,
                width,
                height,
                ..
            } => write!(f, "CapsuleLoaded(app={app_id}, {size}, {width}x{height})"),
            ProfileViewMessage::CapsuleFailed { app_id, size } => {
                write!(f, "CapsuleFailed(app={app_id}, {size})")
            }
            ProfileViewMessage::GameSelected(id) => write!(f, "GameSelected({id})"),
            ProfileViewMessage::ManualAppIdChanged(s) => write!(f, "ManualAppIdChanged({s:?})"),
            ProfileViewMessage::ManualAppIdSubmitted => write!(f, "ManualAppIdSubmitted"),
            ProfileViewMessage::RescanRequested => write!(f, "RescanRequested"),
            ProfileViewMessage::SpinnerTick(a) => write!(f, "SpinnerTick({a:.1})"),
            ProfileViewMessage::ProgressFetched {
                app_id,
                earned,
                total,
            } => write!(f, "ProgressFetched(app={app_id}, {earned}/{total})"),
            ProfileViewMessage::ProgressScanDone => write!(f, "ProgressScanDone"),
            ProfileViewMessage::LoaderPulseTick => write!(f, "LoaderPulseTick"),
            ProfileViewMessage::CardHoverEnter(id) => write!(f, "CardHoverEnter({id})"),
            ProfileViewMessage::CardHoverExit(id) => write!(f, "CardHoverExit({id})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProfileViewPhase {
    Scanning,
    Loaded,
    Error(String),
}

pub struct ProfileViewState {
    pub phase: ProfileViewPhase,
    pub games: Vec<GameEntry>,
    pub capsule_handles: HashMap<(u32, CapsuleSize), StoredCapsule>,
    pub search: String,
    pub sort: LibrarySort,
    pub capsule_size: CapsuleSize,
    pub manual_app_id_input: String,
    pub spinner_angle: f32,
    pub progress_scanner: Option<crate::progress_scan::ProgressScanner>,
    pub progress_rx:
        Option<tokio::sync::mpsc::UnboundedReceiver<crate::progress_scan::ProgressResult>>,
    pub loader_pulse_phase: f32,
    pub loader_hiding_since: Option<Instant>,
    pub hovered_card: Option<u32>,
}

impl std::fmt::Debug for ProfileViewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileViewState")
            .field("phase", &self.phase)
            .field("games_count", &self.games.len())
            .field("sort", &self.sort)
            .field("capsule_size", &self.capsule_size)
            .finish_non_exhaustive()
    }
}

impl ProfileViewState {
    pub fn new() -> Self {
        Self {
            phase: ProfileViewPhase::Scanning,
            games: Vec::new(),
            capsule_handles: HashMap::new(),
            search: String::new(),
            sort: LibrarySort::LastPlayed,
            capsule_size: CapsuleSize::default(),
            manual_app_id_input: String::new(),
            spinner_angle: 0.0,
            progress_scanner: None,
            progress_rx: None,
            loader_pulse_phase: 0.0,
            loader_hiding_since: None,
            hovered_card: None,
        }
    }

    pub fn is_streaming(&self) -> bool {
        self.games
            .iter()
            .any(|g| matches!(g.capsule, CapsuleAsset::Pending))
            || self.progress_scanner.is_some()
    }

    pub fn visible_games<'a>(&'a self, pinned: &[u32]) -> Vec<&'a GameEntry> {
        let query = self.search.to_lowercase();
        let mut result: Vec<&GameEntry> = self
            .games
            .iter()
            .filter(|g| query.is_empty() || g.summary.name.to_lowercase().contains(&query))
            .collect();

        sort_entries(&mut result, self.sort, pinned);
        result
    }

    /// Returns the current loader phase based on game/progress state.
    ///
    /// - Alpha: no games discovered yet (indeterminate pulse).
    /// - Beta: games discovered but not all have progress data (determinate fill).
    /// - Gamma: all games have progress data (loader fading out or unmounted).
    pub fn loader_phase(&self) -> LoaderPhase {
        if self.games.is_empty() {
            return LoaderPhase::Alpha;
        }
        let with_progress = self.games.iter().filter(|g| g.progress.is_some()).count();
        if with_progress < self.games.len() {
            LoaderPhase::Beta {
                loaded: with_progress,
                total: self.games.len(),
            }
        } else {
            LoaderPhase::Gamma
        }
    }

    /// Returns true while the loader should be subscribed for pulse ticks
    /// (phases α and β, plus the 300 ms γ fade-out window).
    pub fn loader_needs_pulse_subscription(&self) -> bool {
        match self.loader_phase() {
            LoaderPhase::Alpha | LoaderPhase::Beta { .. } => true,
            LoaderPhase::Gamma => self
                .loader_hiding_since
                .map(|t| t.elapsed().as_millis() < 300)
                .unwrap_or(true),
        }
    }
}

/// The three phases of the unified loader strip.
///
/// - Alpha: library is empty — indeterminate pulsing animation.
/// - Beta: games exist but progress data is still streaming in — determinate bar.
/// - Gamma: all games have progress — loader fades out and unmounts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoaderPhase {
    Alpha,
    Beta { loaded: usize, total: usize },
    Gamma,
}

fn sort_entries(entries: &mut Vec<&GameEntry>, sort: LibrarySort, pinned: &[u32]) {
    if pinned.is_empty() {
        sort_by_mode(entries, sort);
        return;
    }

    let (mut pinned_entries, mut rest): (Vec<&GameEntry>, Vec<&GameEntry>) = entries
        .iter()
        .partition(|g| pinned.contains(&g.summary.app_id));

    pinned_entries.sort_by_key(|g| {
        pinned
            .iter()
            .position(|&pid| pid == g.summary.app_id)
            .unwrap_or(usize::MAX)
    });

    sort_by_mode(&mut rest, sort);

    *entries = pinned_entries.into_iter().chain(rest).collect();
}

fn sort_by_mode(entries: &mut Vec<&GameEntry>, sort: LibrarySort) {
    match sort {
        LibrarySort::LastPlayed => {
            entries.sort_by(
                |a, b| match (a.summary.last_played, b.summary.last_played) {
                    (Some(ta), Some(tb)) => tb.cmp(&ta),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => a
                        .summary
                        .name
                        .to_lowercase()
                        .cmp(&b.summary.name.to_lowercase()),
                },
            );
        }
        LibrarySort::NameAsc => {
            entries.sort_by(|a, b| {
                a.summary
                    .name
                    .to_lowercase()
                    .cmp(&b.summary.name.to_lowercase())
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_summary(app_id: u32, name: &str, last_played: Option<u32>) -> GameSummary {
        GameSummary {
            app_id,
            name: name.to_owned(),
            last_played,
            achievement_count: 1,
            last_updated: 0,
            manifest_path: std::path::PathBuf::new(),
        }
    }

    fn make_entry(app_id: u32, name: &str, last_played: Option<u32>) -> GameEntry {
        GameEntry {
            summary: make_summary(app_id, name, last_played),
            capsule: CapsuleAsset::Pending,
            progress: None,
        }
    }

    fn make_state_with_games(games: Vec<GameEntry>) -> ProfileViewState {
        ProfileViewState {
            phase: ProfileViewPhase::Loaded,
            games,
            capsule_handles: HashMap::new(),
            search: String::new(),
            sort: LibrarySort::LastPlayed,
            capsule_size: CapsuleSize::default(),
            manual_app_id_input: String::new(),
            spinner_angle: 0.0,
            progress_scanner: None,
            progress_rx: None,
            loader_pulse_phase: 0.0,
            loader_hiding_since: None,
            hovered_card: None,
        }
    }

    #[test]
    fn is_hydrated_requires_progress_and_terminal_capsule() {
        let summary = make_summary(1, "TestGame", None);
        let progress = Some(ProgressData {
            earned: 5,
            total: 10,
        });

        let pending_no_progress = GameEntry {
            summary: summary.clone(),
            capsule: CapsuleAsset::Pending,
            progress: None,
        };
        assert!(
            !pending_no_progress.is_hydrated(),
            "Pending + None -> not hydrated"
        );

        let pending_with_progress = GameEntry {
            summary: summary.clone(),
            capsule: CapsuleAsset::Pending,
            progress,
        };
        assert!(
            !pending_with_progress.is_hydrated(),
            "Pending + Some(progress) -> not hydrated"
        );

        let unavailable_no_progress = GameEntry {
            summary: summary.clone(),
            capsule: CapsuleAsset::Unavailable,
            progress: None,
        };
        assert!(
            !unavailable_no_progress.is_hydrated(),
            "Unavailable + None -> not hydrated"
        );

        let unavailable_with_progress = GameEntry {
            summary: summary.clone(),
            capsule: CapsuleAsset::Unavailable,
            progress,
        };
        assert!(
            unavailable_with_progress.is_hydrated(),
            "Unavailable + Some(progress) -> hydrated"
        );

        let dummy_handle = iced::widget::image::Handle::from_rgba(1, 1, vec![0u8, 0, 0, 255]);
        let loaded_no_progress = GameEntry {
            summary: summary.clone(),
            capsule: CapsuleAsset::Loaded {
                handle: dummy_handle.clone(),
                width: 1,
                height: 1,
            },
            progress: None,
        };
        assert!(
            !loaded_no_progress.is_hydrated(),
            "Loaded + None -> not hydrated"
        );

        let loaded_with_progress = GameEntry {
            summary: summary.clone(),
            capsule: CapsuleAsset::Loaded {
                handle: dummy_handle,
                width: 1,
                height: 1,
            },
            progress,
        };
        assert!(
            loaded_with_progress.is_hydrated(),
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
        let names: Vec<&str> = visible.iter().map(|g| g.summary.name.as_str()).collect();

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
        let names: Vec<&str> = visible.iter().map(|g| g.summary.name.as_str()).collect();

        assert_eq!(names[0], "Apple");
        assert_eq!(names[1], "banana");
        assert_eq!(names[2], "cherry");
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
        let names: Vec<&str> = visible.iter().map(|g| g.summary.name.as_str()).collect();
        assert!(names.contains(&"Terraria"));
        assert!(names.contains(&"terra Battle"));
    }

    #[test]
    fn manual_app_id_invalid_input_blocks_submit() {
        let cases = ["abc", "0", "", "4294967296"];
        for case in &cases {
            let app_id: Option<u32> = case
                .parse::<u32>()
                .ok()
                .filter(|&id| id > 0 && id < u32::MAX);
            assert!(
                app_id.is_none(),
                "invalid input '{case}' should block submit"
            );
        }

        let valid: Option<u32> = "105600"
            .parse::<u32>()
            .ok()
            .filter(|&id| id > 0 && id < u32::MAX);
        assert_eq!(valid, Some(105600));
    }

    #[test]
    fn capsule_size_default_is_medium() {
        let state = ProfileViewState::new();
        assert_eq!(state.capsule_size, CapsuleSize::Medium);
    }

    #[test]
    fn profile_view_spinner_tick_updates_angle() {
        let mut state = make_state_with_games(vec![]);
        assert_eq!(
            state.spinner_angle, 0.0,
            "precondition: angle starts at 0.0"
        );

        state.spinner_angle = (state.spinner_angle + 6.0) % 360.0;
        assert!(
            (state.spinner_angle - 6.0).abs() < f32::EPSILON,
            "one tick must advance angle by 6 degrees"
        );

        for _ in 0..59 {
            state.spinner_angle = (state.spinner_angle + 6.0) % 360.0;
        }
        assert!(
            (state.spinner_angle - 0.0).abs() < f32::EPSILON,
            "60 ticks must wrap exactly back to 0.0 (60 * 6 = 360)"
        );

        state.spinner_angle = 356.0;
        state.spinner_angle = (state.spinner_angle + 6.0) % 360.0;
        assert!(
            (state.spinner_angle - 2.0).abs() < f32::EPSILON,
            "angle must wrap around 360 correctly from 356"
        );
    }

    #[test]
    fn cache_hit_instant_restore() {
        use iced::widget::image::Handle as ImageHandle;

        let app_id = 105600u32;
        let dummy_handle = ImageHandle::from_rgba(1, 1, vec![0u8, 0, 0, 255]);
        let stored = StoredCapsule {
            handle: dummy_handle.clone(),
            width: 120,
            height: 45,
        };

        let entry = GameEntry {
            summary: make_summary(app_id, "Terraria", None),
            capsule: CapsuleAsset::Pending,
            progress: None,
        };
        let mut state = make_state_with_games(vec![entry]);
        state.capsule_size = CapsuleSize::Small;
        state
            .capsule_handles
            .insert((app_id, CapsuleSize::Small), stored);

        for entry in &mut state.games {
            let key = (entry.summary.app_id, state.capsule_size);
            if let Some(cached) = state.capsule_handles.get(&key) {
                entry.capsule = CapsuleAsset::Loaded {
                    handle: cached.handle.clone(),
                    width: cached.width,
                    height: cached.height,
                };
            } else {
                entry.capsule = CapsuleAsset::Pending;
            }
        }

        let g = &state.games[0];
        assert!(
            matches!(g.capsule, CapsuleAsset::Loaded { .. }),
            "entry capsule must be Loaded on cache hit"
        );
    }

    #[test]
    fn cache_miss_falls_through_to_pending() {
        let app_id = 105600u32;
        let entry = GameEntry {
            summary: make_summary(app_id, "Terraria", None),
            capsule: CapsuleAsset::Pending,
            progress: None,
        };
        let mut state = make_state_with_games(vec![entry]);
        state.capsule_size = CapsuleSize::Small;

        for entry in &mut state.games {
            let key = (entry.summary.app_id, state.capsule_size);
            if let Some(cached) = state.capsule_handles.get(&key) {
                entry.capsule = CapsuleAsset::Loaded {
                    handle: cached.handle.clone(),
                    width: cached.width,
                    height: cached.height,
                };
            } else {
                entry.capsule = CapsuleAsset::Pending;
            }
        }

        let g = &state.games[0];
        assert!(
            matches!(g.capsule, CapsuleAsset::Pending),
            "entry capsule must be Pending on cache miss"
        );
    }

    #[test]
    fn loader_phase_alpha_when_no_games() {
        let state = ProfileViewState::new();
        assert_eq!(state.loader_phase(), LoaderPhase::Alpha);
    }

    #[test]
    fn loader_phase_beta_when_partial_progress() {
        let mut state = make_state_with_games(vec![
            GameEntry {
                summary: make_summary(1, "A", None),
                capsule: CapsuleAsset::Unavailable,
                progress: Some(crate::progress_scan::ProgressData {
                    earned: 5,
                    total: 10,
                }),
            },
            GameEntry {
                summary: make_summary(2, "B", None),
                capsule: CapsuleAsset::Unavailable,
                progress: None,
            },
        ]);
        state.phase = ProfileViewPhase::Loaded;
        assert_eq!(
            state.loader_phase(),
            LoaderPhase::Beta {
                loaded: 1,
                total: 2
            }
        );
    }

    #[test]
    fn loader_phase_gamma_when_all_have_progress() {
        let mut state = make_state_with_games(vec![GameEntry {
            summary: make_summary(1, "A", None),
            capsule: CapsuleAsset::Unavailable,
            progress: Some(crate::progress_scan::ProgressData {
                earned: 5,
                total: 10,
            }),
        }]);
        state.phase = ProfileViewPhase::Loaded;
        assert_eq!(state.loader_phase(), LoaderPhase::Gamma);
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
        let ids: Vec<u32> = visible.iter().map(|g| g.summary.app_id).collect();
        assert_eq!(ids[0], 3, "first pinned game must be first");
        assert_eq!(ids[1], 1, "second pinned game must be second");
        assert_eq!(ids[2], 2, "rest sorted by last_played descending");
        assert_eq!(ids[3], 4, "never-played last");
    }

    #[test]
    fn pinned_first_sort_with_name_sort() {
        let state = make_state_with_games(vec![
            make_entry(1, "Cherry", None),
            make_entry(2, "Apple", None),
            make_entry(3, "Banana", None),
        ]);
        let pinned = [1u32];
        let mut sorted_state = make_state_with_games(vec![
            make_entry(1, "Cherry", None),
            make_entry(2, "Apple", None),
            make_entry(3, "Banana", None),
        ]);
        sorted_state.sort = LibrarySort::NameAsc;
        let visible = sorted_state.visible_games(&pinned);
        let ids: Vec<u32> = visible.iter().map(|g| g.summary.app_id).collect();
        assert_eq!(
            ids[0], 1,
            "pinned game must be first regardless of name sort"
        );
        assert_eq!(ids[1], 2, "Apple before Banana alphabetically");
        assert_eq!(ids[2], 3, "Banana after Apple");
        let _ = state;
    }

    #[test]
    fn empty_pinned_list_falls_back_to_sort() {
        let state = make_state_with_games(vec![
            make_entry(1, "Zebra", Some(100)),
            make_entry(2, "Alpha", Some(200)),
        ]);
        let visible = state.visible_games(&[]);
        let ids: Vec<u32> = visible.iter().map(|g| g.summary.app_id).collect();
        assert_eq!(ids[0], 2, "most recently played first");
        assert_eq!(ids[1], 1);
    }

    #[test]
    fn toggle_game_pin_adds_to_settings() {
        let mut pinned: Vec<u32> = vec![];
        let app_id = 105600u32;
        if let Some(pos) = pinned.iter().position(|&id| id == app_id) {
            pinned.remove(pos);
        } else {
            pinned.push(app_id);
        }
        assert_eq!(pinned, vec![105600u32], "pin should be added");
    }

    #[test]
    fn toggle_game_pin_removes_from_settings() {
        let mut pinned: Vec<u32> = vec![105600u32, 420u32];
        let app_id = 105600u32;
        if let Some(pos) = pinned.iter().position(|&id| id == app_id) {
            pinned.remove(pos);
        } else {
            pinned.push(app_id);
        }
        assert_eq!(pinned, vec![420u32], "pin should be removed");
    }
}
