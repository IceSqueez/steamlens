use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct AchievementData {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub is_hidden: bool,
    pub is_achieved: bool,
    pub unlock_time: Option<u32>,
    pub permission: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum StatValue {
    Int(i32),
    Float(f32),
}

impl StatValue {
    pub fn to_edit_string(self) -> String {
        match self {
            StatValue::Int(v) => v.to_string(),
            StatValue::Float(v) => format!("{v:.2}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatData {
    pub id: String,
    pub display_name: String,
    pub value: StatValue,
    pub original_value: StatValue,
    pub max_value: Option<u64>,
    #[allow(dead_code)]
    pub min_value: Option<i64>,
    #[allow(dead_code)]
    pub default_value: Option<i64>,
    pub is_increment_only: bool,
    pub permission: u32,
}

#[derive(Debug, Clone)]
pub struct AchievementRow {
    pub data: AchievementData,
    pub is_dirty: bool,
}

impl AchievementRow {
    pub fn from_data(data: AchievementData) -> Self {
        Self {
            data,
            is_dirty: false,
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

pub fn visible_achievement_ids<'a>(
    achievements: &'a [AchievementRow],
    filter: AchievementFilter,
    search: &str,
) -> HashSet<&'a str> {
    let query = search.to_lowercase();
    achievements
        .iter()
        .filter(|row| {
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
        .map(|row| row.data.id.as_str())
        .collect()
}
