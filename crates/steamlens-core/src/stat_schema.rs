use steamlens_vdf::Value;

use crate::error::SteamError;
use crate::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatKind {
    Int,
    Float,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatDescriptor {
    pub name: String,
    pub display_name: Option<String>,
    pub kind: StatKind,
    pub max_value: Option<u64>,
    pub default_value: Option<i64>,
    pub min_value: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(
    dead_code,
    reason = "consumed once switch to CDN-based icon path lands"
)]
pub struct AchievementIconRefs {
    pub icon: Option<String>,
    pub icon_gray: Option<String>,
}

pub(crate) fn load(app_id: u32) -> Result<Vec<StatDescriptor>, SteamError> {
    let bytes = match read_schema_bytes(app_id) {
        Some(b) => b,
        None => return Ok(Vec::new()),
    };
    let root =
        steamlens_vdf::parse(&bytes).map_err(|source| SteamError::SchemaParseError { source })?;
    Ok(extract_stats(&root, app_id))
}

#[allow(
    dead_code,
    reason = "consumed once switch to CDN-based icon path lands"
)]
pub(crate) fn load_achievement_icons(
    app_id: u32,
) -> Result<std::collections::HashMap<String, AchievementIconRefs>, SteamError> {
    let bytes = match read_schema_bytes(app_id) {
        Some(b) => b,
        None => return Ok(std::collections::HashMap::new()),
    };
    let root =
        steamlens_vdf::parse(&bytes).map_err(|source| SteamError::SchemaParseError { source })?;
    Ok(extract_achievement_icons(&root, app_id))
}

fn read_schema_bytes(app_id: u32) -> Option<Vec<u8>> {
    let root = paths::steam_install_root_candidates().into_iter().next()?;
    let path = paths::appcache_stats_dir(&root).join(format!("UserGameStatsSchema_{app_id}.bin"));
    std::fs::read(&path).ok()
}

fn extract_stats(root: &Value, app_id: u32) -> Vec<StatDescriptor> {
    let path = format!("{app_id}/stats");
    let stats_section = match root.get(&path) {
        Some(v) => v,
        None => return Vec::new(),
    };

    let pairs = match stats_section.as_section() {
        Some(p) => p,
        None => return Vec::new(),
    };

    let mut out = Vec::new();

    for stat_pair in pairs {
        let children = match stat_pair.value.as_section() {
            Some(c) => c,
            None => continue,
        };

        let type_str = children
            .iter()
            .find(|p| p.key == "type")
            .and_then(|p| p.value.as_str())
            .unwrap_or("");

        let kind = match type_str {
            "INT" => StatKind::Int,
            "FLOAT" => StatKind::Float,
            _ => continue,
        };

        let name = match children
            .iter()
            .find(|p| p.key == "name")
            .and_then(|p| p.value.as_str())
        {
            Some(n) if !n.is_empty() => n.to_owned(),
            _ => continue,
        };

        let max_value = children
            .iter()
            .find(|p| p.key == "max")
            .and_then(|p| p.value.as_i32())
            .map(|v| v as u64);

        let default_value = children
            .iter()
            .find(|p| p.key == "default")
            .and_then(|p| p.value.as_i32())
            .map(|v| v as i64);

        let min_value = children
            .iter()
            .find(|p| p.key == "min")
            .and_then(|p| p.value.as_i32())
            .map(|v| v as i64);

        let display_name = children
            .iter()
            .find(|p| p.key == "display")
            .and_then(|p| p.value.as_section())
            .and_then(|display_children| {
                display_children
                    .iter()
                    .find(|p| p.key == "name")
                    .and_then(|p| p.value.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_owned())
            });

        out.push(StatDescriptor {
            name,
            display_name,
            kind,
            max_value,
            default_value,
            min_value,
        });
    }

    out
}

