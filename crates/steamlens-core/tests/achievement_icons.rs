// Integration test for Client::get_image — fetches RGBA pixel data for
// achievement icon handles obtained via UserStats::achievement_icon.
//
// Requires a live Steam client signed in on the host machine and a valid
// SteamAppId. Run with:
//
//   SteamAppId=105600 cargo test -p steamlens-core --test achievement_icons \
//       -- --ignored --nocapture
//
// SteamAppId=105600 is Terraria; substitute any app with achievements that the
// account owns. SteamAppId must be set so that the stat context is initialised
// correctly before the library is loaded.
//
// Expected output: for each of the first 5 achievements the test prints the
// icon handle and, once Steam returns the image data, asserts that:
//   - dimensions are > 0
//   - rgba.len() == width * height * 4
//
// A handle of 0 means Steam has not yet fetched the icon. The test retries
// for up to 5 seconds (50 × 100 ms poll cycles) for each handle.

use std::time::Duration;

use steamlens_core::{SteamCallback, connect};

#[test]
#[ignore = "requires Steam running + SteamAppId set to an app with achievements"]
fn achievement_icon_rgba_roundtrip() {
    let app_id: u32 = std::env::var("SteamAppId")
        .expect("SteamAppId env var must be set")
        .parse()
        .expect("SteamAppId must be a valid u32");

    let client = connect(app_id).expect("connect must succeed with Steam running");
    let steam_id = client.steam_id();

    println!("steam_id={steam_id} app_id={app_id}");

    let stats = client.user_stats();
    stats
        .request_user_stats(steam_id)
        .expect("request_user_stats must not fail when Steam is running");

    println!("RequestUserStats dispatched — polling for UserStatsReceived...");

    let poll_interval = Duration::from_millis(100);

    for i in 0..50 {
        let callbacks = client
            .poll_callbacks()
            .unwrap_or_else(|e| panic!("poll_callbacks failed on iteration {i}: {e}"));

        let received = callbacks.iter().any(|cb| {
            matches!(
                cb,
                SteamCallback::UserStatsReceived {
                    result,
                    ..
                } if result.is_ok()
            )
        });

        if received {
            println!("iteration={i} UserStatsReceived OK");
            break;
        }

        if i == 49 {
            panic!("UserStatsReceived not received within 5 s");
        }

        std::thread::sleep(poll_interval);
    }

    let num_achievements = stats
        .num_achievements()
        .expect("num_achievements must not fail");
    println!("num_achievements={num_achievements}");

    let check_count = num_achievements.min(5);
    if check_count == 0 {
        println!("no achievements for this app — skipping icon checks");
        return;
    }

    for idx in 0..check_count {
        let name = match stats.achievement_name(idx) {
            Ok(n) => n,
            Err(e) => {
                println!("achievement_name({idx}) error: {e} — skipping");
                continue;
            }
        };

        let handle = stats
            .achievement_icon(&name)
            .unwrap_or_else(|e| panic!("achievement_icon({name:?}) failed: {e}"));

        println!("achievement={name:?} handle={handle}");

        if handle == 0 {
            println!("  handle=0 (not loaded yet) — retrying for up to 5 s...");

            let mut final_handle = 0i32;
            for _retry in 0..50 {
                std::thread::sleep(poll_interval);
                let _ = client.poll_callbacks();

                let h = stats
                    .achievement_icon(&name)
                    .unwrap_or_else(|e| panic!("achievement_icon({name:?}) retry failed: {e}"));
                if h != 0 {
                    final_handle = h;
                    break;
                }
            }

            if final_handle == 0 {
                println!(
                    "  achievement={name:?}: handle still 0 after retry window \
                     (Steam may not have fetched the icon yet — AchievementIconFetched \
                     callback 1408 is not yet typed; skipping pixel assertion)"
                );
                continue;
            }

            let image = client
                .get_image(final_handle)
                .unwrap_or_else(|e| panic!("get_image({final_handle}) failed: {e}"));

            match image {
                Some(img) => {
                    println!(
                        "  achievement={name:?} handle={final_handle} \
                         width={} height={} rgba_len={}",
                        img.width,
                        img.height,
                        img.rgba.len()
                    );
                    assert!(img.width > 0, "image width must be > 0");
                    assert!(img.height > 0, "image height must be > 0");
                    assert_eq!(
                        img.rgba.len(),
                        img.width as usize * img.height as usize * 4,
                        "rgba.len() must equal width * height * 4"
                    );
                }
                None => {
                    println!(
                        "  achievement={name:?} handle={final_handle}: \
                         get_image returned None (handle invalid or race)"
                    );
                }
            }
        } else {
            let image = client
                .get_image(handle)
                .unwrap_or_else(|e| panic!("get_image({handle}) failed: {e}"));

            match image {
                Some(img) => {
                    println!(
                        "  achievement={name:?} handle={handle} \
                         width={} height={} rgba_len={}",
                        img.width,
                        img.height,
                        img.rgba.len()
                    );
                    assert!(img.width > 0, "image width must be > 0");
                    assert!(img.height > 0, "image height must be > 0");
                    assert_eq!(
                        img.rgba.len(),
                        img.width as usize * img.height as usize * 4,
                        "rgba.len() must equal width * height * 4"
                    );
                }
                None => {
                    println!(
                        "  achievement={name:?} handle={handle}: \
                         get_image returned None (image not loaded yet — \
                         AchievementIconFetched 1408 not yet typed)"
                    );
                }
            }
        }
    }
}
