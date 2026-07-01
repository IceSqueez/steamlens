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

use super::entries::{CapsuleAsset, GameEntry, TopEntry};
use super::filters::{GameStatusFilter, LibrarySort};
use super::sort::cmp_by_sort;

#[derive(Debug, Default)]
pub struct DerivedProfileView {
    pub visible_indices: Vec<usize>,
    pub summary: WidgetSummary,
    pub top6: Vec<TopEntry>,
    pub scanned_progress_count: usize,
    pub hydrated_count: usize,
    pub loaded_capsules_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProfileViewPhase {
    Scanning,
    Loaded,
}

pub struct ProfileViewState {
    pub phase: ProfileViewPhase,
    pub games: Vec<GameEntry>,
    pub search: String,
    pub sort: LibrarySort,
    pub capsule_size: CapsuleSize,
    pub progress_scanner: Option<crate::progress_scan::ProgressScanner>,
    pub progress_rx: SharedProgressRx,
    pub scan_generation: u64,
    pub failed_app_ids: HashSet<u32>,
    pub library_name_map: HashMap<u32, String>,
    pub hovered_card: Option<u32>,
    pub hovered_bar_slice: Option<RarityTier>,
    pub hovered_card_tier: Option<(u32, RarityTier)>,
    pub status_filter: GameStatusFilter,
    pub genre_filter: HashSet<String>,
    pub available_genres: Vec<String>,
    pub grid_scroll_y: f32,
    pub last_scan_completed_at: Option<Instant>,
    pub scan_started_at: Option<Instant>,
    pub scan_target_count: usize,
    pub search_debounce_generation: u64,
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
            search: String::new(),
            sort: LibrarySort::LastPlayed,
            capsule_size: CapsuleSize::default(),
            progress_scanner: None,
            progress_rx: Arc::new(Mutex::new(None)),
            scan_generation: 0,
            failed_app_ids: HashSet::new(),
            library_name_map: HashMap::new(),
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
            search_debounce_generation: 0,
            derived: DerivedProfileView::default(),
        }
    }

    pub fn start_scan(&mut self, app_ids: Vec<u32>) {
        let (scanner, rx) = crate::progress_scan::ProgressScanner::spawn(app_ids);
        self.scan_generation = self.scan_generation.wrapping_add(1);
        *self.progress_rx.lock().unwrap_or_else(|e| e.into_inner()) = Some(rx);
        self.progress_scanner = Some(scanner);
    }

    pub fn stop_scan(&mut self) {
        self.progress_scanner = None;
        *self.progress_rx.lock().unwrap_or_else(|e| e.into_inner()) = None;
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
        let (scanned_progress_count, hydrated_count, loaded_capsules_count) =
            count_stream_progress(&self.games);
        self.derived = DerivedProfileView {
            visible_indices,
            summary,
            top6,
            scanned_progress_count,
            hydrated_count,
            loaded_capsules_count,
        };
    }

    pub fn recount_stream_progress(&mut self) {
        let (scanned, hydrated, loaded) = count_stream_progress(&self.games);
        self.derived.scanned_progress_count = scanned;
        self.derived.hydrated_count = hydrated;
        self.derived.loaded_capsules_count = loaded;
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

    pub(crate) fn rebuild_available_genres(&mut self) {
        use std::collections::BTreeSet;
        let set: BTreeSet<String> = self.games.iter().filter_map(|g| g.genre.clone()).collect();
        self.available_genres = set.into_iter().collect();
    }

    pub fn has_active_animations(&self) -> bool {
        self.is_streaming()
    }
}

fn count_stream_progress(games: &[GameEntry]) -> (usize, usize, usize) {
    let mut scanned_progress_count = 0;
    let mut hydrated_count = 0;
    let mut loaded_capsules_count = 0;
    for g in games {
        if g.progress.is_some() {
            scanned_progress_count += 1;
        }
        if g.is_hydrated() {
            hydrated_count += 1;
        }
        if !matches!(g.capsule, CapsuleAsset::Pending) {
            loaded_capsules_count += 1;
        }
    }
    (
        scanned_progress_count,
        hydrated_count,
        loaded_capsules_count,
    )
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
