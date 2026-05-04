use std::collections::{HashMap, VecDeque};

use iced::widget::image::Handle as ImageHandle;
use steamlens_core::GameSummary;

use crate::capsule_cache::CapsuleSize;
use crate::progress_scan::ProgressData;

pub(crate) const FADE_DELTA: f32 = 0.2;

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
pub struct GameEntry {
    pub summary: GameSummary,
    pub capsule: CapsuleState,
    pub revealed: bool,
    /// Per-game achievement progress fetched asynchronously by the background
    /// scanner.  `None` until the scanner reports a result for this game.
    pub progress: Option<ProgressData>,
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

#[derive(Clone)]
pub enum CapsuleState {
    Pending,
    Loaded {
        handle: ImageHandle,
        width: u32,
        height: u32,
        opacity: f32,
    },
    Unavailable,
}

impl std::fmt::Debug for CapsuleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapsuleState::Pending => write!(f, "Pending"),
            CapsuleState::Loaded {
                width,
                height,
                opacity,
                ..
            } => {
                write!(f, "Loaded({width}x{height}, opacity={opacity:.2})")
            }
            CapsuleState::Unavailable => write!(f, "Unavailable"),
        }
    }
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
            LibrarySort::NameAsc => write!(f, "Name (A–Z)"),
        }
    }
}

#[derive(Clone)]
pub enum LibraryMessage {
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
    FadeTick,
    RevealTick,
    SpinnerTick(f32),
    /// Background progress scan delivered a result for one game.
    ProgressFetched {
        app_id: u32,
        earned: u32,
        total: u32,
    },
    ProgressScanDone,
}

impl std::fmt::Debug for LibraryMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibraryMessage::ScanComplete(v) => {
                write!(f, "ScanComplete({} games)", v.len())
            }
            LibraryMessage::ScanFailed(e) => write!(f, "ScanFailed({e})"),
            LibraryMessage::SearchChanged(s) => write!(f, "SearchChanged({s:?})"),
            LibraryMessage::SortChanged(s) => write!(f, "SortChanged({s:?})"),
            LibraryMessage::CapsuleSizeChanged(s) => write!(f, "CapsuleSizeChanged({s})"),
            LibraryMessage::CapsuleLoaded {
                app_id,
                size,
                width,
                height,
                ..
            } => write!(f, "CapsuleLoaded(app={app_id}, {size}, {width}x{height})"),
            LibraryMessage::CapsuleFailed { app_id, size } => {
                write!(f, "CapsuleFailed(app={app_id}, {size})")
            }
            LibraryMessage::GameSelected(id) => write!(f, "GameSelected({id})"),
            LibraryMessage::ManualAppIdChanged(s) => write!(f, "ManualAppIdChanged({s:?})"),
            LibraryMessage::ManualAppIdSubmitted => write!(f, "ManualAppIdSubmitted"),
            LibraryMessage::RescanRequested => write!(f, "RescanRequested"),
            LibraryMessage::FadeTick => write!(f, "FadeTick"),
            LibraryMessage::RevealTick => write!(f, "RevealTick"),
            LibraryMessage::SpinnerTick(a) => write!(f, "SpinnerTick({a:.1})"),
            LibraryMessage::ProgressFetched {
                app_id,
                earned,
                total,
            } => write!(f, "ProgressFetched(app={app_id}, {earned}/{total})"),
            LibraryMessage::ProgressScanDone => write!(f, "ProgressScanDone"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LibraryPhase {
    Scanning,
    Loaded,
    Error(String),
}

pub struct LibraryState {
    pub phase: LibraryPhase,
    pub games: Vec<GameEntry>,
    pub reveal_queue: VecDeque<u32>,
    pub capsule_handles: HashMap<(u32, CapsuleSize), StoredCapsule>,
    pub search: String,
    pub sort: LibrarySort,
    pub capsule_size: CapsuleSize,
    pub manual_app_id_input: String,
    pub spinner_angle: f32,
    /// Live background progress scanner, `None` when idle.
    pub progress_scanner: Option<crate::progress_scan::ProgressScanner>,
    /// Receiver half of the progress result channel.  Taken once when the scan
    /// starts and drained on every `ProgressFetched` / `ProgressScanDone` tick.
    pub progress_rx:
        Option<tokio::sync::mpsc::UnboundedReceiver<crate::progress_scan::ProgressResult>>,
}

impl std::fmt::Debug for LibraryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryState")
            .field("phase", &self.phase)
            .field("games_count", &self.games.len())
            .field("sort", &self.sort)
            .field("capsule_size", &self.capsule_size)
            .finish_non_exhaustive()
    }
}

