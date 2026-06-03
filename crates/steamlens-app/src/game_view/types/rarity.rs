use std::cmp::Ordering;
use std::collections::HashMap;

use super::rows::AchievementRow;

const LEGENDARY_TOP_N: usize = 3;

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
    let mut rated: Vec<(String, f32)> = achievements
        .iter()
        .filter_map(|r| r.rarity_percent.map(|p| (r.data.id.clone(), p)))
        .collect();

    if rated.is_empty() {
        return HashMap::new();
    }

    rated.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });

    let total = rated.len();
    let mut legendary_n = total.min(LEGENDARY_TOP_N);
    if legendary_n > 0 && legendary_n < total {
        let threshold = rated[legendary_n - 1].1;
        while legendary_n < total && (rated[legendary_n].1 - threshold).abs() < 0.001 {
            legendary_n += 1;
        }
    }
    let remaining = total - legendary_n;

    let mythical_n = (remaining as f32 * 0.10).round() as usize;
    let rare_n = (remaining as f32 * 0.15).round() as usize;
    let uncommon_n = (remaining as f32 * 0.25).round() as usize;
    let common_n = remaining
        .saturating_sub(mythical_n)
        .saturating_sub(rare_n)
        .saturating_sub(uncommon_n);

    let mut map = HashMap::with_capacity(total);

    let mut idx = 0;

    for (id, _) in &rated[idx..idx + legendary_n] {
        map.insert(id.clone(), RarityTier::Legendary);
    }
    idx += legendary_n;

    for (id, _) in &rated[idx..idx + mythical_n] {
        map.insert(id.clone(), RarityTier::Mythical);
    }
    idx += mythical_n;

    for (id, _) in &rated[idx..idx + rare_n] {
        map.insert(id.clone(), RarityTier::Rare);
    }
    idx += rare_n;

    for (id, _) in &rated[idx..idx + uncommon_n] {
        map.insert(id.clone(), RarityTier::Uncommon);
    }
    idx += uncommon_n;

    for (id, _) in &rated[idx..idx + common_n] {
        map.insert(id.clone(), RarityTier::Common);
    }

    map
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
                let pct = (i as f32 / (count - 1).max(1) as f32) * 100.0;
                make_achievement_row(&format!("a{i}"), Some(pct))
            })
            .collect()
    }

    fn count_legendary(map: &HashMap<String, RarityTier>) -> usize {
        map.values()
            .filter(|t| **t == RarityTier::Legendary)
            .count()
    }

    #[test]
    fn compute_tier_map_balanced_distribution() {
        let rows = make_linear_rows(100);
        let map = compute_tier_map(&rows);

        let legendary = map
            .values()
            .filter(|&&t| t == RarityTier::Legendary)
            .count();
        let mythical = map.values().filter(|&&t| t == RarityTier::Mythical).count();
        let rare = map.values().filter(|&&t| t == RarityTier::Rare).count();
        let uncommon = map.values().filter(|&&t| t == RarityTier::Uncommon).count();
        let common = map.values().filter(|&&t| t == RarityTier::Common).count();

        assert_eq!(legendary, 3, "top 3 lowest -> Legendary");
        assert_eq!(mythical, 10, "~10% of remaining 97 = 10 (rounded)");
        assert_eq!(rare, 15, "~15% of remaining 97 = 15 (rounded)");
        assert_eq!(uncommon, 24, "~25% of remaining 97 = 24 (rounded)");
        assert_eq!(
            common + legendary + mythical + rare + uncommon,
            100,
            "total must sum to 100"
        );
        assert!(
            (45..=50).contains(&common),
            "common gets remainder: {common}"
        );
    }

    #[test]
    fn compute_tier_map_skewed_low_distribution() {
        let rows: Vec<AchievementRow> = (0..100)
            .map(|i| make_achievement_row(&format!("a{i}"), Some(i as f32 * 0.1)))
            .collect();
        let map = compute_tier_map(&rows);

        let legendary = map
            .values()
            .filter(|&&t| t == RarityTier::Legendary)
            .count();
        let mythical = map.values().filter(|&&t| t == RarityTier::Mythical).count();
        let rare = map.values().filter(|&&t| t == RarityTier::Rare).count();
        let uncommon = map.values().filter(|&&t| t == RarityTier::Uncommon).count();
        let common = map.values().filter(|&&t| t == RarityTier::Common).count();

        assert_eq!(legendary, 3);
        assert!(
            mythical > 0,
            "Mythical tier must not be empty even with all-low percents"
        );
        assert!(common > 0, "Common must exist");
        assert_eq!(legendary + mythical + rare + uncommon + common, 100);
    }

    #[test]
    fn compute_tier_map_handles_fewer_than_3_total() {
        let rows = vec![
            make_achievement_row("a1", Some(2.0)),
            make_achievement_row("a2", Some(5.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(map.len(), 2);
        assert_eq!(map["a1"], RarityTier::Legendary);
        assert_eq!(map["a2"], RarityTier::Legendary);
    }

    #[test]
    fn compute_tier_map_excludes_unrated() {
        let rows = vec![
            make_achievement_row("rated1", Some(10.0)),
            make_achievement_row("rated2", Some(50.0)),
            make_achievement_row("rated3", Some(90.0)),
            make_achievement_row("unrated", None),
        ];
        let map = compute_tier_map(&rows);
        assert!(
            !map.contains_key("unrated"),
            "None rows must not appear in map"
        );
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn legendary_no_tie_at_third_position() {
        let rows = vec![
            make_achievement_row("a", Some(1.0)),
            make_achievement_row("b", Some(1.0)),
            make_achievement_row("c", Some(2.0)),
            make_achievement_row("d", Some(3.0)),
            make_achievement_row("e", Some(3.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(count_legendary(&map), 3);
        assert_eq!(map.get("a").copied(), Some(RarityTier::Legendary));
        assert_eq!(map.get("b").copied(), Some(RarityTier::Legendary));
        assert_eq!(map.get("c").copied(), Some(RarityTier::Legendary));
        assert_ne!(map.get("d").copied(), Some(RarityTier::Legendary));
    }

    #[test]
    fn legendary_three_with_same_value_no_extension() {
        let rows = vec![
            make_achievement_row("a", Some(1.0)),
            make_achievement_row("b", Some(1.0)),
            make_achievement_row("c", Some(1.0)),
            make_achievement_row("d", Some(2.0)),
            make_achievement_row("e", Some(3.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(count_legendary(&map), 3);
    }

    #[test]
    fn legendary_extends_when_fourth_matches_third() {
        let rows = vec![
            make_achievement_row("a", Some(1.0)),
            make_achievement_row("b", Some(1.0)),
            make_achievement_row("c", Some(1.0)),
            make_achievement_row("d", Some(1.0)),
            make_achievement_row("e", Some(2.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(count_legendary(&map), 4);
        for id in ["a", "b", "c", "d"] {
            assert_eq!(
                map.get(id).copied(),
                Some(RarityTier::Legendary),
                "{id} should be Legendary"
            );
        }
    }

    #[test]
    fn legendary_extends_through_multiple_ties() {
        let rows = vec![
            make_achievement_row("a", Some(1.0)),
            make_achievement_row("b", Some(2.0)),
            make_achievement_row("c", Some(3.0)),
            make_achievement_row("d", Some(3.0)),
            make_achievement_row("e", Some(3.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(count_legendary(&map), 5);
    }

    #[test]
    fn legendary_fewer_than_three_rated() {
        let rows = vec![
            make_achievement_row("a", Some(1.0)),
            make_achievement_row("b", Some(2.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(count_legendary(&map), 2);
    }

    #[test]
    fn legendary_zero_rated_returns_empty() {
        let rows = vec![
            make_achievement_row("a", None),
            make_achievement_row("b", None),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(count_legendary(&map), 0);
        assert!(map.is_empty());
    }

    #[test]
    fn legendary_all_same_percent() {
        let rows = vec![
            make_achievement_row("a", Some(1.0)),
            make_achievement_row("b", Some(1.0)),
            make_achievement_row("c", Some(1.0)),
            make_achievement_row("d", Some(1.0)),
            make_achievement_row("e", Some(1.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(count_legendary(&map), 5);
    }
}
