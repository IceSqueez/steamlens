use std::collections::{HashMap, HashSet};
use std::time::Instant;

use iced::widget::image::Handle as ImageHandle;

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
    pub app_id: u32,
    pub change_number: u32,
    pub last_played: Option<u32>,
    pub name: Option<String>,
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
            .field("app_id", &self.app_id)
            .field("name", &self.name)
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
    Completion,
}

impl LibrarySort {
    pub fn short_label(&self) -> &'static str {
        match self {
            LibrarySort::NameAsc => "A–Z",
            LibrarySort::LastPlayed => "LP",
            LibrarySort::Completion => "C",
        }
    }

    pub fn tooltip(&self) -> &'static str {
        match self {
            LibrarySort::NameAsc => "Sort by name (A → Z)",
            LibrarySort::LastPlayed => "Sort by last played",
            LibrarySort::Completion => "Sort by completion % (highest first)",
        }
    }
}

impl std::fmt::Display for LibrarySort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibrarySort::LastPlayed => write!(f, "Last Played"),
            LibrarySort::NameAsc => write!(f, "Name (A\u{2013}Z)"),
            LibrarySort::Completion => write!(f, "Completion"),
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub enum ProfileViewMessage {
    ScanComplete(Vec<steamlens_core::GameSummary>),
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
    RetryFailedScans,
    BarSliceHoverEnter(RarityTier),
    BarSliceHoverExit,
}

impl std::fmt::Debug for ProfileViewMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileViewMessage::ScanComplete(v) => {
                write!(f, "ScanComplete({} enumerated)", v.len())
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
            ProfileViewMessage::RetryFailedScans => write!(f, "RetryFailedScans"),
            ProfileViewMessage::BarSliceHoverEnter(t) => write!(f, "BarSliceHoverEnter({t:?})"),
            ProfileViewMessage::BarSliceHoverExit => write!(f, "BarSliceHoverExit"),
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
    pub failed_app_ids: HashSet<u32>,
    pub steam_running: Option<bool>,
    pub loader_pulse_phase: f32,
    pub loader_hiding_since: Option<Instant>,
    pub hovered_card: Option<u32>,
    pub hovered_bar_slice: Option<RarityTier>,
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
            failed_app_ids: HashSet::new(),
            steam_running: None,
            loader_pulse_phase: 0.0,
            loader_hiding_since: None,
            hovered_card: None,
            hovered_bar_slice: None,
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
            .filter(|g| {
                query.is_empty()
                    || g.name
                        .as_deref()
                        .map(|n| n.to_lowercase().contains(&query))
                        .unwrap_or(false)
            })
            .collect();

