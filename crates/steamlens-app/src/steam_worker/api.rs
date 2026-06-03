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
    AchievementsFull {
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
    request_sender: mpsc::UnboundedSender<SteamRequest>,
}

impl SteamWorker {
    pub fn spawn(reply_sender: mpsc::UnboundedSender<SteamReply>) -> Self {
        let (request_sender, request_receiver) = mpsc::unbounded_channel::<SteamRequest>();
        tokio::spawn(bridge_loop(request_receiver, reply_sender));
        SteamWorker { request_sender }
    }

    pub fn dispatch<M: 'static + Send>(&self, request: SteamRequest, message: M) -> Task<M> {
        let request_sender = self.request_sender.clone();
        Task::future(async move {
            let _ = request_sender.send(request);
            message
        })
    }

    pub fn dispatch_checked<M: 'static + Send>(
        &self,
        request: SteamRequest,
        steam_running: bool,
        user_logged_in: bool,
        message: M,
    ) -> Result<Task<M>, crate::worker_subprocess::ConnectivityError> {
        crate::worker_subprocess::preflight(steam_running, user_logged_in)?;
        Ok(self.dispatch(request, message))
    }
}
