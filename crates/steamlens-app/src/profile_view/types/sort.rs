use super::entries::GameEntry;
use super::filters::LibrarySort;

pub(crate) fn sort_games_in_place(games: &mut [GameEntry], sort: LibrarySort, pinned: &[u32]) {
    games.sort_by(|a, b| {
        let pa = pinned.iter().position(|&pid| pid == a.app_id);
        let pb = pinned.iter().position(|&pid| pid == b.app_id);
        match (pa, pb) {
            (Some(ia), Some(ib)) => return ia.cmp(&ib),
            (Some(_), None) => return std::cmp::Ordering::Less,
            (None, Some(_)) => return std::cmp::Ordering::Greater,
            (None, None) => {}
        }
        cmp_by_sort(a, b, sort)
    });
}

fn cmp_by_sort(a: &GameEntry, b: &GameEntry, sort: LibrarySort) -> std::cmp::Ordering {
    is_skeleton(a)
        .cmp(&is_skeleton(b))
        .then_with(|| match sort {
            LibrarySort::LastPlayed => match (a.last_played, b.last_played) {
                (Some(ta), Some(tb)) => tb.cmp(&ta),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => entry_name(a)
                    .to_lowercase()
                    .cmp(&entry_name(b).to_lowercase()),
            },
            LibrarySort::NameAsc => entry_name(a)
                .to_lowercase()
                .cmp(&entry_name(b).to_lowercase()),
            LibrarySort::Completion => {
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
            }
        })
}

pub(super) fn sort_entries(entries: &mut Vec<&GameEntry>, sort: LibrarySort, pinned: &[u32]) {
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