        sort_entries(&mut result, self.sort, pinned);
        result
    }

    pub fn loader_phase(&self) -> LoaderPhase {
        if self.games.is_empty() {
            if self.steam_running == Some(false) {
                return LoaderPhase::SteamOff;
            }
            return LoaderPhase::Alpha;
        }
        let total = self.games.len();
        let with_progress = self.games.iter().filter(|g| g.progress.is_some()).count();
        let failed = self.failed_app_ids.len();
        let pending = total.saturating_sub(with_progress).saturating_sub(failed);
        if pending > 0 {
            LoaderPhase::Beta {
                loaded: with_progress,
                total,
            }
        } else if failed > 0 {
            LoaderPhase::Failed { failed, total }
        } else {
            LoaderPhase::Gamma
        }
    }

    pub fn loader_needs_pulse_subscription(&self) -> bool {
        match self.loader_phase() {
            LoaderPhase::Alpha | LoaderPhase::Beta { .. } => true,
            LoaderPhase::Failed { .. } | LoaderPhase::SteamOff => false,
            LoaderPhase::Gamma => self
                .loader_hiding_since
                .map(|t| t.elapsed().as_millis() < 300)
                .unwrap_or(true),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoaderPhase {
    Alpha,
    Beta { loaded: usize, total: usize },
    Gamma,
    Failed { failed: usize, total: usize },
    SteamOff,
}

fn sort_entries(entries: &mut Vec<&GameEntry>, sort: LibrarySort, pinned: &[u32]) {
    if pinned.is_empty() {
        sort_by_mode(entries, sort);
        return;
    }

    let (mut pinned_entries, mut rest): (Vec<&GameEntry>, Vec<&GameEntry>) =
        entries.iter().partition(|g| pinned.contains(&g.app_id));

    pinned_entries.sort_by_key(|g| {
        pinned
            .iter()
            .position(|&pid| pid == g.app_id)
            .unwrap_or(usize::MAX)
    });

    sort_by_mode(&mut rest, sort);

    *entries = pinned_entries.into_iter().chain(rest).collect();
}

fn entry_name(entry: &GameEntry) -> &str {
    entry.name.as_deref().unwrap_or("")
}

fn sort_by_mode(entries: &mut Vec<&GameEntry>, sort: LibrarySort) {
    match sort {
        LibrarySort::LastPlayed => {
            entries.sort_by(|a, b| match (a.last_played, b.last_played) {
                (Some(ta), Some(tb)) => tb.cmp(&ta),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => entry_name(a)
                    .to_lowercase()
                    .cmp(&entry_name(b).to_lowercase()),
            });
        }
        LibrarySort::NameAsc => {
            entries.sort_by(|a, b| {
                entry_name(a)
                    .to_lowercase()
                    .cmp(&entry_name(b).to_lowercase())
            });
        }
        LibrarySort::Completion => {
            entries.sort_by(|a, b| {
                let pct_b = completion_pct(b);
                let pct_a = completion_pct(a);
                pct_b
                    .partial_cmp(&pct_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        entry_name(a)
                            .to_lowercase()
                            .cmp(&entry_name(b).to_lowercase())
                    })
            });
        }
    }
}

fn completion_pct(entry: &GameEntry) -> f32 {
    match entry.progress {
        Some(p) if p.total > 0 => p.earned as f32 / p.total as f32,
        _ => -1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(app_id: u32, name: &str, last_played: Option<u32>) -> GameEntry {
        GameEntry {
            app_id,
            change_number: 0,
            last_played,
            name: Some(name.to_owned()),
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
            failed_app_ids: HashSet::new(),
            steam_running: None,
            loader_pulse_phase: 0.0,
            loader_hiding_since: None,
            hovered_card: None,
            hovered_bar_slice: None,
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
    fn library_sort_short_label_and_tooltip() {
        assert_eq!(LibrarySort::NameAsc.short_label(), "A\u{2013}Z");
        assert_eq!(LibrarySort::LastPlayed.short_label(), "LP");
        assert_eq!(LibrarySort::Completion.short_label(), "C");
        assert!(!LibrarySort::Completion.tooltip().is_empty());
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

        let entry = make_entry(app_id, "Terraria", None);
        let mut state = make_state_with_games(vec![entry]);
        state.capsule_size = CapsuleSize::Small;
        state
            .capsule_handles
            .insert((app_id, CapsuleSize::Small), stored);

        for entry in &mut state.games {
            let key = (entry.app_id, state.capsule_size);
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
        let entry = make_entry(app_id, "Terraria", None);
        let mut state = make_state_with_games(vec![entry]);
        state.capsule_size = CapsuleSize::Small;

        for entry in &mut state.games {
            let key = (entry.app_id, state.capsule_size);
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
                app_id: 1,
                change_number: 0,
                last_played: None,
                name: Some("A".to_owned()),
                capsule: CapsuleAsset::Unavailable,
                progress: Some(crate::progress_scan::ProgressData {
                    earned: 5,
                    total: 10,
                }),
            },
            GameEntry {
                app_id: 2,
                change_number: 0,
                last_played: None,
                name: Some("B".to_owned()),
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
            app_id: 1,
            change_number: 0,
            last_played: None,
            name: Some("A".to_owned()),
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
        let ids: Vec<u32> = visible.iter().map(|g| g.app_id).collect();
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
        let ids: Vec<u32> = visible.iter().map(|g| g.app_id).collect();
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
        let ids: Vec<u32> = visible.iter().map(|g| g.app_id).collect();
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
