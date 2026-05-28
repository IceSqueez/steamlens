use std::collections::HashMap;

use super::rows::{AchievementRow, StatRow, StatValue};

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
