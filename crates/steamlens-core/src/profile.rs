use std::io;
use std::path::{Path, PathBuf};

use steamlens_vdf::{TextValue, TextVdfError, parse_text};
use thiserror::Error;

/// Profile of the Steam user currently logged into the local client, read
/// entirely from disk — no live Steam pipe required.
#[derive(Debug, Clone)]
pub struct UserProfile {
    /// 64-bit SteamID (Steam community ID, not Steam3 ID).
    pub steam_id: u64,
    /// Display name as shown in the Steam overlay and friends list.
    pub persona_name: String,
    /// Login account name (ASCII, used for login — rarely displayed).
    pub account_name: String,
    /// Raw PNG bytes of the cached avatar, or `None` if no avatar is cached.
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
}

/// Load the profile of the most-recently-active local Steam user from disk.
///
/// Reads `<steam_root>/config/loginusers.vdf` (text VDF), selects the entry
/// where `MostRecent == "1"`, falling back to the first entry if none is
/// flagged. Also reads the cached avatar PNG from
/// `<steam_root>/config/avatarcache/<steam_id>.png` if it exists.
pub fn load_local_profile() -> Result<UserProfile, ProfileError> {
    let root = default_steam_root();
    load_profile_from_root(&root)
}

/// Testable entry point; accepts an explicit Steam root path.
pub fn load_profile_from_root(steam_root: &Path) -> Result<UserProfile, ProfileError> {
    let vdf_path = steam_root.join("config/loginusers.vdf");
    let content = std::fs::read_to_string(&vdf_path)?;
    let root = parse_text(&content)?;
    parse_profile(&root, steam_root)
}

fn parse_profile(root: &TextValue, steam_root: &Path) -> Result<UserProfile, ProfileError> {
    let users_block = root
        .get("users")
        .and_then(|v| v.as_block())
        .ok_or(ProfileError::NoUsers)?;

    if users_block.is_empty() {
        return Err(ProfileError::NoUsers);
    }

    let chosen = users_block
        .iter()
        .find(|(_, v)| v.get("MostRecent").and_then(|f| f.as_str()) == Some("1"))
        .or_else(|| users_block.iter().next())
        .ok_or(ProfileError::NoUsers)?;

    let (id_str, user_block) = chosen;

    let steam_id: u64 = id_str.parse().unwrap_or(0);

    let persona_name = user_block
        .get("PersonaName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();

    let account_name = user_block
        .get("AccountName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();

    let avatar_path = steam_root
        .join("config/avatarcache")
        .join(format!("{steam_id}.png"));

    let avatar_png_bytes = std::fs::read(&avatar_path).ok();

    Ok(UserProfile {
        steam_id,
        persona_name,
        account_name,
        avatar_png_bytes,
    })
}

fn default_steam_root() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".local/share/Steam")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join("Library/Application Support/Steam")
    }
    #[cfg(target_os = "windows")]
    {
        let program_files = std::env::var("ProgramFiles(x86)")
            .unwrap_or_else(|_| r"C:\Program Files (x86)".to_owned());
        PathBuf::from(program_files).join("Steam")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_loginusers(dir: &Path, content: &str) {
        let config_dir = dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("loginusers.vdf"), content).unwrap();
    }

    fn tempdir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "steamlens_profile_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn picks_most_recent_user() {
        let dir = tempdir();
        write_loginusers(
            &dir,
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
        let profile = load_profile_from_root(&dir).unwrap();
        assert_eq!(profile.steam_id, 22222);
        assert_eq!(profile.account_name, "new_user");
        assert_eq!(profile.persona_name, "New");
    }

    #[test]
    fn falls_back_to_first_when_no_most_recent() {
        let dir = tempdir();
        write_loginusers(
            &dir,
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
        let profile = load_profile_from_root(&dir).unwrap();
        assert_eq!(profile.steam_id, 55555);
        assert_eq!(profile.account_name, "first_user");
    }

    #[test]
    fn no_users_block_returns_no_users_error() {
        let dir = tempdir();
        write_loginusers(&dir, r#""something_else" { }"#);
        let err = load_profile_from_root(&dir).unwrap_err();
        assert!(matches!(err, ProfileError::NoUsers));
    }

    #[test]
    fn empty_users_block_returns_no_users_error() {
        let dir = tempdir();
        write_loginusers(&dir, r#""users" { }"#);
        let err = load_profile_from_root(&dir).unwrap_err();
        assert!(matches!(err, ProfileError::NoUsers));
    }

    #[test]
    fn missing_vdf_returns_login_users_io_error() {
        let dir = tempdir();
        let config_dir = dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let err = load_profile_from_root(&dir).unwrap_err();
        assert!(matches!(err, ProfileError::LoginUsersIo(_)));
    }

    #[test]
    fn avatar_bytes_none_when_file_absent() {
        let dir = tempdir();
        write_loginusers(
            &dir,
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
        let profile = load_profile_from_root(&dir).unwrap();
        assert!(profile.avatar_png_bytes.is_none());
    }

    #[test]
    fn avatar_bytes_present_when_file_exists() {
        let dir = tempdir();
        write_loginusers(
            &dir,
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

        let profile = load_profile_from_root(&dir).unwrap();
        assert_eq!(
            profile.avatar_png_bytes.as_deref(),
            Some(fake_png.as_slice())
        );
    }

    #[test]
    #[ignore = "requires real Steam installation on disk"]
    fn load_local_profile_live() {
        let profile =
            load_local_profile().expect("should succeed on a machine with Steam installed");
        assert!(profile.steam_id > 0, "steam_id must be non-zero");
        assert!(
            !profile.account_name.is_empty(),
            "account_name must not be empty"
        );
        println!(
            "Profile: steam_id={}, account={}, persona={}, avatar={} bytes",
            profile.steam_id,
            profile.account_name,
            profile.persona_name,
            profile
                .avatar_png_bytes
                .as_ref()
                .map(|b| b.len())
                .unwrap_or(0)
        );
    }
}
