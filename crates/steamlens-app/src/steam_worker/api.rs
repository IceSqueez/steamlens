use std::collections::HashMap;

use iced::Task;
use tokio::sync::mpsc;

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
    request_tx: mpsc::UnboundedSender<SteamRequest>,
}

impl SteamWorker {
    pub fn spawn(reply_tx: mpsc::UnboundedSender<SteamReply>) -> Self {
        let (req_tx, req_rx) = mpsc::unbounded_channel::<SteamRequest>();
        tokio::spawn(bridge_loop(req_rx, reply_tx));
        SteamWorker { request_tx: req_tx }
    }

    pub fn dispatch<M: 'static + Send>(&self, req: SteamRequest, noop: M) -> Task<M> {
        let tx = self.request_tx.clone();
        Task::future(async move {
            let _ = tx.send(req);
            noop
        })
    }

    pub fn dispatch_checked<M: 'static + Send>(
        &self,
        req: SteamRequest,
        steam_running: bool,
        user_logged_in: bool,
        noop: M,
    ) -> Result<Task<M>, crate::worker_subprocess::ConnectivityError> {
        crate::worker_subprocess::preflight(steam_running, user_logged_in)?;
        Ok(self.dispatch(req, noop))
    }
}
