use std::collections::HashMap;
use std::sync::mpsc;

use tokio::sync::mpsc as async_mpsc;

use steamlens_core::AchievementIcon;

use super::bridge::bridge_loop;

pub enum SteamRequest {
    ConnectWithApp(u32),
    RequestUserStats,
    RequestGlobalPercentages,
    ApplyChanges {
        achievements_to_set: Vec<String>,
        achievements_to_clear: Vec<String>,
        stats_int: HashMap<String, i32>,
        stats_float: HashMap<String, f32>,
    },
    Disconnect,
}

#[derive(Debug, Clone)]
pub enum SteamReply {
    Connected {
        app_name: Option<String>,
    },
    ConnectFailed(String),
    RequestStatsFailed(String),
    AchievementsAndStats {
        achievements: Vec<steamlens_core::AchievementData>,
        stats: Vec<steamlens_core::StatData>,
    },
    LoadFailed(String),
    ChangesSaved,
    SaveFailed(String),
    IconUpdated {
        name: String,
        icon: AchievementIcon,
    },
    GlobalPercentagesReady(HashMap<String, f32>),
    GlobalPercentagesFailed,
    Disconnected,
}

pub struct SteamWorker {
    request_tx: async_mpsc::UnboundedSender<SteamRequest>,
}

impl SteamWorker {
    pub fn spawn() -> (Self, mpsc::Receiver<SteamReply>) {
        let (req_tx, req_rx) = async_mpsc::unbounded_channel::<SteamRequest>();
        let (rep_tx, rep_rx) = mpsc::channel::<SteamReply>();

        tokio::spawn(bridge_loop(req_rx, rep_tx));

        (SteamWorker { request_tx: req_tx }, rep_rx)
    }

    pub fn send(&self, req: SteamRequest) {
        let _ = self.request_tx.send(req);
    }

    /// Sync because the underlying mpsc send is sync; pre-flight runs before any pipe touch.
    pub fn send_checked(
        &self,
        req: SteamRequest,
        steam_running: bool,
        user_logged_in: bool,
    ) -> Result<(), crate::worker_subprocess::ConnectivityError> {
        crate::worker_subprocess::preflight(steam_running, user_logged_in)?;
        let _ = self.request_tx.send(req);
        Ok(())
    }
}