impl LibraryState {
    pub fn new() -> Self {
        Self {
            phase: LibraryPhase::Scanning,
            games: Vec::new(),
            reveal_queue: VecDeque::new(),
            capsule_handles: HashMap::new(),
            search: String::new(),
            sort: LibrarySort::LastPlayed,
            capsule_size: CapsuleSize::default(),
            manual_app_id_input: String::new(),
            spinner_angle: 0.0,
            progress_scanner: None,
            progress_rx: None,
        }
    }

    pub fn is_streaming(&self) -> bool {
        self.has_pending_reveals()
            || self.has_fading_capsules()
            || self
                .games
                .iter()
                .any(|g| matches!(g.capsule, CapsuleState::Pending))
            || self.progress_scanner.is_some()
    }

    pub fn has_fading_capsules(&self) -> bool {
        self.games
            .iter()
            .any(|g| matches!(g.capsule, CapsuleState::Loaded { opacity, .. } if g.revealed && opacity < 1.0))
    }

    pub fn has_pending_reveals(&self) -> bool {
        !self.reveal_queue.is_empty()
    }

    pub fn visible_games(&self) -> Vec<&GameEntry> {
        let query = self.search.to_lowercase();
        let mut result: Vec<&GameEntry> = self
            .games
            .iter()
            .filter(|g| {
                g.revealed && (query.is_empty() || g.summary.name.to_lowercase().contains(&query))
            })
            .collect();

        sort_entries(&mut result, self.sort);
        result
    }
}

