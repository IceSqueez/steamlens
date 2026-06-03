use std::cmp::Ordering;
use std::collections::HashMap;

use super::rows::AchievementRow;

const TIER_PLAN: [(RarityTier, f32, f32); 4] = [
    (RarityTier::Legendary, 0.05, 10.0),
    (RarityTier::Mythical, 0.10, 15.0),
    (RarityTier::Rare, 0.15, 30.0),
    (RarityTier::Uncommon, 0.25, 50.0),
];

const TIE_BREAK_EPSILON: f32 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RarityTier {
    Common,
    Uncommon,
    Rare,
    Mythical,
    Legendary,
}

impl RarityTier {
    pub fn label(self) -> &'static str {
        match self {
            RarityTier::Common => "Common",
            RarityTier::Uncommon => "Uncommon",
            RarityTier::Rare => "Rare",
            RarityTier::Mythical => "Mythical",
            RarityTier::Legendary => "Legendary",
        }
    }
}

pub fn compute_tier_map(achievements: &[AchievementRow]) -> HashMap<String, RarityTier> {
    let rated_achievements: Vec<(String, f32)> = achievements
        .iter()
        .filter_map(|row| {
            row.rarity_percent
                .map(|percent| (row.data.id.clone(), percent))
        })
        .collect();
    assign_tiers_from_percentages(rated_achievements)
}

