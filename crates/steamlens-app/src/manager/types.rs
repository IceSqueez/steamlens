use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

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

impl AchievementRow {
    pub fn from_data(data: AchievementData) -> Self {
        Self {
            data,
            is_dirty: false,
            revealed: false,
            appeared: false,
            card_opacity: 0.0,
            rarity_percent: None,
        }
    }

    pub fn effective_achieved(&self) -> bool {
        if self.is_dirty {
            !self.data.is_achieved
        } else {
            self.data.is_achieved
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

impl StatRow {
    pub fn from_data(data: StatData) -> Self {
        let edit_text = data.value.to_edit_string();
        Self {
            data,
            edit_text,
            edit_error: None,
            is_dirty: false,
        }
    }

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

#[derive(Debug, Clone, Copy, PartialEq)]
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

    pub const ALL: &'static [AchievementFilter] = &[
        AchievementFilter::All,
        AchievementFilter::Unlocked,
        AchievementFilter::Locked,
    ];
}

impl std::fmt::Display for AchievementFilter {
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

fn sort_for_display(rows: Vec<&AchievementRow>) -> Vec<&AchievementRow> {
    let mut sorted = rows;
    sorted.sort_by(|a, b| {
        let ga = display_group(a);
        let gb = display_group(b);
        ga.cmp(&gb).then_with(|| {
            a.data
                .display_name
                .to_lowercase()
                .cmp(&b.data.display_name.to_lowercase())
        })
    });
    sorted
}

const LEGENDARY_TOP_N: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn classify(percent: f32, is_legendary: bool) -> Self {
        if is_legendary {
            return Self::Legendary;
        }
        if percent < 25.0 {
            Self::Mythical
        } else if percent < 50.0 {
            Self::Rare
        } else if percent < 75.0 {
            Self::Uncommon
        } else {
            Self::Common
        }
    }
}

pub fn top_3_legendary_ids(achievements: &[AchievementRow]) -> HashSet<String> {
    let mut candidates: Vec<(&str, f32)> = achievements
        .iter()
        .filter_map(|r| r.rarity_percent.map(|p| (r.data.id.as_str(), p)))
        .collect();
    candidates.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(Ordering::Equal)
            .then(a.0.cmp(b.0))
    });
    candidates
        .into_iter()
        .take(LEGENDARY_TOP_N)
        .map(|(id, _)| id.to_owned())
        .collect()
}

pub fn visible_achievement_ids<'a>(
    achievements: &'a [AchievementRow],
    filter: AchievementFilter,
    search: &str,
) -> Vec<&'a str> {
    let query = search.to_lowercase();
    let filtered: Vec<&AchievementRow> = achievements
        .iter()
        .filter(|row| {
            if !row.appeared {
                return false;
            }
            let effective = row.effective_achieved();
            let filter_ok = match filter {
                AchievementFilter::All => true,
                AchievementFilter::Unlocked => effective,
                AchievementFilter::Locked => !effective,
            };
            let search_ok = query.is_empty()
                || row.data.display_name.to_lowercase().contains(&query)
                || row.data.description.to_lowercase().contains(&query)
                || row.data.id.to_lowercase().contains(&query);
            filter_ok && search_ok
        })
        .collect();
    sort_for_display(filtered)
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

    #[test]
    fn top_3_legendary_returns_empty_when_no_data() {
        let rows = vec![
            make_row("a1", None),
            make_row("a2", None),
            make_row("a3", None),
        ];
        assert!(top_3_legendary_ids(&rows).is_empty());
    }

    #[test]
    fn top_3_legendary_returns_all_when_fewer_than_3() {
        let rows = vec![
            make_row("a1", Some(2.0)),
            make_row("a2", Some(7.5)),
            make_row("a3", None),
        ];
        let ids = top_3_legendary_ids(&rows);
        assert_eq!(ids.len(), 2);
        assert!(ids.contains("a1"));
        assert!(ids.contains("a2"));
    }

    #[test]
    fn top_3_legendary_takes_lowest_3() {
        let rows = vec![
            make_row("a1", Some(0.5)),
            make_row("a2", Some(1.2)),
            make_row("a3", Some(3.0)),
            make_row("a4", Some(5.5)),
            make_row("a5", Some(8.1)),
        ];
        let ids = top_3_legendary_ids(&rows);
        assert_eq!(ids.len(), 3);
        assert!(ids.contains("a1"));
        assert!(ids.contains("a2"));
        assert!(ids.contains("a3"));
        assert!(!ids.contains("a4"));
        assert!(!ids.contains("a5"));
    }

    #[test]
    fn top_3_legendary_picks_globally_lowest_regardless_of_threshold() {
        let rows = vec![make_row("a1", Some(62.0)), make_row("a2", Some(80.0))];
        let ids = top_3_legendary_ids(&rows);
        assert_eq!(ids.len(), 2);
        assert!(ids.contains("a1"));
        assert!(ids.contains("a2"));
    }

    #[test]
    fn classify_boundaries() {
        assert_eq!(RarityTier::classify(100.0, false), RarityTier::Common);
        assert_eq!(RarityTier::classify(75.0, false), RarityTier::Common);
        assert_eq!(RarityTier::classify(74.9, false), RarityTier::Uncommon);
        assert_eq!(RarityTier::classify(50.0, false), RarityTier::Uncommon);
        assert_eq!(RarityTier::classify(49.9, false), RarityTier::Rare);
        assert_eq!(RarityTier::classify(25.0, false), RarityTier::Rare);
        assert_eq!(RarityTier::classify(24.9, false), RarityTier::Mythical);
        assert_eq!(RarityTier::classify(0.0, false), RarityTier::Mythical);
    }

    #[test]
    fn classify_legendary_priority_over_mythical() {
        assert_eq!(RarityTier::classify(0.5, true), RarityTier::Legendary);
        assert_eq!(RarityTier::classify(0.0, true), RarityTier::Legendary);
        assert_eq!(RarityTier::classify(80.0, true), RarityTier::Legendary);
    }
}
