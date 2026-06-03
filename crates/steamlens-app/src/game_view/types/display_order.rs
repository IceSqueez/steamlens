use std::cmp::Ordering;
use std::collections::HashMap;

use super::filters::{AchievementFilter, AchievementSort};
use super::rarity::RarityTier;
#[cfg(test)]
use super::rarity::compute_tier_map;
use super::rows::AchievementRow;

fn display_group(row: &AchievementRow) -> u8 {
    if row.data.is_achieved {
        0
    } else if row.data.is_hidden {
        2
    } else {
        1
    }
}

fn tier_rank(tier: Option<RarityTier>) -> u8 {
    match tier {
        Some(RarityTier::Common) => 0,
        Some(RarityTier::Uncommon) => 1,
        Some(RarityTier::Rare) => 2,
        Some(RarityTier::Mythical) => 3,
        Some(RarityTier::Legendary) => 4,
        None => 5,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn visible_achievement_indices(
    achievements: &[AchievementRow],
    tier_map: &HashMap<String, RarityTier>,
    filter: AchievementFilter,
    search: &str,
    sort: AchievementSort,
    rarity_tier_set: &std::collections::HashSet<RarityTier>,
    include_hidden: bool,
    unlocked_at_top: bool,
) -> Vec<usize> {
    let query = search.to_lowercase();
    let mut filtered: Vec<(usize, &AchievementRow)> = achievements
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            if !row.has_appeared {
                return false;
            }
            let is_spoiler = row.is_spoiler_hidden();
            let is_hidden_attribute = row.data.is_hidden;
            let filter_ok = match filter {
                AchievementFilter::All => true,
                AchievementFilter::Unlocked => row.data.is_achieved,
                AchievementFilter::Locked => !row.data.is_achieved,
            };
            let search_ok = query.is_empty()
                || row.data.display_name.to_lowercase().contains(&query)
                || row.data.description.to_lowercase().contains(&query)
                || row.data.id.to_lowercase().contains(&query);
            let any_pill_selected = !rarity_tier_set.is_empty() || include_hidden;
            let rarity_ok = if !any_pill_selected {
                true
            } else {
                let tier_match = !is_spoiler
                    && match tier_map.get(&row.data.id).copied() {
                        Some(tier) => rarity_tier_set.contains(&tier),
                        None => false,
                    };
                let hidden_match = is_hidden_attribute && include_hidden;
                tier_match || hidden_match
            };
            filter_ok && search_ok && rarity_ok
        })
        .collect();
    sort_indexed_for_display(&mut filtered, tier_map, sort, unlocked_at_top);
    filtered.into_iter().map(|(i, _)| i).collect()
}

