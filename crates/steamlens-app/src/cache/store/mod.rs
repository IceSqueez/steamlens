mod game_cache;
mod game_summary;
mod primitives;

pub(crate) use game_cache::merge_preserved_fields;
pub use game_cache::{delete_game_cache_dir, load_game_cache, write_game_cache};
pub(crate) use game_summary::load_game_summary_from_path;
pub use game_summary::write_game_summary;
pub use primitives::{CacheIoError, atomic_write};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::types::{
        CURRENT_SCHEMA_VERSION, CachedAchievement, CachedProgress, CachedStat, CachedStatValue,
        GameAchievementsCache, GameCacheEntry, GameSummaryCache, SUMMARY_SCHEMA_VERSION,
    };
    use std::path::Path;
    use std::sync::Arc;

    use super::game_cache::{load_game_cache_from_path, write_game_cache_at};
    use super::primitives::cache_write_lock;

    fn make_entry(app_id: u32, name: &str) -> GameCacheEntry {
        GameCacheEntry {
            schema_version: CURRENT_SCHEMA_VERSION,
            app_id,
            name: name.to_owned(),
            steam_last_played: 1_777_926_953,
            cached_at: 1_746_360_000,
            achievements: vec![
                CachedAchievement {
                    api_name: "KILL_BOSS".to_owned(),
                    display_name: "Miner for Fire".to_owned(),
                    description: "Defeat the Wall of Flesh.".to_owned(),
                    is_hidden: false,
                    icon_path: Some("achievements/105600/KILL_BOSS.jpg".to_owned()),
                    icon_locked_path: Some("achievements/105600/KILL_BOSS_locked.jpg".to_owned()),
                    is_achieved: true,
                    earned_at: Some(1_700_000_000),
                    global_percent: Some(18.5),
                },
                CachedAchievement {
                    api_name: "NEVER_EARNED".to_owned(),
                    display_name: "Hidden Gem".to_owned(),
                    description: String::new(),
                    is_hidden: true,
                    icon_path: None,
                    icon_locked_path: None,
                    is_achieved: false,
                    earned_at: None,
                    global_percent: None,
                },
            ],
            stats: vec![CachedStat {
                api_name: "NumDeaths".to_owned(),
                display_name: "Deaths".to_owned(),
                value: CachedStatValue::Int(42),
                max_value: None,
                min_value: None,
                default_value: None,
                is_increment_only: false,
                permission: 0,
            }],
            progress: CachedProgress {
                earned: 1,
                total: 2,
            },
            tier_breakdown: Vec::new(),
            genre: None,
            playtime_minutes: None,
        }
    }

    fn count_tmp_leftovers(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .expect("readdir")
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .count()
    }

    #[tokio::test]
    async fn atomic_write_creates_file_with_expected_bytes() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let target = dir.path().join("test.bin");
        let payload = b"hello steamlens";

        atomic_write(&target, payload).await.expect("write");

        let read_back = std::fs::read(&target).expect("read");
        assert_eq!(read_back, payload);

        assert_eq!(
            count_tmp_leftovers(dir.path()),
            0,
            "no .tmp.* files must remain after rename"
        );
    }

    #[tokio::test]
    async fn atomic_write_overwrites_existing_file() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let target = dir.path().join("overwrite.bin");
        std::fs::write(&target, b"old content").expect("setup");

        atomic_write(&target, b"new content").await.expect("write");

        let read_back = std::fs::read(&target).expect("read");
        assert_eq!(read_back, b"new content");

        assert_eq!(
            count_tmp_leftovers(dir.path()),
            0,
            "no .tmp.* files must remain"
        );
    }

    #[tokio::test]
    async fn atomic_write_concurrent_to_same_target_does_not_race() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let target = dir.path().join("contended.bin");

        let mut tasks = Vec::new();
        for i in 0u8..16 {
            let path = target.clone();
            tasks.push(tokio::spawn(async move {
                let payload = vec![i; 64];
                atomic_write(&path, &payload).await
            }));
        }

        for t in tasks {
            t.await
                .expect("join")
                .expect("each concurrent write must succeed");
        }

        let final_bytes = std::fs::read(&target).expect("final read");
        assert_eq!(final_bytes.len(), 64, "final file size must be 64 bytes");
        let marker = final_bytes[0];
        assert!(
            final_bytes.iter().all(|&b| b == marker),
            "final file must be one writer's full payload, not a mix"
        );

        assert_eq!(
            count_tmp_leftovers(dir.path()),
            0,
            "no .tmp.* leftovers from concurrent writes"
        );
    }

    #[tokio::test]
    async fn atomic_write_creates_parent_dirs() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let target = dir.path().join("nested").join("deep").join("file.bin");

        atomic_write(&target, b"nested").await.expect("write");

        let read_back = std::fs::read(&target).expect("read");
        assert_eq!(read_back, b"nested");
    }

    #[tokio::test]
    async fn game_cache_round_trip() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("105600.json");
        let original = make_entry(105600, "Terraria");

        let bytes = serde_json::to_vec_pretty(&original).expect("serialize");
        atomic_write(&path, &bytes).await.expect("write");

        let restored = load_game_cache_from_path(&path)
            .await
            .expect("should deserialize");

        assert_eq!(restored.app_id, original.app_id);
        assert_eq!(restored.name, original.name);
        assert_eq!(restored.schema_version, original.schema_version);
        assert_eq!(restored.steam_last_played, original.steam_last_played);
        assert_eq!(restored.cached_at, original.cached_at);
        assert_eq!(restored.achievements.len(), original.achievements.len());
        assert_eq!(restored.achievements[0].api_name, "KILL_BOSS");
        assert!(restored.achievements[0].is_achieved);
        assert_eq!(restored.achievements[0].earned_at, Some(1_700_000_000));
        assert_eq!(restored.achievements[0].global_percent, Some(18.5));
        assert_eq!(restored.achievements[1].earned_at, None);
        assert_eq!(restored.achievements[1].global_percent, None);
        assert_eq!(restored.achievements[1].icon_path, None);
        assert_eq!(restored.stats[0].value, CachedStatValue::Int(42));
        assert_eq!(restored.progress.earned, 1);
        assert_eq!(restored.progress.total, 2);
    }

    #[tokio::test]
    async fn game_cache_null_options_round_trip() {
        let entry = make_entry(105600, "Terraria");
        let json = serde_json::to_vec_pretty(&entry).expect("serialize");
        let text = std::str::from_utf8(&json).expect("utf8");

        assert!(
            text.contains("\"earned_at\": null"),
            "None earned_at must serialize as null, got:\n{text}"
        );
        assert!(
            text.contains("\"global_percent\": null"),
            "None global_percent must serialize as null, got:\n{text}"
        );
        assert!(
            text.contains("\"icon_path\": null"),
            "None icon_path must serialize as null, got:\n{text}"
        );

        let restored: GameCacheEntry = serde_json::from_slice(&json).expect("deserialize");
        assert_eq!(restored.achievements[1].earned_at, None);
        assert_eq!(restored.achievements[1].global_percent, None);
        assert_eq!(restored.achievements[1].icon_path, None);
    }

    #[tokio::test]
    async fn game_cache_atomic_overwrite() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("200.json");

        let entry_v1 = make_entry(200, "Game One");
        let bytes_v1 = serde_json::to_vec_pretty(&entry_v1).expect("serialize v1");
        atomic_write(&path, &bytes_v1).await.expect("write v1");

        let entry_v2 = make_entry(200, "Game Two");
        let bytes_v2 = serde_json::to_vec_pretty(&entry_v2).expect("serialize v2");
        atomic_write(&path, &bytes_v2).await.expect("write v2");

        let loaded = load_game_cache_from_path(&path)
            .await
            .expect("should deserialize v2");
        assert_eq!(loaded.name, "Game Two");
    }

    #[tokio::test]
    async fn load_game_cache_schema_mismatch_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("999.json");
        let bad_json = br#"{"schema_version":999,"app_id":999,"name":"Bad","steam_last_played":0,"cached_at":0,"achievements":[],"stats":[],"progress":{"earned":0,"total":0}}"#;
        std::fs::write(&path, bad_json).expect("write");

        let result = load_game_cache_from_path(&path).await;
        assert!(result.is_none(), "schema mismatch must return None");
    }

    #[tokio::test]
    async fn load_game_cache_missing_file_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("nonexistent_app.json");
        let result = load_game_cache_from_path(&path).await;
        assert!(result.is_none(), "missing file must return None");
    }

    #[tokio::test]
    async fn load_game_cache_corrupted_json_returns_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("corrupted.json");
        std::fs::write(&path, b"not json at all ][[[").expect("write");

        let result = load_game_cache_from_path(&path).await;
        assert!(result.is_none(), "corrupted JSON must return None");
    }

    #[tokio::test]
    async fn game_summary_round_trip() {
        let summary = GameSummaryCache {
            schema_version: SUMMARY_SCHEMA_VERSION,
            app_id: 105600,
            name: "Terraria".to_owned(),
            cached_change_number: 42,
            cached_at: 1_746_360_000,
            progress: CachedProgress {
                earned: 18,
                total: 88,
            },
            tier_breakdown: Vec::new(),
            genre: Some("Action".to_owned()),
            playtime_minutes: None,
        };

        let bytes = serde_json::to_vec_pretty(&summary).expect("serialize");
        let restored: GameSummaryCache = serde_json::from_slice(&bytes).expect("deserialize");

        assert_eq!(restored.app_id, summary.app_id);
        assert_eq!(restored.cached_change_number, 42);
        assert_eq!(restored.progress.earned, 18);
        assert_eq!(restored.progress.total, 88);
        assert_eq!(restored.genre.as_deref(), Some("Action"));
        assert_eq!(restored.schema_version, SUMMARY_SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn game_achievements_round_trip() {
        let achievements = GameAchievementsCache {
            schema_version: SUMMARY_SCHEMA_VERSION,
            app_id: 105600,
            cached_at: 1_746_360_000,
            achievements: vec![CachedAchievement {
                api_name: "KILL_BOSS".to_owned(),
                display_name: "Miner for Fire".to_owned(),
                description: "Defeat the Wall of Flesh.".to_owned(),
                is_hidden: false,
                icon_path: None,
                icon_locked_path: None,
                is_achieved: true,
                earned_at: Some(1_700_000_000),
                global_percent: Some(18.5),
            }],
            stats: vec![CachedStat {
                api_name: "NumDeaths".to_owned(),
                display_name: "Deaths".to_owned(),
                value: CachedStatValue::Int(42),
                max_value: None,
                min_value: None,
                default_value: None,
                is_increment_only: false,
                permission: 0,
            }],
        };

        let bytes = serde_json::to_vec_pretty(&achievements).expect("serialize");
        let restored: GameAchievementsCache = serde_json::from_slice(&bytes).expect("deserialize");

        assert_eq!(restored.app_id, achievements.app_id);
        assert_eq!(restored.achievements.len(), 1);
        assert_eq!(restored.achievements[0].api_name, "KILL_BOSS");
        assert!(restored.achievements[0].is_achieved);
        assert_eq!(restored.stats[0].value, CachedStatValue::Int(42));
        assert_eq!(restored.schema_version, SUMMARY_SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn write_and_load_game_summary_round_trip() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("summary.json");

        let summary = GameSummaryCache {
            schema_version: SUMMARY_SCHEMA_VERSION,
            app_id: 200,
            name: "Half-Life 2".to_owned(),
            cached_change_number: 7,
            cached_at: 1_746_000_000,
            progress: CachedProgress {
                earned: 10,
                total: 33,
            },
            tier_breakdown: Vec::new(),
            genre: None,
            playtime_minutes: None,
        };

        let bytes = serde_json::to_vec_pretty(&summary).expect("serialize");
        atomic_write(&path, &bytes).await.expect("write");

        let read_back = std::fs::read(&path).expect("read");
        let restored: GameSummaryCache = serde_json::from_slice(&read_back).expect("deserialize");
        assert_eq!(restored.cached_change_number, 7);
        assert_eq!(restored.progress.total, 33);
    }

    fn empty_ach(api: &str) -> CachedAchievement {
        CachedAchievement {
            api_name: api.to_owned(),
            display_name: String::new(),
            description: String::new(),
            is_hidden: false,
            icon_path: None,
            icon_locked_path: None,
            is_achieved: false,
            earned_at: None,
            global_percent: None,
        }
    }

    fn filled_ach(api: &str, name: &str, desc: &str, is_hidden: bool) -> CachedAchievement {
        CachedAchievement {
            api_name: api.to_owned(),
            display_name: name.to_owned(),
            description: desc.to_owned(),
            is_hidden,
            icon_path: None,
            icon_locked_path: None,
            is_achieved: false,
            earned_at: None,
            global_percent: None,
        }
    }

    fn shell_entry(achievements: Vec<CachedAchievement>, genre: Option<String>) -> GameCacheEntry {
        GameCacheEntry {
            schema_version: CURRENT_SCHEMA_VERSION,
            app_id: 1,
            name: "X".to_owned(),
            steam_last_played: 0,
            cached_at: 0,
            achievements,
            stats: Vec::new(),
            progress: CachedProgress {
                earned: 0,
                total: 0,
            },
            tier_breakdown: Vec::new(),
            genre,
            playtime_minutes: None,
        }
    }

    #[test]
    fn merge_keeps_old_display_when_new_empty() {
        let old = shell_entry(
            vec![filled_ach("ACH_A", "Boss Killer", "Defeat the boss.", true)],
            Some("RPG".to_owned()),
        );
        let mut new = shell_entry(vec![empty_ach("ACH_A")], None);

        merge_preserved_fields(&mut new, &old);

        assert_eq!(new.achievements[0].display_name, "Boss Killer");
        assert_eq!(new.achievements[0].description, "Defeat the boss.");
        assert!(new.achievements[0].is_hidden);
        assert_eq!(new.genre.as_deref(), Some("RPG"));
    }

    #[test]
    fn merge_keeps_new_when_present() {
        let old = shell_entry(
            vec![filled_ach("ACH_A", "Old Name", "Old desc.", false)],
            Some("Old".to_owned()),
        );
        let mut new = shell_entry(
            vec![filled_ach("ACH_A", "New Name", "New desc.", true)],
            Some("New".to_owned()),
        );

        merge_preserved_fields(&mut new, &old);

        assert_eq!(new.achievements[0].display_name, "New Name");
        assert_eq!(new.achievements[0].description, "New desc.");
        assert!(new.achievements[0].is_hidden);
        assert_eq!(new.genre.as_deref(), Some("New"));
    }

    #[test]
    fn merge_skips_unknown_new_achievement() {
        let old = shell_entry(vec![filled_ach("ACH_A", "Old", "Old.", false)], None);
        let mut new = shell_entry(vec![empty_ach("ACH_B")], None);

        merge_preserved_fields(&mut new, &old);

        assert_eq!(new.achievements[0].api_name, "ACH_B");
        assert!(new.achievements[0].display_name.is_empty());
        assert!(new.achievements[0].description.is_empty());
    }

    #[test]
    fn merge_ignores_dropped_old_achievement() {
        let old = shell_entry(
            vec![
                filled_ach("ACH_A", "Kept", "k.", false),
                filled_ach("ACH_DROPPED", "Gone", "g.", false),
            ],
            None,
        );
        let mut new = shell_entry(vec![empty_ach("ACH_A")], None);

        merge_preserved_fields(&mut new, &old);

        assert_eq!(new.achievements.len(), 1);
        assert_eq!(new.achievements[0].display_name, "Kept");
    }

    #[test]
    fn cache_write_lock_same_app_id_returns_same_arc() {
        let l1 = cache_write_lock(91337);
        let l2 = cache_write_lock(91337);
        assert!(
            Arc::ptr_eq(&l1, &l2),
            "two calls for same app_id must yield the same lock instance"
        );
    }

    #[test]
    fn cache_write_lock_distinct_app_ids_return_different_arcs() {
        let l1 = cache_write_lock(91338);
        let l2 = cache_write_lock(91339);
        assert!(
            !Arc::ptr_eq(&l1, &l2),
            "distinct app_ids must yield distinct lock instances"
        );
    }

    #[tokio::test]
    async fn concurrent_writes_preserve_fields_across_writers() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("91340.json");

        let filled = GameCacheEntry {
            schema_version: CURRENT_SCHEMA_VERSION,
            app_id: 91340,
            name: "Race Test".to_owned(),
            steam_last_played: 0,
            cached_at: 0,
            achievements: vec![CachedAchievement {
                api_name: "ACH_X".to_owned(),
                display_name: "Filled Display".to_owned(),
                description: "Filled Description.".to_owned(),
                is_hidden: true,
                icon_path: None,
                icon_locked_path: None,
                is_achieved: false,
                earned_at: None,
                global_percent: None,
            }],
            stats: Vec::new(),
            progress: CachedProgress {
                earned: 0,
                total: 1,
            },
            tier_breakdown: Vec::new(),
            genre: Some("Action".to_owned()),
            playtime_minutes: None,
        };

        let empty = GameCacheEntry {
            achievements: vec![CachedAchievement {
                api_name: "ACH_X".to_owned(),
                display_name: String::new(),
                description: String::new(),
                is_hidden: false,
                ..filled.achievements[0].clone()
            }],
            genre: None,
            ..filled.clone()
        };

        let p1 = path.clone();
        let p2 = path.clone();
        let (r1, r2) = tokio::join!(
            write_game_cache_at(&p1, &filled),
            write_game_cache_at(&p2, &empty),
        );
        r1.expect("filled write");
        r2.expect("empty write");

        let final_entry = load_game_cache_from_path(&path)
            .await
            .expect("disk readback");

        let ach = &final_entry.achievements[0];
        assert_eq!(
            ach.display_name, "Filled Display",
            "filled display_name must survive the race"
        );
        assert_eq!(
            ach.description, "Filled Description.",
            "filled description must survive the race"
        );
        assert!(ach.is_hidden, "filled `hidden` flag must survive the race");
        assert_eq!(
            final_entry.genre.as_deref(),
            Some("Action"),
            "filled genre must survive the race"
        );
    }
}
