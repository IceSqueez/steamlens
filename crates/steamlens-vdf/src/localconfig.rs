use std::collections::HashMap;

use crate::text::parse as parse_text;

/// Parse `localconfig.vdf` and return `app_id → LastPlayed` Unix
/// timestamps, dropping zero values. Errors collapse to an empty map —
/// this file is supplementary and must not block boot.
pub fn parse_localconfig_last_played(content: &str) -> HashMap<u32, u32> {
    let root = match parse_text(content) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    let apps = root
        .get("UserLocalConfigStore")
        .and_then(|v| v.get("Software"))
        .and_then(|v| v.get("Valve"))
        .and_then(|v| v.get("Steam"))
        .and_then(|v| v.get("apps"));

    let Some(apps_section) = apps else {
        return HashMap::new();
    };

    let Some(entries) = apps_section.as_block() else {
        return HashMap::new();
    };

    let mut map = HashMap::new();

    for (key, entry) in entries {
        let app_id: u32 = match key.parse() {
            Ok(id) => id,
            Err(_) => continue,
        };

        let last_played = entry
            .get("LastPlayed")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&ts| ts != 0);

        if let Some(ts) = last_played {
            map.insert(app_id, ts);
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap(apps_content: &str) -> String {
        format!(
            r#""UserLocalConfigStore"
{{
    "Software"
    {{
        "Valve"
        {{
            "Steam"
            {{
                "apps"
                {{
{apps_content}
                }}
            }}
        }}
    }}
}}"#
        )
    }

    #[test]
    fn present_last_played_included() {
        let vdf = wrap(
            r#"                    "12345"
                    {
                        "LastPlayed" "1700000000"
                    }"#,
        );
        let map = parse_localconfig_last_played(&vdf);
        assert_eq!(map.get(&12345), Some(&1_700_000_000u32));
    }

    #[test]
    fn last_played_zero_excluded() {
        let vdf = wrap(
            r#"                    "12345"
                    {
                        "LastPlayed" "0"
                    }"#,
        );
        let map = parse_localconfig_last_played(&vdf);
        assert!(!map.contains_key(&12345));
    }

    #[test]
    fn missing_last_played_excluded() {
        let vdf = wrap(
            r#"                    "12345"
                    {
                        "Playtime" "42"
                    }"#,
        );
        let map = parse_localconfig_last_played(&vdf);
        assert!(!map.contains_key(&12345));
    }

    #[test]
    fn non_numeric_key_skipped() {
        let vdf = wrap(
            r#"                    "IgnorableConflicts"
                    {
                        "LastPlayed" "999"
                    }"#,
        );
        let map = parse_localconfig_last_played(&vdf);
        assert!(map.is_empty());
    }

    #[test]
    fn multiple_apps_mixed() {
        let vdf = wrap(
            r#"                    "100"
                    {
                        "LastPlayed" "111"
                    }
                    "200"
                    {
                        "LastPlayed" "0"
                    }
                    "300"
                    {
                        "Playtime" "5"
                    }
                    "400"
                    {
                        "LastPlayed" "444"
                    }"#,
        );
        let map = parse_localconfig_last_played(&vdf);
        assert_eq!(map.get(&100), Some(&111u32));
        assert!(!map.contains_key(&200));
        assert!(!map.contains_key(&300));
        assert_eq!(map.get(&400), Some(&444u32));
    }

    #[test]
    fn missing_apps_block_returns_empty() {
        let vdf = r#""UserLocalConfigStore" { "something" { } }"#;
        let map = parse_localconfig_last_played(vdf);
        assert!(map.is_empty());
    }

    #[test]
    fn malformed_vdf_returns_empty() {
        let map = parse_localconfig_last_played("not valid vdf {{{{");
        assert!(map.is_empty());
    }
}
