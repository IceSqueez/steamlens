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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameStatusFilter {
    #[default]
    All,
    Started,
    Completed,
    NotStarted,
}

#[derive(Clone)]
pub struct GameEntry {
    pub app_id: u32,
    pub change_number: u32,
    pub last_played: Option<u32>,
    pub name: Option<String>,
    pub capsule: CapsuleAsset,
    pub progress: Option<ProgressData>,
    pub genre: Option<String>,
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
    pub earned: u32,
    pub total: u32,
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
pub enum ProfileViewMessage {
    ScanComplete(Vec<steamlens_core::GameSummary>),
    ScanFailed {
        app_id: u32,
        reason: String,
    },
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
    RescanRequested,
    RetrySingleFailedScan(u32),
    StatusFilterChanged(GameStatusFilter),
    GenreFilterToggled(String),
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
    CardTierHovered {
        app_id: u32,
        tier: Option<RarityTier>,
    },
    RequestToggleGamePin(u32),
    RequestOpenGame(u32),
    DrainProgressResults,
}

#[derive(Debug, Clone)]
pub enum ProfileEvent {
    None,
    OpenGame(u32),
    ToggleGamePin(u32),
    RequestRescan,
    DrainedProgress {
        cache_entries: Vec<crate::cache::GameCacheEntry>,
        no_ach_entries: Vec<(u32, u32)>,
    },
}

impl std::fmt::Debug for ProfileViewMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileViewMessage::ScanComplete(v) => {
                write!(f, "ScanComplete({} enumerated)", v.len())
            }
            ProfileViewMessage::ScanFailed { app_id, reason } => {
                write!(f, "ScanFailed({{ app_id: {app_id}, reason: {reason:?} }})")
            }
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
            ProfileViewMessage::CardTierHovered { app_id, tier } => {
                write!(f, "CardTierHovered(app={app_id}, tier={tier:?})")
            }
            ProfileViewMessage::RequestToggleGamePin(id) => write!(f, "RequestToggleGamePin({id})"),
            ProfileViewMessage::RequestOpenGame(id) => write!(f, "RequestOpenGame({id})"),
            ProfileViewMessage::RetrySingleFailedScan(id) => {
                write!(f, "RetrySingleFailedScan({id})")
            }
            ProfileViewMessage::DrainProgressResults => write!(f, "DrainProgressResults"),
            ProfileViewMessage::StatusFilterChanged(f2) => write!(f, "StatusFilterChanged({f2:?})"),
            ProfileViewMessage::GenreFilterToggled(g) => write!(f, "GenreFilterToggled({g:?})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProfileViewPhase {
    Scanning,
    Loaded,
}

pub struct ProfileViewState {
    pub phase: ProfileViewPhase,
    pub games: Vec<GameEntry>,
    pub capsule_handles: HashMap<(u32, CapsuleSize), StoredCapsule>,
    pub search: String,
    pub sort: LibrarySort,
    pub capsule_size: CapsuleSize,
    pub spinner_angle: f32,
    pub progress_scanner: Option<crate::progress_scan::ProgressScanner>,
    pub progress_rx:
        Option<tokio::sync::mpsc::UnboundedReceiver<crate::progress_scan::ProgressResult>>,
    pub failed_app_ids: HashSet<u32>,
    pub library_name_map: HashMap<u32, String>,
    pub loader_pulse_phase: f32,
    pub loader_hiding_since: Option<Instant>,
    pub hovered_card: Option<u32>,
    pub hovered_bar_slice: Option<RarityTier>,
    pub hovered_card_tier: Option<(u32, RarityTier)>,
    pub status_filter: GameStatusFilter,
    pub genre_filter: HashSet<String>,
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
            spinner_angle: 0.0,
            progress_scanner: None,
            progress_rx: None,
            failed_app_ids: HashSet::new(),
            library_name_map: HashMap::new(),
            loader_pulse_phase: 0.0,
            loader_hiding_since: None,
            hovered_card: None,
            hovered_bar_slice: None,
            hovered_card_tier: None,
            status_filter: GameStatusFilter::default(),
            genre_filter: HashSet::new(),
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
        let status = self.status_filter;
        let genre_filter = &self.genre_filter;

        let mut result: Vec<&GameEntry> = self
            .games
            .iter()
            .filter(|g| {
                if !query.is_empty()
                    && !g
                        .name
                        .as_deref()
                        .map(|n| n.to_lowercase().contains(&query))
                        .unwrap_or(false)
                {
                    return false;
                }

                let status_ok = match status {
                    GameStatusFilter::All => true,
                    GameStatusFilter::Started => g
                        .progress
                        .as_ref()
                        .map(|p| p.earned > 0 && p.earned < p.total)
                        .unwrap_or(false),
                    GameStatusFilter::Completed => g
                        .progress
                        .as_ref()
                        .map(|p| p.total > 0 && p.earned == p.total)
                        .unwrap_or(false),
                    GameStatusFilter::NotStarted => {
                        g.progress.as_ref().map(|p| p.earned == 0).unwrap_or(true)
                    }
                };
                if !status_ok {
                    return false;
                }

                if !genre_filter.is_empty() {
                    return g
                        .genre
                        .as_deref()
                        .map(|genre_str| genre_filter.contains(genre_str))
                        .unwrap_or(false);
                }

                true
            })
            .collect();

        sort_entries(&mut result, self.sort, pinned);
        result
    }

    pub fn available_genres(&self) -> Vec<String> {
        let set: std::collections::BTreeSet<String> =
            self.games.iter().filter_map(|g| g.genre.clone()).collect();
        set.into_iter().collect()
    }

    pub fn loader_phase(&self, steam_running: Option<bool>) -> LoaderPhase {
        if self.games.is_empty() {
            if steam_running == Some(false) {
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

    pub fn loader_needs_pulse_subscription(&self, steam_running: Option<bool>) -> bool {
        match self.loader_phase(steam_running) {
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

fn is_skeleton(entry: &GameEntry) -> bool {
    entry.progress.is_none()
}

fn sort_by_mode(entries: &mut Vec<&GameEntry>, sort: LibrarySort) {
    match sort {
        LibrarySort::LastPlayed => {
            entries.sort_by(|a, b| {
                is_skeleton(a).cmp(&is_skeleton(b)).then_with(|| {
                    match (a.last_played, b.last_played) {
                        (Some(ta), Some(tb)) => tb.cmp(&ta),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => entry_name(a)
                            .to_lowercase()
                            .cmp(&entry_name(b).to_lowercase()),
                    }
                })
            });
        }
        LibrarySort::NameAsc => {
            entries.sort_by(|a, b| {
                is_skeleton(a).cmp(&is_skeleton(b)).then_with(|| {
                    entry_name(a)
                        .to_lowercase()
                        .cmp(&entry_name(b).to_lowercase())
                })
            });
        }
        LibrarySort::Completion => {
            entries.sort_by(|a, b| {
                is_skeleton(a).cmp(&is_skeleton(b)).then_with(|| {
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
            genre: None,
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
            spinner_angle: 0.0,
            progress_scanner: None,
            progress_rx: None,
            failed_app_ids: HashSet::new(),
            library_name_map: HashMap::new(),
            loader_pulse_phase: 0.0,
            loader_hiding_since: None,
            hovered_card: None,
            hovered_bar_slice: None,
            hovered_card_tier: None,
            status_filter: GameStatusFilter::default(),
            genre_filter: HashSet::new(),
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
        assert_eq!(state.loader_phase(None), LoaderPhase::Alpha);
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
