#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameStatusFilter {
    #[default]
    All,
    Started,
    Completed,
    NotStarted,
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
            LibrarySort::NameAsc => "A \u{2014} Z",
            LibrarySort::LastPlayed => "Last played",
            LibrarySort::Completion => "Completion",
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
