use iced::widget::image::Handle as ImageHandle;
use steamlens_core::GameSummary;

#[derive(Clone)]
pub struct GameEntry {
    pub summary: GameSummary,
    pub capsule: CapsuleState,
}

impl std::fmt::Debug for GameEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameEntry")
            .field("app_id", &self.summary.app_id)
            .field("name", &self.summary.name)
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
    },
    Unavailable,
}

impl std::fmt::Debug for CapsuleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapsuleState::Pending => write!(f, "Pending"),
            CapsuleState::Loaded { width, height, .. } => {
                write!(f, "Loaded({width}x{height})")
            }
            CapsuleState::Unavailable => write!(f, "Unavailable"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    CardWidthChanged(f32),
    CapsuleLoaded {
        app_id: u32,
        handle: ImageHandle,
        width: u32,
        height: u32,
    },
    CapsuleFailed(u32),
    GameSelected(u32),
    ManualAppIdChanged(String),
    ManualAppIdSubmitted,
    RescanRequested,
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
            LibraryMessage::CardWidthChanged(w) => write!(f, "CardWidthChanged({w})"),
            LibraryMessage::CapsuleLoaded {
                app_id,
                width,
                height,
                ..
            } => write!(f, "CapsuleLoaded(app={app_id}, {width}x{height})"),
            LibraryMessage::CapsuleFailed(id) => write!(f, "CapsuleFailed({id})"),
            LibraryMessage::GameSelected(id) => write!(f, "GameSelected({id})"),
            LibraryMessage::ManualAppIdChanged(s) => write!(f, "ManualAppIdChanged({s:?})"),
            LibraryMessage::ManualAppIdSubmitted => write!(f, "ManualAppIdSubmitted"),
            LibraryMessage::RescanRequested => write!(f, "RescanRequested"),
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
    pub search: String,
    pub sort: LibrarySort,
    pub card_width: f32,
    pub manual_app_id_input: String,
    pub has_opened_a_game: bool,
}

impl std::fmt::Debug for LibraryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryState")
            .field("phase", &self.phase)
            .field("games_count", &self.games.len())
            .field("sort", &self.sort)
            .finish_non_exhaustive()
    }
}

impl LibraryState {
    pub fn new() -> Self {
        Self {
            phase: LibraryPhase::Scanning,
            games: Vec::new(),
            search: String::new(),
            sort: LibrarySort::LastPlayed,
            card_width: 160.0,
            manual_app_id_input: String::new(),
            has_opened_a_game: false,
        }
    }

    pub fn visible_games(&self) -> Vec<&GameEntry> {
        let query = self.search.to_lowercase();
        let mut result: Vec<&GameEntry> = self
            .games
            .iter()
            .filter(|g| query.is_empty() || g.summary.name.to_lowercase().contains(&query))
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
        }
    }

    fn make_entry(app_id: u32, name: &str, last_played: Option<u32>) -> GameEntry {
        GameEntry {
            summary: make_summary(app_id, name, last_played),
            capsule: CapsuleState::Pending,
        }
    }

    fn make_state_with_games(games: Vec<GameEntry>) -> LibraryState {
        LibraryState {
            phase: LibraryPhase::Loaded,
            games,
            search: String::new(),
            sort: LibrarySort::LastPlayed,
            card_width: 160.0,
            manual_app_id_input: String::new(),
            has_opened_a_game: false,
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
}