pub fn assign_tiers_from_percentages(mut rated: Vec<(String, f32)>) -> HashMap<String, RarityTier> {
    if rated.is_empty() {
        return HashMap::new();
    }

    rated.sort_by(|left, right| {
        left.1
            .partial_cmp(&right.1)
            .unwrap_or(Ordering::Equal)
            .then(left.0.cmp(&right.0))
    });

    let total = rated.len();
    let mut tier_map = HashMap::with_capacity(total);
    let mut cursor = 0usize;

    for (tier, slot_share, ceiling_percent) in TIER_PLAN {
        if cursor >= total {
            break;
        }
        let remaining = total - cursor;
        let raw_slot_count = ((remaining as f32) * slot_share).ceil() as usize;
        let slot_count = raw_slot_count.max(1);
        let mut slot_end = (cursor + slot_count).min(total);

        if slot_end > cursor && slot_end < total {
            let boundary_percent = rated[slot_end - 1].1;
            while slot_end < total
                && (rated[slot_end].1 - boundary_percent).abs() < TIE_BREAK_EPSILON
            {
                slot_end += 1;
            }
        }

        while cursor < slot_end {
            let (id, percent) = &rated[cursor];
            if *percent < ceiling_percent {
                tier_map.insert(id.clone(), tier);
                cursor += 1;
            } else {
                break;
            }
        }
    }

    for (id, _) in &rated[cursor..] {
        tier_map.insert(id.clone(), RarityTier::Common);
    }

    tier_map
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

    fn make_linear_rows(count: usize) -> Vec<AchievementRow> {
        (0..count)
            .map(|i| {
                let percent = (i as f32 / (count - 1).max(1) as f32) * 100.0;
                make_achievement_row(&format!("a{i}"), Some(percent))
            })
            .collect()
    }

    fn count_tier(map: &HashMap<String, RarityTier>, tier: RarityTier) -> usize {
        map.values().filter(|&&entry| entry == tier).count()
    }

    #[test]
    fn empty_input_returns_empty_map() {
        let map = compute_tier_map(&[]);
        assert!(map.is_empty());
    }

    #[test]
    fn all_unrated_returns_empty_map() {
        let rows = vec![
            make_achievement_row("a", None),
            make_achievement_row("b", None),
        ];
        let map = compute_tier_map(&rows);
        assert!(map.is_empty());
    }

    #[test]
    fn unrated_achievements_excluded_from_tier_map() {
        let rows = vec![
            make_achievement_row("rated_low", Some(5.0)),
            make_achievement_row("rated_high", Some(80.0)),
            make_achievement_row("unrated", None),
        ];
        let map = compute_tier_map(&rows);
        assert!(!map.contains_key("unrated"));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn easy_game_with_all_high_percentages_assigns_only_common() {
        let rows: Vec<_> = (0..50)
            .map(|i| make_achievement_row(&format!("a{i}"), Some(50.0 + i as f32)))
            .collect();
        let map = compute_tier_map(&rows);
        assert_eq!(map.len(), 50);
        assert!(
            map.values().all(|&entry| entry == RarityTier::Common),
            "all >=50% achievements must demote through every top-tier ceiling"
        );
    }

    #[test]
    fn single_low_percent_achievement_assigned_legendary() {
        let rows = vec![make_achievement_row("solo", Some(0.5))];
        let map = compute_tier_map(&rows);
        assert_eq!(map["solo"], RarityTier::Legendary);
    }

    #[test]
    fn single_mid_percent_achievement_demotes_to_uncommon() {
        let rows = vec![make_achievement_row("solo", Some(39.1))];
        let map = compute_tier_map(&rows);
        assert_eq!(map["solo"], RarityTier::Uncommon);
    }

    #[test]
    fn single_common_percent_achievement_falls_through_all_ceilings() {
        let rows = vec![make_achievement_row("solo", Some(85.0))];
        let map = compute_tier_map(&rows);
        assert_eq!(map["solo"], RarityTier::Common);
    }

    #[test]
    fn linear_distribution_produces_cascading_caps() {
        let rows = make_linear_rows(100);
        let map = compute_tier_map(&rows);

        assert_eq!(count_tier(&map, RarityTier::Legendary), 5);
        assert_eq!(count_tier(&map, RarityTier::Mythical), 10);
        assert_eq!(count_tier(&map, RarityTier::Rare), 13);
        assert_eq!(count_tier(&map, RarityTier::Uncommon), 18);
        assert_eq!(count_tier(&map, RarityTier::Common), 54);
    }

    #[test]
    fn hardcore_game_with_all_low_percentages_still_distributes_via_caps() {
        let rows: Vec<_> = (0..100)
            .map(|i| make_achievement_row(&format!("a{i}"), Some(i as f32 * 0.05)))
            .collect();
        let map = compute_tier_map(&rows);

        let legendary = count_tier(&map, RarityTier::Legendary);
        let mythical = count_tier(&map, RarityTier::Mythical);
        let rare = count_tier(&map, RarityTier::Rare);
        let uncommon = count_tier(&map, RarityTier::Uncommon);
        let common = count_tier(&map, RarityTier::Common);

        assert_eq!(legendary + mythical + rare + uncommon + common, 100);
        assert_eq!(legendary, 5);
        assert_eq!(mythical, 10);
        assert_eq!(rare, 13);
        assert_eq!(uncommon, 18);
        assert!(
            common > 0,
            "even all-rare games yield Common via cascading caps"
        );
    }

    #[test]
    fn each_assigned_tier_satisfies_its_ceiling() {
        let rows = make_linear_rows(100);
        let map = compute_tier_map(&rows);

        for row in &rows {
            let percent = row.rarity_percent.expect("linear rows have percent");
            match map.get(&row.data.id).copied() {
                Some(RarityTier::Legendary) => {
                    assert!(percent < 10.0, "Legendary at {percent}%")
                }
                Some(RarityTier::Mythical) => assert!(percent < 15.0, "Mythical at {percent}%"),
                Some(RarityTier::Rare) => assert!(percent < 30.0, "Rare at {percent}%"),
                Some(RarityTier::Uncommon) => assert!(percent < 50.0, "Uncommon at {percent}%"),
                Some(RarityTier::Common) => {}
                None => panic!("linear row {} unmapped", row.data.id),
            }
        }
    }

    #[test]
    fn tier_assignment_preserves_rarity_order() {
        let rows = make_linear_rows(100);
        let map = compute_tier_map(&rows);

        let tier_rank = |tier: RarityTier| match tier {
            RarityTier::Legendary => 0,
            RarityTier::Mythical => 1,
            RarityTier::Rare => 2,
            RarityTier::Uncommon => 3,
            RarityTier::Common => 4,
        };

        let mut by_percent: Vec<_> = rows
            .iter()
            .map(|row| (row.rarity_percent.expect("linear"), map[&row.data.id]))
            .collect();
        by_percent.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let mut previous_rank = 0;
        for (percent, tier) in by_percent {
            let rank = tier_rank(tier);
            assert!(
                rank >= previous_rank,
                "rarity order violated at {percent}% ({tier:?})"
            );
            previous_rank = rank;
        }
    }

    #[test]
    fn ceiling_failure_demotes_overflow_into_next_tier() {
        let mut rows = vec![make_achievement_row("leg1", Some(5.0))];
        rows.push(make_achievement_row("myth1", Some(11.0)));
        rows.push(make_achievement_row("myth2", Some(11.5)));
        rows.push(make_achievement_row("rare_overflow1", Some(12.0)));
        rows.push(make_achievement_row("rare_overflow2", Some(12.5)));
        for i in 0..15 {
            rows.push(make_achievement_row(
                &format!("com{i}"),
                Some(60.0 + i as f32 * 0.5),
            ));
        }
        let map = compute_tier_map(&rows);

        assert_eq!(map["leg1"], RarityTier::Legendary);
        assert_eq!(map["myth1"], RarityTier::Mythical);
        assert_eq!(map["myth2"], RarityTier::Mythical);
        assert_eq!(
            map["rare_overflow1"],
            RarityTier::Rare,
            "12% overflows Mythical slot but still <30% so lands in Rare"
        );
        assert_eq!(map["rare_overflow2"], RarityTier::Rare);
        for i in 0..15 {
            assert_eq!(
                map[&format!("com{i}")],
                RarityTier::Common,
                "60%+ fails Rare and Uncommon ceilings, falls to Common"
            );
        }
    }

    #[test]
    fn two_low_percent_achievements_split_across_legendary_and_mythical() {
        let rows = vec![
            make_achievement_row("a1", Some(2.0)),
            make_achievement_row("a2", Some(5.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(map["a1"], RarityTier::Legendary);
        assert_eq!(map["a2"], RarityTier::Mythical);
    }

    #[test]
    fn tie_at_legendary_boundary_extends_tier_past_slot_cap() {
        let rows = vec![
            make_achievement_row("a", Some(1.0)),
            make_achievement_row("b", Some(1.0)),
            make_achievement_row("c", Some(1.0)),
            make_achievement_row("d", Some(1.0)),
            make_achievement_row("e", Some(2.0)),
        ];
        let map = compute_tier_map(&rows);
        for id in ["a", "b", "c", "d"] {
            assert_eq!(
                map[id],
                RarityTier::Legendary,
                "{id} tied at 1% must extend Legendary"
            );
        }
        assert_eq!(map["e"], RarityTier::Mythical);
    }

    #[test]
    fn tie_extension_stops_at_first_different_percentage() {
        let rows = vec![
            make_achievement_row("a", Some(1.0)),
            make_achievement_row("b", Some(1.0)),
            make_achievement_row("c", Some(1.0)),
            make_achievement_row("d", Some(2.0)),
            make_achievement_row("e", Some(3.0)),
        ];
        let map = compute_tier_map(&rows);
        for id in ["a", "b", "c"] {
            assert_eq!(map[id], RarityTier::Legendary);
        }
        assert_eq!(map["d"], RarityTier::Mythical);
        assert_eq!(map["e"], RarityTier::Rare);
    }

    #[test]
    fn all_same_percent_assigns_every_achievement_via_tie_extension() {
        let rows = vec![
            make_achievement_row("a", Some(1.0)),
            make_achievement_row("b", Some(1.0)),
            make_achievement_row("c", Some(1.0)),
            make_achievement_row("d", Some(1.0)),
            make_achievement_row("e", Some(1.0)),
        ];
        let map = compute_tier_map(&rows);
        assert!(map.values().all(|&entry| entry == RarityTier::Legendary));
    }

    #[test]
    fn cascading_caps_through_distinct_percentages() {
        let rows = vec![
            make_achievement_row("a", Some(1.0)),
            make_achievement_row("b", Some(2.0)),
            make_achievement_row("c", Some(3.0)),
            make_achievement_row("d", Some(3.0)),
            make_achievement_row("e", Some(3.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(map["a"], RarityTier::Legendary);
        assert_eq!(map["b"], RarityTier::Mythical);
        for id in ["c", "d", "e"] {
            assert_eq!(map[id], RarityTier::Rare);
        }
    }

    #[test]
    fn tier_progression_fills_each_tier_when_gaps_separate_percentages() {
        let rows = vec![
            make_achievement_row("a", Some(8.0)),
            make_achievement_row("b", Some(11.0)),
            make_achievement_row("c", Some(14.0)),
            make_achievement_row("d", Some(40.0)),
            make_achievement_row("e", Some(41.0)),
            make_achievement_row("f", Some(75.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(map["a"], RarityTier::Legendary);
        assert_eq!(map["b"], RarityTier::Mythical);
        assert_eq!(map["c"], RarityTier::Rare);
        assert_eq!(map["d"], RarityTier::Uncommon);
        assert_eq!(map["e"], RarityTier::Common);
        assert_eq!(map["f"], RarityTier::Common);
    }
}
