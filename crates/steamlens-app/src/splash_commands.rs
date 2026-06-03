use std::time::Duration;

use iced::Task;

use crate::ProbeFailure;
use crate::timeouts;

pub fn min_splash_wait() -> Task<crate::Message> {
    Task::perform(
        async { tokio::time::sleep(Duration::from_millis(750)).await },
        |_| crate::Message::SplashMinElapsed,
    )
}

pub fn probe_steam_boot() -> Task<crate::Message> {
    Task::perform(
        async {
            steamlens_core::probe_steam(timeouts::PROBE_STEAM_BOOT)
                .await
                .map(Box::new)
                .map_err(ProbeFailure::from)
        },
        crate::Message::ProbeResult,
    )
}

pub fn probe_steam_reconnect() -> Task<crate::Message> {
    Task::perform(
        async {
            steamlens_core::probe_steam(timeouts::PROBE_STEAM_RECONNECT)
                .await
                .map(Box::new)
                .map_err(ProbeFailure::from)
        },
        crate::Message::ProbeResult,
    )
}