#[allow(
    dead_code,
    reason = "consumed once switch to CDN-based icon path lands"
)]
fn extract_achievement_icons(
    root: &Value,
    app_id: u32,
) -> std::collections::HashMap<String, AchievementIconRefs> {
    let mut out = std::collections::HashMap::new();
    let stats_section = match root.get(&format!("{app_id}/stats")) {
        Some(v) => v,
        None => return out,
    };
    let pairs = match stats_section.as_section() {
        Some(p) => p,
        None => return out,
    };

    for stat_pair in pairs {
        let children = match stat_pair.value.as_section() {
            Some(c) => c,
            None => continue,
        };
        let type_str = children
            .iter()
            .find(|p| p.key == "type")
            .and_then(|p| p.value.as_str())
            .unwrap_or("");
        if !type_str.eq_ignore_ascii_case("ACHIEVEMENTS") {
            continue;
        }
        let bits = match children
            .iter()
            .find(|p| p.key == "bits")
            .and_then(|p| p.value.as_section())
        {
            Some(b) => b,
            None => continue,
        };
        for ach_pair in bits {
            let ach_children = match ach_pair.value.as_section() {
                Some(c) => c,
                None => continue,
            };
            let name = match ach_children
                .iter()
                .find(|p| p.key == "name")
                .and_then(|p| p.value.as_str())
            {
                Some(n) if !n.is_empty() => n.to_owned(),
                _ => continue,
            };
            let display = ach_children
                .iter()
                .find(|p| p.key == "display")
                .and_then(|p| p.value.as_section());
            let (icon, icon_gray) = match display {
                Some(d) => {
                    let icon = d
                        .iter()
                        .find(|p| p.key == "icon")
                        .and_then(|p| p.value.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_owned());
                    let icon_gray = d
                        .iter()
                        .find(|p| p.key == "icon_gray")
                        .and_then(|p| p.value.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_owned());
                    (icon, icon_gray)
                }
                None => (None, None),
            };
            out.insert(name, AchievementIconRefs { icon, icon_gray });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use steamlens_vdf::{KeyValuePair, Value};

    use super::*;

    fn make_stat_entry(index: &str, type_str: &str, name: &str) -> KeyValuePair {
        KeyValuePair {
            key: index.to_owned(),
            value: Value::Section(vec![
                KeyValuePair {
                    key: "type".to_owned(),
                    value: Value::String(type_str.to_owned()),
                },
                KeyValuePair {
                    key: "name".to_owned(),
                    value: Value::String(name.to_owned()),
                },
            ]),
        }
    }

    fn make_schema_root(app_id: u32, stat_entries: Vec<KeyValuePair>) -> Value {
        Value::Section(vec![KeyValuePair {
            key: app_id.to_string(),
            value: Value::Section(vec![KeyValuePair {
                key: "stats".to_owned(),
                value: Value::Section(stat_entries),
            }]),
        }])
    }

    #[test]
    fn extract_int_and_float_skips_achievements() {
        let root = make_schema_root(
            105600,
            vec![
                make_stat_entry("0", "INT", "NumFishCaught"),
                make_stat_entry("1", "FLOAT", "TimeElapsed"),
                make_stat_entry("2", "ACHIEVEMENTS", "AchievBits"),
            ],
        );

        let stats = extract_stats(&root, 105600);
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].name, "NumFishCaught");
        assert_eq!(stats[0].kind, StatKind::Int);
        assert_eq!(stats[1].name, "TimeElapsed");
        assert_eq!(stats[1].kind, StatKind::Float);
    }

    #[test]
    fn extract_optional_fields_present() {
        let root = make_schema_root(
            1,
            vec![KeyValuePair {
                key: "0".to_owned(),
                value: Value::Section(vec![
                    KeyValuePair {
                        key: "type".to_owned(),
                        value: Value::String("INT".to_owned()),
                    },
                    KeyValuePair {
                        key: "name".to_owned(),
                        value: Value::String("kills".to_owned()),
                    },
                    KeyValuePair {
                        key: "max".to_owned(),
                        value: Value::Int32(9999),
                    },
                    KeyValuePair {
                        key: "default".to_owned(),
                        value: Value::Int32(0),
                    },
                    KeyValuePair {
                        key: "min".to_owned(),
                        value: Value::Int32(-1),
                    },
                ]),
            }],
        );

        let stats = extract_stats(&root, 1);
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].max_value, Some(9999));
        assert_eq!(stats[0].default_value, Some(0));
        assert_eq!(stats[0].min_value, Some(-1));
    }

    #[test]
    fn extract_optional_fields_absent() {
        let root = make_schema_root(99, vec![make_stat_entry("0", "INT", "score")]);
        let stats = extract_stats(&root, 99);
        assert_eq!(stats.len(), 1);
        assert!(stats[0].max_value.is_none());
        assert!(stats[0].default_value.is_none());
        assert!(stats[0].min_value.is_none());
    }

    #[test]
    fn extract_missing_app_section_returns_empty() {
        let root = Value::Section(vec![]);
        let stats = extract_stats(&root, 12345);
        assert!(stats.is_empty());
    }

    #[test]
    fn extract_skips_entry_with_empty_name() {
        let root = make_schema_root(
            1,
            vec![
                make_stat_entry("0", "INT", ""),
                make_stat_entry("1", "INT", "valid"),
            ],
        );
        let stats = extract_stats(&root, 1);
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].name, "valid");
    }

    #[test]
    fn load_returns_empty_for_nonexistent_path() {
        let result = load(0xDEAD_BEEF);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    fn make_achievement_entry(api: &str, icon: &str, icon_gray: &str) -> KeyValuePair {
        KeyValuePair {
            key: "0".to_owned(),
            value: Value::Section(vec![
                KeyValuePair {
                    key: "name".to_owned(),
                    value: Value::String(api.to_owned()),
                },
                KeyValuePair {
                    key: "display".to_owned(),
                    value: Value::Section(vec![
                        KeyValuePair {
                            key: "icon".to_owned(),
                            value: Value::String(icon.to_owned()),
                        },
                        KeyValuePair {
                            key: "icon_gray".to_owned(),
                            value: Value::String(icon_gray.to_owned()),
                        },
                    ]),
                },
            ]),
        }
    }

    fn schema_with_achievements_section(app_id: u32, bits: Vec<KeyValuePair>) -> Value {
        Value::Section(vec![KeyValuePair {
            key: app_id.to_string(),
            value: Value::Section(vec![KeyValuePair {
                key: "stats".to_owned(),
                value: Value::Section(vec![KeyValuePair {
                    key: "0".to_owned(),
                    value: Value::Section(vec![
                        KeyValuePair {
                            key: "type".to_owned(),
                            value: Value::String("ACHIEVEMENTS".to_owned()),
                        },
                        KeyValuePair {
                            key: "bits".to_owned(),
                            value: Value::Section(bits),
                        },
                    ]),
                }]),
            }]),
        }])
    }

    #[test]
    fn extract_icons_semantic_filename() {
        let root = schema_with_achievements_section(
            220,
            vec![make_achievement_entry(
                "HL2_HIT_CANCOP_WITHCAN",
                "hl2_hit_cancop_withcan.jpg",
                "hl2_hit_cancop_withcan_bw.jpg",
            )],
        );
        let icons = extract_achievement_icons(&root, 220);
        let refs = icons.get("HL2_HIT_CANCOP_WITHCAN").expect("present");
        assert_eq!(refs.icon.as_deref(), Some("hl2_hit_cancop_withcan.jpg"));
        assert_eq!(
            refs.icon_gray.as_deref(),
            Some("hl2_hit_cancop_withcan_bw.jpg")
        );
    }

    #[test]
    fn extract_icons_sha1_filename() {
        let root = schema_with_achievements_section(
            286690,
            vec![make_achievement_entry(
                "ACH_MODERN",
                "9ab9949c908b669ecdec318dd9303f2d8c3f2314.jpg",
                "955747179aab607d0be0e276cd824f46b2034263.jpg",
            )],
        );
        let icons = extract_achievement_icons(&root, 286690);
        let refs = icons.get("ACH_MODERN").expect("present");
        assert!(refs.icon.as_deref().unwrap().ends_with(".jpg"));
        assert_eq!(refs.icon.as_deref().unwrap().len(), 44);
    }

    #[test]
    fn extract_icons_missing_display_returns_none_fields() {
        let root = schema_with_achievements_section(
            1,
            vec![KeyValuePair {
                key: "0".to_owned(),
                value: Value::Section(vec![KeyValuePair {
                    key: "name".to_owned(),
                    value: Value::String("NO_DISPLAY".to_owned()),
                }]),
            }],
        );
        let icons = extract_achievement_icons(&root, 1);
        let refs = icons.get("NO_DISPLAY").expect("present");
        assert!(refs.icon.is_none());
        assert!(refs.icon_gray.is_none());
    }

    #[test]
    fn extract_icons_skips_empty_string_fields() {
        let root = schema_with_achievements_section(
            1,
            vec![make_achievement_entry("EMPTY_ICONS", "", "")],
        );
        let icons = extract_achievement_icons(&root, 1);
        let refs = icons.get("EMPTY_ICONS").expect("present");
        assert!(refs.icon.is_none());
        assert!(refs.icon_gray.is_none());
    }

    #[test]
    fn extract_icons_ignores_non_achievement_sections() {
        let root = make_schema_root(99, vec![make_stat_entry("0", "INT", "Kills")]);
        let icons = extract_achievement_icons(&root, 99);
        assert!(icons.is_empty());
    }
}
