#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AchievementIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AchievementData {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub is_hidden: bool,
    pub is_achieved: bool,
    pub unlock_time: Option<u32>,
    pub permission: u32,
    pub icon: Option<AchievementIcon>,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatData {
    pub id: String,
    pub display_name: String,
    pub value: StatValue,
    pub original_value: StatValue,
    pub max_value: Option<u64>,
    pub min_value: Option<i64>,
    pub default_value: Option<i64>,
    pub is_increment_only: bool,
    pub permission: u32,
}
