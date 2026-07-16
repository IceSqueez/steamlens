use std::time::Instant;

use crate::app_context::AppContext;
use crate::messaging::{self, BannerSeverity};
use crate::{App, Message, Screen};

pub(crate) fn looks_like_steam_died(reason: &str) -> bool {
    let lower = reason.to_lowercase();
    lower.contains("steam client is not running")
        || lower.contains("steam is not running")
        || lower.contains("timed out waiting for userstatsreceived")
        || lower.contains("connect:")
        || lower.contains("unexpectedeof")
        || lower.contains("worker killed by signal")
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SteamUnavailable {
    NotRunning,
    NotLoggedIn,
}

pub(crate) fn surface_steam_unavailable(ctx: &mut AppContext, state: SteamUnavailable) {
    let body: &'static str = match state {
        SteamUnavailable::NotRunning => "Steam is not running - reconnect to load live data",
        SteamUnavailable::NotLoggedIn => {
            "Steam is running but no user is signed in - showing cached data"
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
    if let Screen::ProfileView(profile_view_state) = &mut app.screen {
        profile_view_state.stop_scan();
        profile_view_state.last_scan_completed_at = Some(Instant::now());
    }
    surface_steam_unavailable(&mut app.context, SteamUnavailable::NotRunning);
}

#[cfg(test)]
mod tests {
    use super::looks_like_steam_died;
    use crate::worker_subprocess::WorkerProtocolError;
    use steamlens_core::SteamError;
    use steamlens_core::ipc::WorkerErrorStage;

    #[test]
    fn global_user_unavailable_is_not_treated_as_dead_steam() {
        let message = SteamError::GlobalUserUnavailable { app_id: 3527290 }.to_string();
        let reason = WorkerProtocolError::WorkerError {
            stage: WorkerErrorStage::GlobalUserUnavailable,
            message,
        }
        .to_string();
        assert!(
            !looks_like_steam_died(&reason),
            "a single game that Steam declines a session for must not flip global connectivity: {reason:?}"
        );
    }

    #[test]
    fn genuine_dead_steam_reasons_still_classify() {
        assert!(looks_like_steam_died(
            "Steam client is not running. Please start Steam and try again."
        ));
        assert!(looks_like_steam_died(
            "worker error: Connect: could not locate steamclient.so"
        ));
        assert!(looks_like_steam_died("worker killed by signal 9 (SIGKILL)"));
    }
}
