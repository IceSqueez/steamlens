use crate::app_context::AppContext;
use crate::messaging::{self, BannerSeverity};
use crate::{App, Message, Screen};

pub(crate) fn looks_like_steam_died(reason: &str) -> bool {
    let r = reason.to_lowercase();
    r.contains("steam client is not running")
        || r.contains("steam is not running")
        || r.contains("timed out waiting for userstatsreceived")
        || r.contains("connect:")
        || r.contains("unexpectedeof")
        || r.contains("worker killed by signal")
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SteamUnavailable {
    NotRunning,
    NotLoggedIn,
}

pub(crate) fn surface_steam_unavailable(ctx: &mut AppContext, state: SteamUnavailable) {
    let body: &'static str = match state {
        SteamUnavailable::NotRunning => "Steam is not running \u{2014} reconnect to load live data",
        SteamUnavailable::NotLoggedIn => {
            "Steam is running but no user is signed in \u{2014} showing cached data"
        }
    };
    if ctx.messaging.banners.iter().any(|b| b.body == body) {
        return;
    }
    ctx.messaging
        .dismiss_all_banners_by_severity(BannerSeverity::Warning);
    ctx.messaging.push_banner(
        BannerSeverity::Warning,
        body,
        Some(messaging::BannerAction {
            label: "Reconnect",
            message: Message::RetrySteamConnect,
        }),
        false,
    );
}

pub(crate) fn mark_steam_offline_and_warn(app: &mut App) {
    app.context.connectivity.steam_running = Some(false);
    if let Screen::ProfileView(pv_state) = &mut app.screen {
        pv_state.stop_scan();
        pv_state.last_scan_completed_at = Some(std::time::Instant::now());
    }
    surface_steam_unavailable(&mut app.context, SteamUnavailable::NotRunning);
}
