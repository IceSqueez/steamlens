use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use steamlens_vdf::{ImageAsset, parse_appinfo_assets};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: dump_app_assets <appid> [<appid> ...]");
        return ExitCode::from(2);
    }

    let path = PathBuf::from(env::var("HOME").expect("$HOME"))
        .join(".local/share/Steam/appcache/appinfo.vdf");
    let bytes = std::fs::read(&path).expect("read appinfo.vdf");
    let map = parse_appinfo_assets(&bytes).expect("parse appinfo");

    for arg in args {
        let app_id: u32 = arg.parse().expect("appid u32");
        match map.get(&app_id) {
            Some(a) => {
                println!("=== app {app_id} ===");
                print_slot("cover             (portrait)", &a.cover);
                print_slot("background        (landscape art)", &a.background);
                print_slot("logo              (transparent overlay)", &a.logo);
                print_slot(
                    "wide_cover        (modern landscape with text)",
                    &a.wide_cover,
                );
                print_slot(
                    "wide_cover_legacy (old landscape with text)",
                    &a.wide_cover_legacy,
                );
                println!();
                println!("URLs (our flow):");
                emit_url(app_id, "cover", &a.cover);
                emit_url(app_id, "background", &a.background);
                emit_url(app_id, "logo", &a.logo);
                emit_url(app_id, "wide_cover", &a.wide_cover);
                emit_url(app_id, "wide_cover_legacy", &a.wide_cover_legacy);
            }
            None => println!("{app_id}: <NOT IN MAP>"),
        }
    }
    ExitCode::SUCCESS
}

fn print_slot(label: &str, slot: &Option<ImageAsset>) {
    match slot {
        Some(ImageAsset::Hashed { hash, filename }) => {
            println!("  {label}: Hashed hash={hash} filename={filename}");
        }
        Some(ImageAsset::Plain { filename }) => {
            println!("  {label}: Plain filename={filename}");
        }
        None => println!("  {label}: <none>"),
    }
}

fn emit_url(app_id: u32, slot: &str, asset: &Option<ImageAsset>) {
    let Some(asset) = asset else {
        return;
    };
    let url = match asset {
        ImageAsset::Hashed { hash, filename } => format!(
            "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{app_id}/{hash}/{filename}"
        ),
        ImageAsset::Plain { filename } => format!(
            "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{app_id}/{filename}"
        ),
    };
    println!("  {slot}: {url}");
}
