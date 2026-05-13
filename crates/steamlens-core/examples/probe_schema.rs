use steamlens_core::load_achievement_icons;

fn main() {
    for &app_id in &[220_u32, 570, 286690, 105600, 480] {
        let icons = load_achievement_icons(app_id).unwrap_or_default();
        eprintln!("app {} -> {} entries", app_id, icons.len());
        for (k, v) in icons.iter().take(3) {
            eprintln!("  {}: icon={:?} icon_gray={:?}", k, v.icon, v.icon_gray);
        }
    }
}
