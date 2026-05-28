use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::capsule_cache::CapsuleSize;
use crate::game_view::types::RarityTier;

use super::entries::{CapsuleAsset, GameEntry, StoredCapsule};
use super::filters::{GameStatusFilter, LibrarySort};
use super::sort::sort_entries;

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
    pub last_scan_completed_at: Option<Instant>,
    pub scan_started_at: Option<Instant>,
    pub scan_target_count: usize,
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
            last_scan_completed_at: None,
            scan_started_at: None,
            scan_target_count: 0,
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