fn sort_indexed_for_display(
    rows: &mut [(usize, &AchievementRow)],
    tier_map: &HashMap<String, RarityTier>,
    sort: AchievementSort,
    unlocked_at_top: bool,
) {
    match sort {
        AchievementSort::UnlockChance => {
            rows.sort_by(|(_, a), (_, b)| {
                let group_cmp = if unlocked_at_top {
                    display_group(a).cmp(&display_group(b))
                } else {
                    Ordering::Equal
                };
                group_cmp
                    .then_with(|| {
                        let pa = a.rarity_percent;
                        let pb = b.rarity_percent;
                        match (pa, pb) {
                            (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(Ordering::Equal),
                            (Some(_), None) => Ordering::Less,
                            (None, Some(_)) => Ordering::Greater,
                            (None, None) => Ordering::Equal,
                        }
                    })
                    .then_with(|| {
                        a.data
                            .display_name
                            .to_lowercase()
                            .cmp(&b.data.display_name.to_lowercase())
                    })
            });
        }
        AchievementSort::RarityAndName => {
            rows.sort_by(|(_, a), (_, b)| {
                let group_cmp = if unlocked_at_top {
                    display_group(a).cmp(&display_group(b))
                } else {
                    Ordering::Equal
                };
                group_cmp
                    .then_with(|| {
                        tier_rank(tier_map.get(&a.data.id).copied())
                            .cmp(&tier_rank(tier_map.get(&b.data.id).copied()))
                    })
                    .then_with(|| {
                        a.data
                            .display_name
                            .to_lowercase()
                            .cmp(&b.data.display_name.to_lowercase())
                    })
            });
        }
        AchievementSort::Name => {
            rows.sort_by(|(_, a), (_, b)| {
                let group_cmp = if unlocked_at_top {
                    display_group(a).cmp(&display_group(b))
                } else {
                    Ordering::Equal
                };
                group_cmp.then_with(|| {
                    a.data
                        .display_name
                        .to_lowercase()
                        .cmp(&b.data.display_name.to_lowercase())
                })
            });
        }
    }
}

#[cfg(test)]
pub fn visible_achievement_ids<'a>(
    achievements: &'a [AchievementRow],
    filter: AchievementFilter,
    search: &str,
    sort: AchievementSort,
    rarity_tier_set: &std::collections::HashSet<RarityTier>,
    include_hidden: bool,
) -> Vec<&'a str> {
    let tier_map = compute_tier_map(achievements);
    visible_achievement_indices(
        achievements,
        &tier_map,
        filter,
        search,
        sort,
        rarity_tier_set,
        include_hidden,
        true,
    )
    .into_iter()
    .map(|i| achievements[i].data.id.as_str())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_view::types::rows::AchievementData;

    fn make_achievement_row(id: &str, rarity: Option<f32>) -> AchievementRow {
        AchievementRow {
            data: AchievementData {
                id: id.to_owned(),
                display_name: id.to_owned(),
                description: String::new(),
                is_achieved: false,
                unlock_time: None,
                is_hidden: false,
                permission: 0,
                icon: None,
            },
            is_dirty: false,
            is_revealed: false,
            has_appeared: false,
            card_opacity: 0.0,
            rarity_percent: rarity,
        }
    }

    fn make_appeared(id: &str, rarity: Option<f32>) -> AchievementRow {
        AchievementRow {
            has_appeared: true,
            ..make_achievement_row(id, rarity)
        }
    }

    fn make_linear_rows(count: usize) -> Vec<AchievementRow> {
        (0..count)
            .map(|i| {
                let pct = (i as f32 / (count - 1).max(1) as f32) * 100.0;
                make_achievement_row(&format!("a{i}"), Some(pct))
            })
            .collect()
    }

    fn make_hidden_row(
        id: &str,
        is_achieved: bool,
        is_revealed: bool,
        rarity: Option<f32>,
    ) -> AchievementRow {
        AchievementRow {
            data: AchievementData {
                id: id.to_owned(),
                display_name: id.to_owned(),
                description: String::new(),
                is_achieved,
                unlock_time: None,
                is_hidden: true,
                permission: 0,
                icon: None,
            },
            is_dirty: false,
            is_revealed,
            has_appeared: true,
            card_opacity: 1.0,
            rarity_percent: rarity,
        }
    }

    #[test]
    fn sort_by_name_ignores_tier() {
        let rows = vec![
            make_appeared("zebra", Some(1.0)),
            make_appeared("apple", Some(90.0)),
            make_appeared("mango", Some(30.0)),
        ];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::All,
            "",
            AchievementSort::Name,
            &std::collections::HashSet::new(),
            false,
        );
        assert_eq!(ids, vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn sort_by_unlock_chance_high_percent_first() {
        let rows = vec![
            make_appeared("rare_ach", Some(2.0)),
            make_appeared("common_ach", Some(95.0)),
            make_appeared("mid_ach", Some(40.0)),
        ];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::All,
            "",
            AchievementSort::UnlockChance,
            &std::collections::HashSet::new(),
            false,
        );
        assert_eq!(ids[0], "common_ach", "highest % first");
        assert_eq!(ids[1], "mid_ach");
        assert_eq!(ids[2], "rare_ach", "lowest % last");
    }

    #[test]
    fn sort_by_unlock_chance_none_goes_to_end() {
        let rows = vec![
            make_appeared("no_data", None),
            make_appeared("has_data", Some(50.0)),
        ];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::All,
            "",
            AchievementSort::UnlockChance,
            &std::collections::HashSet::new(),
            false,
        );
        assert_eq!(ids[0], "has_data");
        assert_eq!(ids[1], "no_data", "None rarity goes to end");
    }

    #[test]
    fn rarity_filter_legendary_only() {
        let rows: Vec<AchievementRow> = make_linear_rows(10)
            .into_iter()
            .map(|mut r| {
                r.has_appeared = true;
                r
            })
            .collect();
        let map = compute_tier_map(&rows);
        let legendary_count = map
            .values()
            .filter(|&&t| t == RarityTier::Legendary)
            .count();

        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::All,
            "",
            AchievementSort::RarityAndName,
            &std::collections::HashSet::from([RarityTier::Legendary]),
            false,
        );
        assert_eq!(
            ids.len(),
            legendary_count,
            "filter must return exactly the legendary tier count"
        );
    }

    #[test]
    fn rarity_filter_excludes_no_data_when_specific() {
        let rows = vec![
            make_appeared("r1", Some(10.0)),
            make_appeared("r2", Some(50.0)),
            make_appeared("r3", Some(90.0)),
            make_appeared("no_data", None),
        ];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::All,
            "",
            AchievementSort::RarityAndName,
            &std::collections::HashSet::from([RarityTier::Common]),
            false,
        );
        assert!(
            !ids.contains(&"no_data"),
            "unrated must be excluded by specific filter"
        );
    }

    #[test]
    fn filter_locked_keeps_dirty_unlock_toggle_in_locked_group() {
        let row = AchievementRow {
            has_appeared: true,
            is_dirty: true,
            ..make_achievement_row("dirty_unlock", None)
        };
        let rows = [row];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::Locked,
            "",
            AchievementSort::Name,
            &std::collections::HashSet::new(),
            false,
        );
        assert!(
            ids.contains(&"dirty_unlock"),
            "pending-unlock row must still be visible under Locked filter (persisted state = locked)"
        );
    }

    #[test]
    fn filter_unlocked_keeps_dirty_lock_toggle_in_unlocked_group() {
        let row = AchievementRow {
            has_appeared: true,
            is_dirty: true,
            data: AchievementData {
                id: "dirty_lock".to_owned(),
                display_name: "dirty_lock".to_owned(),
                description: String::new(),
                is_achieved: true,
                unlock_time: None,
                is_hidden: false,
                permission: 0,
                icon: None,
            },
            is_revealed: false,
            card_opacity: 1.0,
            rarity_percent: None,
        };
        let rows = [row];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::Unlocked,
            "",
            AchievementSort::Name,
            &std::collections::HashSet::new(),
            false,
        );
        assert!(
            ids.contains(&"dirty_lock"),
            "pending-lock row must still be visible under Unlocked filter (persisted state = unlocked)"
        );
    }

    #[test]
    fn filter_locked_default_includes_spoiler() {
        let rows = vec![
            make_appeared("regular_locked", None),
            make_hidden_row("spoiler", false, false, None),
            make_hidden_row("revealed_hidden", false, true, None),
        ];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::Locked,
            "",
            AchievementSort::Name,
            &std::collections::HashSet::new(),
            false,
        );
        assert!(
            ids.contains(&"regular_locked"),
            "regular locked row must appear under Locked filter"
        );
        assert!(
            ids.contains(&"revealed_hidden"),
            "revealed hidden (no longer spoiler) must appear under Locked filter"
        );
        assert!(
            ids.contains(&"spoiler"),
            "no pills selected = Locked filter includes spoiler-locked rows by default"
        );
    }

    #[test]
    fn filter_locked_with_only_hidden_pill_keeps_just_spoilers() {
        let rows = vec![
            make_appeared("regular_locked", None),
            make_hidden_row("spoiler", false, false, None),
        ];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::Locked,
            "",
            AchievementSort::Name,
            &std::collections::HashSet::new(),
            true,
        );
        assert_eq!(ids, vec!["spoiler"]);
    }

    #[test]
    fn filter_unlocked_includes_earned_hidden() {
        let rows = vec![make_hidden_row("earned_secret", true, false, None)];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::Unlocked,
            "",
            AchievementSort::Name,
            &std::collections::HashSet::new(),
            false,
        );
        assert!(
            ids.contains(&"earned_secret"),
            "earned hidden achievement (is_achieved=true) must be visible under Unlocked"
        );
    }

    #[test]
    fn default_state_shows_spoilers_under_status_all() {
        let rows = vec![
            make_appeared("regular", None),
            make_hidden_row("spoiler", false, false, None),
        ];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::All,
            "",
            AchievementSort::Name,
            &std::collections::HashSet::new(),
            false,
        );
        assert!(
            ids.contains(&"spoiler"),
            "no pills selected = show everything, including spoilers"
        );
    }

    #[test]
    fn rarity_filter_legendary_excludes_spoiler() {
        let rows = vec![
            make_appeared("a0", Some(1.0)),
            make_appeared("a1", Some(2.0)),
            make_appeared("a2", Some(3.0)),
            make_hidden_row("hidden_legendary", false, false, Some(0.5)),
        ];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::All,
            "",
            AchievementSort::Name,
            &std::collections::HashSet::from([RarityTier::Legendary]),
            false,
        );
        assert!(
            !ids.contains(&"hidden_legendary"),
            "spoiler-hidden achievement must be excluded from Legendary rarity filter"
        );
    }

    #[test]
    fn no_pills_selected_shows_all_including_spoilers() {
        let rows = vec![
            make_appeared("a0", Some(1.0)),
            make_appeared("a1", Some(2.0)),
            make_appeared("a2", Some(3.0)),
            make_hidden_row("hidden_legendary", false, false, Some(0.5)),
        ];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::All,
            "",
            AchievementSort::Name,
            &std::collections::HashSet::new(),
            false,
        );
        assert_eq!(ids.len(), 4);
        assert!(
            ids.contains(&"hidden_legendary"),
            "no pills selected = default show-all includes spoilers"
        );
    }

    #[test]
    fn only_hidden_pill_filters_to_spoilers_only() {
        let rows = vec![
            make_appeared("a0", Some(1.0)),
            make_hidden_row("hidden_legendary", false, false, Some(0.5)),
        ];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::All,
            "",
            AchievementSort::Name,
            &std::collections::HashSet::new(),
            true,
        );
        assert_eq!(ids, vec!["hidden_legendary"]);
    }

    #[test]
    fn tier_and_hidden_pills_show_union() {
        let rows = vec![
            make_appeared("non_legendary_common", Some(80.0)),
            make_appeared("legendary_a", Some(1.0)),
            make_appeared("legendary_b", Some(2.0)),
            make_hidden_row("hidden_row", false, false, Some(60.0)),
        ];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::All,
            "",
            AchievementSort::Name,
            &std::collections::HashSet::from([RarityTier::Legendary]),
            true,
        );
        assert!(
            ids.contains(&"hidden_row"),
            "Hidden pill must surface spoiler even when it does not sit in the chosen tier"
        );
        assert!(
            ids.contains(&"legendary_a") && ids.contains(&"legendary_b"),
            "Legendary pill must surface non-spoiler legendaries"
        );
        assert!(
            !ids.contains(&"non_legendary_common"),
            "Non-legendary, non-spoiler row must be excluded from Legendary + Hidden union"
        );
    }

    #[test]
    fn rarity_filter_legendary_includes_earned_hidden() {
        let rows = vec![
            make_appeared("a0", Some(2.0)),
            make_appeared("a1", Some(3.0)),
            make_appeared("a2", Some(4.0)),
            make_hidden_row("earned_legendary", true, false, Some(1.0)),
        ];
        let ids = visible_achievement_ids(
            &rows,
            AchievementFilter::All,
            "",
            AchievementSort::Name,
            &std::collections::HashSet::from([RarityTier::Legendary]),
            false,
        );
        assert!(
            ids.contains(&"earned_legendary"),
            "earned hidden achievement (no longer spoiler) must appear under Legendary rarity filter"
        );
    }
}
