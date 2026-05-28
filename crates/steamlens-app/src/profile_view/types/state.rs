use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::sync::mpsc;

use crate::cache::GameCacheEntry;
use crate::capsule_cache::CapsuleSize;
use crate::game_view::types::RarityTier;
use crate::profile_view::widget::{compute_profile_summary, top6_closest_to_complete};
use crate::progress_scan::ProgressResult;
use crate::ui::widgets::widget::WidgetSummary;

pub type SharedProgressRx = Arc<Mutex<Option<mpsc::UnboundedReceiver<ProgressResult>>>>;

use super::entries::{CapsuleAsset, GameEntry, StoredCapsule, TopEntry};
use super::filters::{GameStatusFilter, LibrarySort};
use super::sort::cmp_by_sort;

#[derive(Debug, Default)]
pub struct DerivedProfileView {
    pub visible_indices: Vec<usize>,
    pub summary: WidgetSummary,
    pub top6: Vec<TopEntry>,
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
    pub progress_rx: SharedProgressRx,
    pub scan_generation: u64,
    pub failed_app_ids: HashSet<u32>,
    pub library_name_map: HashMap<u32, String>,
    pub loader_pulse_phase: f32,
    pub loader_hiding_since: Option<Instant>,
    pub hovered_card: Option<u32>,
    pub hovered_bar_slice: Option<RarityTier>,
    pub hovered_card_tier: Option<(u32, RarityTier)>,
    pub status_filter: GameStatusFilter,
    pub genre_filter: HashSet<String>,
    pub last_scan_completed_at: Option<Instant>,
    pub scan_started_at: Option<Instant>,
    pub scan_target_count: usize,
    pub derived: DerivedProfileView,
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
            last_scan_completed_at: None,
            scan_started_at: None,
            scan_target_count: 0,
            derived: DerivedProfileView::default(),
        }
    }

    pub fn start_scan(&mut self, app_ids: Vec<u32>) {
        let (scanner, rx) = crate::progress_scan::ProgressScanner::spawn(app_ids);
        self.scan_generation = self.scan_generation.wrapping_add(1);
        *self.progress_rx.lock().expect("progress_rx poisoned") = Some(rx);
        self.progress_scanner = Some(scanner);
    }

    pub fn stop_scan(&mut self) {
        self.progress_scanner = None;
        *self.progress_rx.lock().expect("progress_rx poisoned") = None;
    }

    pub fn recompute_derived(
        &mut self,
        cached_entries: &HashMap<u32, GameCacheEntry>,
        pinned: &[u32],
    ) {
        let visible_indices = compute_visible_indices(
            &self.games,
            &self.search,
            self.status_filter,
            &self.genre_filter,
            self.sort,
            pinned,
        );
        let summary = compute_profile_summary(cached_entries);
        let top6 = top6_closest_to_complete(&self.games, cached_entries);
        self.derived = DerivedProfileView {
            visible_indices,
            summary,
            top6,
        };
    }

    pub fn is_streaming(&self) -> bool {
        self.games
            .iter()
            .any(|g| matches!(g.capsule, CapsuleAsset::Pending))
            || self.progress_scanner.is_some()
    }

    #[cfg(test)]
    pub fn visible_games<'a>(&'a self, pinned: &[u32]) -> Vec<&'a GameEntry> {
        compute_visible_indices(
            &self.games,
            &self.search,
            self.status_filter,
            &self.genre_filter,
            self.sort,
            pinned,
        )
        .into_iter()
        .map(|i| &self.games[i])
        .collect()
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

    pub fn tick_animations(&mut self, delta_secs: f32, steam_running: Option<bool>) {
        const SPINNER_DEG_PER_SEC: f32 = 75.0;
        const LOADER_PULSE_PER_SEC: f32 = 0.57;

        if self.is_streaming() {
            self.spinner_angle = (self.spinner_angle + SPINNER_DEG_PER_SEC * delta_secs) % 360.0;
        }

        if self.loader_needs_pulse_subscription(steam_running) {
            self.loader_pulse_phase =
                (self.loader_pulse_phase + LOADER_PULSE_PER_SEC * delta_secs).rem_euclid(1.0);
            if let LoaderPhase::Gamma = self.loader_phase(steam_running) {
                if self.loader_hiding_since.is_none() {
                    self.loader_hiding_since = Some(Instant::now());
                }
            } else {
                self.loader_hiding_since = None;
            }
        }
    }

    pub fn has_active_animations(&self, steam_running: Option<bool>) -> bool {
        self.is_streaming() || self.loader_needs_pulse_subscription(steam_running)
    }
}

fn compute_visible_indices(
    games: &[GameEntry],
    search: &str,
    status: GameStatusFilter,
    genre_filter: &HashSet<String>,
    sort: LibrarySort,
    pinned: &[u32],
) -> Vec<usize> {
    let query = search.to_lowercase();
    let mut indices: Vec<usize> = games
        .iter()
        .enumerate()
        .filter(|(_, g)| {
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
        .map(|(i, _)| i)
        .collect();

    indices.sort_by(|&ia, &ib| {
        let a = &games[ia];
        let b = &games[ib];
        let pa = pinned.iter().position(|&pid| pid == a.app_id);
        let pb = pinned.iter().position(|&pid| pid == b.app_id);
        match (pa, pb) {
            (Some(ipa), Some(ipb)) => return ipa.cmp(&ipb),
            (Some(_), None) => return std::cmp::Ordering::Less,
            (None, Some(_)) => return std::cmp::Ordering::Greater,
            (None, None) => {}
        }
        cmp_by_sort(a, b, sort)
    });

    indices
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoaderPhase {
    Alpha,
    Beta { loaded: usize, total: usize },
    Gamma,
    Failed { failed: usize, total: usize },
    SteamOff,
}
