use std::path::PathBuf;

use steamlens_core::AchievementIcon;

fn icons_dir(app_id: u32) -> PathBuf {
    crate::paths::cache_dir()
        .join("games")
        .join(app_id.to_string())
        .join("icons")
}

fn icon_path(app_id: u32, ach_id: &str) -> PathBuf {
    let safe = ach_id.replace(['/', '\\', ':', '\0'], "_");
    icons_dir(app_id).join(format!("{safe}.png"))
}

pub fn write_blocking(app_id: u32, ach_id: &str, icon: &AchievementIcon) -> std::io::Result<()> {
    use image::{ImageFormat, RgbaImage};
    use std::io::Cursor;

    let Some(rgba) = RgbaImage::from_raw(icon.width, icon.height, icon.rgba.clone()) else {
        return Err(std::io::Error::other(
            "RGBA dimensions mismatch buffer size",
        ));
    };
    let mut buf = Cursor::new(Vec::with_capacity(icon.rgba.len() / 4));
    rgba.write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| std::io::Error::other(format!("PNG encode failed: {e}")))?;
    let bytes = buf.into_inner();

    let path = icon_path(app_id, ach_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, bytes)
}

pub fn load_blocking(app_id: u32, ach_id: &str) -> Option<AchievementIcon> {
    let path = icon_path(app_id, ach_id);
    let bytes = std::fs::read(&path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.into_rgba8();
    Some(AchievementIcon {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    })
}