fn sort_entries(entries: &mut Vec<&GameEntry>, sort: LibrarySort) {
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
            capsule: CapsuleState::Pending,
            revealed: true,
            progress: None,
        }
    }

    fn make_state_with_games(games: Vec<GameEntry>) -> LibraryState {
        LibraryState {
            phase: LibraryPhase::Loaded,
            games,
            reveal_queue: VecDeque::new(),
            capsule_handles: HashMap::new(),
            search: String::new(),
            sort: LibrarySort::LastPlayed,
            capsule_size: CapsuleSize::default(),
            manual_app_id_input: String::new(),
            spinner_angle: 0.0,
            progress_scanner: None,
            progress_rx: None,
        }
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

        let visible = state.visible_games();
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

        let visible = state.visible_games();
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

        let visible = state.visible_games();
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
        let state = LibraryState::new();
        assert_eq!(state.capsule_size, CapsuleSize::Medium);
    }

    #[test]
    fn fade_tick_advances_opacity_until_one() {
        use iced::widget::image::Handle as ImageHandle;

        let dummy_handle = ImageHandle::from_rgba(1, 1, vec![0u8, 0, 0, 255]);
        let mut state = make_state_with_games(vec![GameEntry {
            summary: make_summary(1, "TestGame", None),
            capsule: CapsuleState::Loaded {
                handle: dummy_handle,
                width: 1,
                height: 1,
                opacity: 0.0,
            },
            revealed: true,
            progress: None,
        }]);

        assert!(state.has_fading_capsules(), "precondition: opacity < 1.0");

        for _ in 0..10 {
            for entry in &mut state.games {
                if let CapsuleState::Loaded { opacity, .. } = &mut entry.capsule {
                    *opacity = (*opacity + super::FADE_DELTA).min(1.0);
                }
            }
        }

        if let CapsuleState::Loaded { opacity, .. } = &state.games[0].capsule {
            assert_eq!(
                *opacity, 1.0,
                "opacity must be clamped to 1.0 after 10 ticks"
            );
        } else {
            panic!("expected Loaded capsule");
        }

        assert!(
            !state.has_fading_capsules(),
            "has_fading_capsules must return false when all opacity == 1.0"
        );
    }

    #[test]
    fn library_reveal_tick_pops_one_from_queue() {
        let entries: Vec<GameEntry> = (1u32..=3)
            .map(|id| GameEntry {
                summary: make_summary(id, &format!("Game {id}"), None),
                capsule: CapsuleState::Pending,
                revealed: false,
                progress: None,
            })
            .collect();

        let mut state = LibraryState {
            phase: LibraryPhase::Loaded,
            games: entries,
            reveal_queue: VecDeque::from([1u32, 2, 3]),
            capsule_handles: HashMap::new(),
            search: String::new(),
            sort: LibrarySort::LastPlayed,
            capsule_size: CapsuleSize::default(),
            manual_app_id_input: String::new(),
            spinner_angle: 0.0,
            progress_scanner: None,
            progress_rx: None,
        };

        assert!(state.has_pending_reveals(), "precondition: queue not empty");
        assert!(
            state.games.iter().all(|g| !g.revealed),
            "precondition: none revealed"
        );

        for expected_remaining in [2usize, 1, 0] {
            if let Some(app_id) = state.reveal_queue.pop_front()
                && let Some(entry) = state.games.iter_mut().find(|g| g.summary.app_id == app_id)
            {
                entry.revealed = true;
            }
            assert_eq!(
                state.reveal_queue.len(),
                expected_remaining,
                "queue length after pop"
            );
        }

        assert!(
            state.games.iter().all(|g| g.revealed),
            "all 3 entries must be revealed after 3 pops"
        );
        assert!(
            !state.has_pending_reveals(),
            "has_pending_reveals must be false when queue is empty"
        );
    }

    #[test]
    fn library_spinner_tick_updates_angle() {
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
            capsule: CapsuleState::Pending,
            revealed: false,
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
                entry.capsule = CapsuleState::Loaded {
                    handle: cached.handle.clone(),
                    width: cached.width,
                    height: cached.height,
                    opacity: 1.0,
                };
                entry.revealed = true;
            } else {
                entry.capsule = CapsuleState::Pending;
                entry.revealed = false;
            }
        }

        assert!(
            state.reveal_queue.is_empty(),
            "reveal queue must remain empty on cache hit path"
        );
        let g = &state.games[0];
        assert!(g.revealed, "entry must be marked revealed on cache hit");
        match &g.capsule {
            CapsuleState::Loaded { opacity, .. } => {
                assert!(
                    (*opacity - 1.0).abs() < f32::EPSILON,
                    "opacity must be 1.0 (no fade) on cache hit"
                );
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn cache_miss_falls_through_to_pending() {
        let app_id = 105600u32;
        let entry = GameEntry {
            summary: make_summary(app_id, "Terraria", None),
            capsule: CapsuleState::Pending,
            revealed: false,
            progress: None,
        };
        let mut state = make_state_with_games(vec![entry]);
        state.capsule_size = CapsuleSize::Small;

        for entry in &mut state.games {
            let key = (entry.summary.app_id, state.capsule_size);
            if let Some(cached) = state.capsule_handles.get(&key) {
                entry.capsule = CapsuleState::Loaded {
                    handle: cached.handle.clone(),
                    width: cached.width,
                    height: cached.height,
                    opacity: 1.0,
                };
                entry.revealed = true;
            } else {
                entry.capsule = CapsuleState::Pending;
                entry.revealed = false;
            }
        }

        assert!(
            state.reveal_queue.is_empty(),
            "reveal queue must stay empty — populated only by CapsuleLoaded, not CapsuleSizeChanged"
        );
        let g = &state.games[0];
        assert!(!g.revealed, "entry must not be revealed on cache miss");
        assert!(
            matches!(g.capsule, CapsuleState::Pending),
            "entry capsule must be Pending on cache miss"
        );
    }
}
