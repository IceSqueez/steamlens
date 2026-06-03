use std::io;
use std::path::{Path, PathBuf};

use steamlens_vdf::{TextValue, TextVdfError, parse_text};
use thiserror::Error;

use crate::paths;

pub const STEAM_ID_64_INDIVIDUAL_MIN: u64 = 0x0110_0001_0000_0000;

#[derive(Debug, Clone)]
pub struct UserProfile {
    pub steam_id: u64,
    pub nickname: String,
    pub avatar_png_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("could not read loginusers.vdf: {0}")]
    LoginUsersIo(#[from] io::Error),
    #[error("loginusers.vdf parse failed: {0}")]
    LoginUsersParse(#[from] TextVdfError),
    #[error("loginusers.vdf contains no user entries")]
    NoUsers,
    #[error("invalid steam id: {0}")]
    InvalidSteamId(String),
}

pub fn load_local_profile() -> Result<UserProfile, ProfileError> {
    let root = paths::steam_install_root_candidates()
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("."));
    load_profile_from_root(&root)
}

pub fn load_profile_from_root(steam_root: &Path) -> Result<UserProfile, ProfileError> {
    let vdf_path = steam_root.join("config").join("loginusers.vdf");
    let content = std::fs::read_to_string(&vdf_path)?;
    let vdf_root = parse_text(&content)?;
    parse_profile(&vdf_root, steam_root)
}

fn parse_profile(vdf_root: &TextValue, steam_root: &Path) -> Result<UserProfile, ProfileError> {
    let users_block = vdf_root
        .get("users")
        .and_then(TextValue::as_block)
        .ok_or(ProfileError::NoUsers)?;

    if users_block.is_empty() {
        return Err(ProfileError::NoUsers);
    }

    let (id_str, user_block) = users_block
        .iter()
        .find(|(_id, user)| user.get("MostRecent").and_then(TextValue::as_str) == Some("1"))
        .or_else(|| users_block.iter().next())
        .ok_or(ProfileError::NoUsers)?;

    let steam_id: u64 = id_str
        .parse()
        .map_err(|_| ProfileError::InvalidSteamId(id_str.clone()))?;

    let nickname = user_block
        .get("PersonaName")
        .and_then(TextValue::as_str)
        .unwrap_or("")
        .to_owned();

    let avatar_path = steam_root
        .join("config")
        .join("avatarcache")
        .join(format!("{steam_id}.png"));

    let avatar_png_bytes = std::fs::read(&avatar_path).ok();

    Ok(UserProfile {
        steam_id,
        nickname,
        avatar_png_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_loginusers(dir: &Path, content: &str) {
        let config_dir = dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("loginusers.vdf"), content).unwrap();
    }

    #[test]
    fn non_numeric_steam_id_returns_invalid_steam_id_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        write_loginusers(
            dir,
            r#""users"
{
    "not_a_number"
    {
        "AccountName"   "user"
        "PersonaName"   "User"
        "MostRecent"    "1"
    }
}"#,
        );
        let err = load_profile_from_root(dir).unwrap_err();
        assert!(
            matches!(err, ProfileError::InvalidSteamId(ref bad_id) if bad_id == "not_a_number"),
            "expected InvalidSteamId(\"not_a_number\"), got {err:?}"
        );
    }

    #[test]
    fn valid_numeric_steam_id_parses_correctly() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        write_loginusers(
            dir,
            r#""users"
{
    "76561198000000001"
    {
        "AccountName"   "user"
        "PersonaName"   "User"
        "MostRecent"    "1"
    }
}"#,
        );
        let profile = load_profile_from_root(dir).unwrap();
        assert_eq!(profile.steam_id, 76561198000000001u64);
    }

    #[test]
    fn picks_most_recent_user() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        write_loginusers(
            dir,
            r#""users"
{
    "11111"
    {
        "AccountName"   "old_user"
        "PersonaName"   "Old"
        "MostRecent"    "0"
    }
    "22222"
    {
        "AccountName"   "new_user"
        "PersonaName"   "New"
        "MostRecent"    "1"
    }
}"#,
        );
        let profile = load_profile_from_root(dir).unwrap();
        assert_eq!(profile.steam_id, 22222);
        assert_eq!(profile.nickname, "New");
    }

    #[test]
    fn falls_back_to_first_when_no_most_recent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        write_loginusers(
            dir,
            r#""users"
{
    "55555"
    {
        "AccountName"   "first_user"
        "PersonaName"   "First"
        "MostRecent"    "0"
    }
    "66666"
    {
        "AccountName"   "second_user"
        "PersonaName"   "Second"
        "MostRecent"    "0"
    }
}"#,
        );
        let profile = load_profile_from_root(dir).unwrap();
        assert_eq!(profile.steam_id, 55555);
    }

    #[test]
    fn no_users_block_returns_no_users_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        write_loginusers(dir, r#""something_else" { }"#);
        let err = load_profile_from_root(dir).unwrap_err();
        assert!(matches!(err, ProfileError::NoUsers));
    }

    #[test]
    fn empty_users_block_returns_no_users_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        write_loginusers(dir, r#""users" { }"#);
        let err = load_profile_from_root(dir).unwrap_err();
        assert!(matches!(err, ProfileError::NoUsers));
    }

    #[test]
    fn missing_vdf_returns_login_users_io_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        let config_dir = dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let err = load_profile_from_root(dir).unwrap_err();
        assert!(matches!(err, ProfileError::LoginUsersIo(_)));
    }

    #[test]
    fn avatar_bytes_none_when_file_absent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        write_loginusers(
            dir,
            r#""users"
{
    "99999"
    {
        "AccountName"   "user"
        "PersonaName"   "User"
        "MostRecent"    "1"
    }
}"#,
        );
        let profile = load_profile_from_root(dir).unwrap();
        assert!(profile.avatar_png_bytes.is_none());
    }

    #[test]
    fn avatar_bytes_present_when_file_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        write_loginusers(
            dir,
            r#""users"
{
    "12345"
    {
        "AccountName"   "user"
        "PersonaName"   "User"
        "MostRecent"    "1"
    }
}"#,
        );
        let avatarcache = dir.join("config/avatarcache");
        std::fs::create_dir_all(&avatarcache).unwrap();
        let fake_png = b"\x89PNG\r\n\x1a\n";
        std::fs::write(avatarcache.join("12345.png"), fake_png).unwrap();

        let profile = load_profile_from_root(dir).unwrap();
        assert_eq!(
            profile.avatar_png_bytes.as_deref(),
            Some(fake_png.as_slice())
        );
    }
}
