use iced::Task;

use crate::timeouts;

pub fn min_splash_wait() -> Task<crate::Message> {
    Task::perform(
        async { tokio::time::sleep(std::time::Duration::from_millis(750)).await },
        |_| crate::Message::SplashMinElapsed,
    )
}

pub fn probe_steam_boot() -> Task<crate::Message> {
    Task::perform(
        async {
            steamlens_core::probe_steam(timeouts::PROBE_STEAM_BOOT)
                .await
                .map_err(|e| e.to_string())
        },
        crate::Message::ProbeResult,
    )
}

pub fn probe_steam_reconnect() -> Task<crate::Message> {
    Task::perform(
        async {
            steamlens_core::probe_steam(timeouts::PROBE_STEAM_RECONNECT)
                .await
                .map_err(|e| e.to_string())
        },
        crate::Message::ProbeResult,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_splash_wait_builds() {
        let _: Task<crate::Message> = min_splash_wait();
    }

    #[test]
    fn probe_steam_boot_builds() {
        let _: Task<crate::Message> = probe_steam_boot();
    }

    #[test]
    fn probe_steam_reconnect_builds() {
        let _: Task<crate::Message> = probe_steam_reconnect();
    }
}
