use std::cmp::Ordering;
use std::collections::HashMap;

pub use steamlens_core::{AchievementData, StatData, StatValue};

#[derive(Debug, Clone)]
pub struct AchievementRow {
    pub data: AchievementData,
    pub is_dirty: bool,
    pub revealed: bool,
    pub appeared: bool,
    pub card_opacity: f32,
    pub rarity_percent: Option<f32>,
}

impl From<AchievementData> for AchievementRow {
    fn from(data: AchievementData) -> Self {
        Self {
            data,
            is_dirty: false,
            revealed: false,
            appeared: false,
            card_opacity: 0.0,
            rarity_percent: None,
        }
    }
}

impl AchievementRow {
    pub fn effective_achieved(&self) -> bool {
        if self.is_dirty {
            !self.data.is_achieved
        } else {
            self.data.is_achieved
        }
    }

    pub fn is_spoiler_hidden(&self) -> bool {
        self.data.is_hidden && !self.data.is_achieved && !self.revealed
    }

    pub fn status_label(&self) -> &'static str {
        if self.data.permission != 0 {
            "Protected"
        } else if self.is_dirty {
            "Pending"
        } else if self.is_spoiler_hidden() {
            "Hidden"
        } else if self.effective_achieved() {
            "Unlocked"
        } else {
            "Locked"
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatRow {
    pub data: StatData,
    pub edit_text: String,
    pub edit_error: Option<String>,
    pub is_dirty: bool,
}

impl From<StatData> for StatRow {
    fn from(data: StatData) -> Self {
        let edit_text = data.value.to_edit_string();
        Self {
            data,
            edit_text,
            edit_error: None,
            is_dirty: false,
        }
    }
}

impl StatRow {
    pub fn validate_and_parse(&mut self) {
        let trimmed = self.edit_text.trim();
        match self.data.value {
            StatValue::Int(_) => match trimmed.parse::<i32>() {
                Ok(v) => {
                    let original_int = match self.data.original_value {
                        StatValue::Int(orig) => orig,
                        StatValue::Float(orig) => orig as i32,
                    };
                    if self.data.is_increment_only && v < original_int {
                        self.edit_error = Some(format!(
                            "Increment-only: value cannot be less than {original_int}"
                        ));
                    } else {
                        self.edit_error = None;
                        self.data.value = StatValue::Int(v);
                        self.is_dirty = v != original_int;
                    }
                }
                Err(_) => {
                    self.edit_error = Some("Must be a whole number".to_owned());
                }
            },
            StatValue::Float(_) => match trimmed.parse::<f32>() {
                Ok(v) => {
                    let original_float = match self.data.original_value {
                        StatValue::Float(orig) => orig,
                        StatValue::Int(orig) => orig as f32,
                    };
                    if self.data.is_increment_only && v < original_float {
                        self.edit_error = Some(format!(
                            "Increment-only: value cannot be less than {original_float:.2}"
                        ));
                    } else {
                        self.edit_error = None;
                        self.data.value = StatValue::Float(v);
                        self.is_dirty = (v - original_float).abs() > f32::EPSILON;
                    }
                }
                Err(_) => {
                    self.edit_error = Some("Must be a decimal number".to_owned());
                }
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AchievementFilter {
    All,
    Unlocked,
    Locked,
}

impl AchievementFilter {
    pub fn label(self) -> &'static str {
        match self {
            AchievementFilter::All => "All",
            AchievementFilter::Unlocked => "Unlocked",
            AchievementFilter::Locked => "Locked",
        }
    }
}

impl std::fmt::Display for AchievementFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AchievementSort {
    UnlockChance,
    RarityAndName,
    Name,
}

impl AchievementSort {
    pub fn label(self) -> &'static str {
        match self {
            AchievementSort::UnlockChance => "Unlock Chance",
            AchievementSort::RarityAndName => "Rarity & Name",
            AchievementSort::Name => "Name",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            AchievementSort::UnlockChance => "UC",
            AchievementSort::RarityAndName => "R&N",
            AchievementSort::Name => "A\u{2013}Z",
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            AchievementSort::UnlockChance => "Sort by unlock chance (rarest first)",
            AchievementSort::RarityAndName => "Sort by rarity tier, then name",
            AchievementSort::Name => "Sort by name (A to Z)",
        }
    }

    pub const ALL: &'static [AchievementSort] = &[
        AchievementSort::UnlockChance,
        AchievementSort::RarityAndName,
        AchievementSort::Name,
    ];
}

impl std::fmt::Display for AchievementSort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveTab {
    Achievements,
    Stats,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResetScope {
    Pending,
    StatsOnly,
    StatsAndAchievements,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BannerKind {
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Banner {
    pub kind: BannerKind,
    pub message: String,
    pub dismissible: bool,
}

#[derive(Debug, Clone)]
pub enum BulkOp {
    Unlock,
    Lock,
    Invert,
}

pub struct ApplyPayload {
    pub achievements_to_set: Vec<String>,
    pub achievements_to_clear: Vec<String>,
    pub stats_int: HashMap<String, i32>,
    pub stats_float: HashMap<String, f32>,
}

pub fn build_apply_payload(achievements: &[AchievementRow], stats: &[StatRow]) -> ApplyPayload {
    let mut to_set = Vec::new();
    let mut to_clear = Vec::new();
    let mut stats_int = HashMap::new();
    let mut stats_float = HashMap::new();

    for row in achievements {
        if row.is_dirty {
            if row.effective_achieved() {
                to_set.push(row.data.id.clone());
            } else {
                to_clear.push(row.data.id.clone());
            }
        }
    }

    for row in stats {
        if row.is_dirty && row.edit_error.is_none() {
            match row.data.value {
                StatValue::Int(v) => {
                    stats_int.insert(row.data.id.clone(), v);
                }
                StatValue::Float(v) => {
                    stats_float.insert(row.data.id.clone(), v);
                }
            }
        }
    }

    ApplyPayload {
        achievements_to_set: to_set,
        achievements_to_clear: to_clear,
        stats_int,
        stats_float,
    }
}

pub fn dirty_count(achievements: &[AchievementRow], stats: &[StatRow]) -> usize {
    let ach = achievements.iter().filter(|r| r.is_dirty).count();
    let st = stats
        .iter()
        .filter(|r| r.is_dirty && r.edit_error.is_none())
        .count();
    ach + st
}

pub fn has_stat_errors(stats: &[StatRow]) -> bool {
    stats.iter().any(|r| r.edit_error.is_some())
}

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

fn sort_for_display<'a>(
    rows: Vec<&'a AchievementRow>,
    tier_map: &HashMap<String, RarityTier>,
    sort: AchievementSort,
) -> Vec<&'a AchievementRow> {
    let mut sorted = rows;
    match sort {
        AchievementSort::UnlockChance => {
            sorted.sort_by(|a, b| {
                display_group(a)
                    .cmp(&display_group(b))
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
            sorted.sort_by(|a, b| {
                display_group(a)
                    .cmp(&display_group(b))
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
            sorted.sort_by(|a, b| {
                display_group(a).cmp(&display_group(b)).then_with(|| {
                    a.data
                        .display_name
                        .to_lowercase()
                        .cmp(&b.data.display_name.to_lowercase())
                })
            });
        }
    }
    sorted
}

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

pub fn visible_achievement_ids<'a>(
    achievements: &'a [AchievementRow],
    filter: AchievementFilter,
    search: &str,
    sort: AchievementSort,
    rarity_tier_set: &std::collections::HashSet<RarityTier>,
    include_hidden: bool,
) -> Vec<&'a str> {
    let tier_map = compute_tier_map(achievements);
    let query = search.to_lowercase();
    let filtered: Vec<&AchievementRow> = achievements
        .iter()
        .filter(|row| {
            if !row.appeared {
                return false;
            }
            let is_spoiler = row.is_spoiler_hidden();
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
                let hidden_match = is_spoiler && include_hidden;
                tier_match || hidden_match
            };
            filter_ok && search_ok && rarity_ok
        })
        .collect();
    sort_for_display(filtered, &tier_map, sort)
        .into_iter()
        .map(|row| row.data.id.as_str())
        .collect()
}

#[cfg(test)]
mod rarity_tests {
    use super::*;
    use steamlens_core::AchievementData;

    fn make_row(id: &str, rarity: Option<f32>) -> AchievementRow {
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
            revealed: false,
            appeared: false,
            card_opacity: 0.0,
            rarity_percent: rarity,
        }
    }

    fn make_appeared(id: &str, rarity: Option<f32>) -> AchievementRow {
        AchievementRow {
            appeared: true,
            ..make_row(id, rarity)
        }
    }

    fn make_linear_rows(count: usize) -> Vec<AchievementRow> {
        (0..count)
            .map(|i| {
                let pct = (i as f32 / (count - 1).max(1) as f32) * 100.0;
                make_row(&format!("a{i}"), Some(pct))
            })
            .collect()
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
            .map(|i| make_row(&format!("a{i}"), Some(i as f32 * 0.1)))
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
        let rows = vec![make_row("a1", Some(2.0)), make_row("a2", Some(5.0))];
        let map = compute_tier_map(&rows);
        assert_eq!(map.len(), 2);
        assert_eq!(map["a1"], RarityTier::Legendary);
        assert_eq!(map["a2"], RarityTier::Legendary);
    }

    #[test]
    fn compute_tier_map_excludes_unrated() {
        let rows = vec![
            make_row("rated1", Some(10.0)),
            make_row("rated2", Some(50.0)),
            make_row("rated3", Some(90.0)),
            make_row("unrated", None),
        ];
        let map = compute_tier_map(&rows);
        assert!(
            !map.contains_key("unrated"),
            "None rows must not appear in map"
        );
        assert_eq!(map.len(), 3);
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
                r.appeared = true;
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
            appeared: true,
            is_dirty: true,
            ..make_row("dirty_unlock", None)
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
            appeared: true,
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
            revealed: false,
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

    fn make_hidden_row(
        id: &str,
        is_achieved: bool,
        revealed: bool,
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
            revealed,
            appeared: true,
            card_opacity: 1.0,
            rarity_percent: rarity,
        }
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

    fn count_legendary(map: &HashMap<String, RarityTier>) -> usize {
        map.values()
            .filter(|t| **t == RarityTier::Legendary)
            .count()
    }

    #[test]
    fn legendary_no_tie_at_third_position() {
        let rows = vec![
            make_row("a", Some(1.0)),
            make_row("b", Some(1.0)),
            make_row("c", Some(2.0)),
            make_row("d", Some(3.0)),
            make_row("e", Some(3.0)),
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
            make_row("a", Some(1.0)),
            make_row("b", Some(1.0)),
            make_row("c", Some(1.0)),
            make_row("d", Some(2.0)),
            make_row("e", Some(3.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(count_legendary(&map), 3);
    }

    #[test]
    fn legendary_extends_when_fourth_matches_third() {
        let rows = vec![
            make_row("a", Some(1.0)),
            make_row("b", Some(1.0)),
            make_row("c", Some(1.0)),
            make_row("d", Some(1.0)),
            make_row("e", Some(2.0)),
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
            make_row("a", Some(1.0)),
            make_row("b", Some(2.0)),
            make_row("c", Some(3.0)),
            make_row("d", Some(3.0)),
            make_row("e", Some(3.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(count_legendary(&map), 5);
    }

    #[test]
    fn legendary_fewer_than_three_rated() {
        let rows = vec![make_row("a", Some(1.0)), make_row("b", Some(2.0))];
        let map = compute_tier_map(&rows);
        assert_eq!(count_legendary(&map), 2);
    }

    #[test]
    fn legendary_zero_rated_returns_empty() {
        let rows = vec![make_row("a", None), make_row("b", None)];
        let map = compute_tier_map(&rows);
        assert_eq!(count_legendary(&map), 0);
        assert!(map.is_empty());
    }

    #[test]
    fn legendary_all_same_percent() {
        let rows = vec![
            make_row("a", Some(1.0)),
            make_row("b", Some(1.0)),
            make_row("c", Some(1.0)),
            make_row("d", Some(1.0)),
            make_row("e", Some(1.0)),
        ];
        let map = compute_tier_map(&rows);
        assert_eq!(count_legendary(&map), 5);
    }
}

#[cfg(test)]
mod behavior_tests {
    use super::*;
    use steamlens_core::AchievementData;

    fn make_row(
        is_hidden: bool,
        is_achieved: bool,
        revealed: bool,
        is_dirty: bool,
        permission: u32,
    ) -> AchievementRow {
        AchievementRow {
            data: AchievementData {
                id: "test".to_owned(),
                display_name: "Test".to_owned(),
                description: String::new(),
                is_achieved,
                unlock_time: None,
                is_hidden,
                permission,
                icon: None,
            },
            is_dirty,
            revealed,
            appeared: true,
            card_opacity: 1.0,
            rarity_percent: None,
        }
    }

    #[test]
    fn spoiler_hidden_persisted_unlock_overrides_dirty() {
        let row = make_row(true, true, false, true, 0);
        assert!(
            !row.is_spoiler_hidden(),
            "already-unlocked secret stays visible even when dirty (pending to lock)"
        );
    }

    #[test]
    fn spoiler_hidden_clean_locked_hidden() {
        let row = make_row(true, false, false, false, 0);
        assert!(
            row.is_spoiler_hidden(),
            "locked+hidden+not-revealed = spoiler"
        );
    }

    #[test]
    fn spoiler_hidden_after_reveal_click() {
        let row = make_row(true, false, true, false, 0);
        assert!(
            !row.is_spoiler_hidden(),
            "user clicked reveal: spoiler lifted"
        );
    }

    #[test]
    fn spoiler_hidden_non_hidden_achievement() {
        let row = make_row(false, false, false, false, 0);
        assert!(
            !row.is_spoiler_hidden(),
            "non-hidden achievement is never a spoiler"
        );
    }

    #[test]
    fn spoiler_hidden_dirty_locked_does_not_unspoil() {
        let row = make_row(true, false, false, true, 0);
        assert!(
            row.is_spoiler_hidden(),
            "pending-unlock on hidden card: still a spoiler until Apply commits"
        );
    }

    #[test]
    fn status_label_protected_overrides_all() {
        let row = make_row(false, false, false, true, 1);
        assert_eq!(row.status_label(), "Protected");
    }

    #[test]
    fn status_label_pending_overrides_hidden() {
        let row = make_row(true, true, false, true, 0);
        assert_eq!(
            row.status_label(),
            "Pending",
            "dirty wins over Hidden when achievement was already unlocked"
        );
    }

    #[test]
    fn status_label_pending_on_hidden_spoiler() {
        let row = make_row(true, false, false, true, 0);
        assert_eq!(
            row.status_label(),
            "Pending",
            "dirty wins over Hidden even on spoiler card so progress is visible"
        );
    }

    #[test]
    fn status_label_hidden_when_clean_and_secret() {
        let row = make_row(true, false, false, false, 0);
        assert_eq!(row.status_label(), "Hidden");
    }

    #[test]
    fn status_label_unlocked_persisted() {
        let row = make_row(false, true, false, false, 0);
        assert_eq!(row.status_label(), "Unlocked");
    }

    #[test]
    fn status_label_locked_default() {
        let row = make_row(false, false, false, false, 0);
        assert_eq!(row.status_label(), "Locked");
    }

    #[test]
    fn status_label_unlocked_after_revealed_secret() {
        let row = make_row(true, true, false, false, 0);
        assert_eq!(
            row.status_label(),
            "Unlocked",
            "secret naturally revealed by being earned shows Unlocked"
        );
    }
}
