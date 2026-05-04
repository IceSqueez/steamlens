use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use steamlens_core::{Client, GameSummary, StatKind, SteamCallback};

use crate::manager::types::{AchievementData, AchievementIcon, ResetScope, StatData, StatValue};

fn try_resolve_icon(c: &Client, icon_handle: i32) -> Option<AchievementIcon> {
    if icon_handle == 0 {
        return None;
    }
    c.get_image(icon_handle)
        .ok()
        .flatten()
        .map(|img| AchievementIcon {
            width: img.width,
            height: img.height,
            rgba: img.rgba,
        })
}

fn forward_icon_callbacks(
    callbacks: Vec<SteamCallback>,
    c: &Client,
    tx: &mpsc::Sender<SteamReply>,
) {
    for cb in callbacks {
        match cb {
            SteamCallback::UserAchievementIconFetched {
                achievement_name,
                icon_handle,
                ..
            } => {
                if let Some(icon) = try_resolve_icon(c, icon_handle) {
                    let _ = tx.send(SteamReply::IconUpdated {
                        name: achievement_name,
                        icon,
                    });
                }
            }
            other => {
                let _ = tx.send(SteamReply::Callback(other));
            }
        }
    }
}

#[allow(dead_code)]
pub enum SteamRequest {
    ConnectWithApp(u32),
    ScanLibrary,
    RequestUserStats,
    ApplyChanges {
        achievements_to_set: Vec<String>,
        achievements_to_clear: Vec<String>,
        stats_int: HashMap<String, i32>,
        stats_float: HashMap<String, f32>,
    },
    ResetAll {
        scope: ResetScope,
        stat_driven_progress_max: HashMap<String, u32>,
    },
    Disconnect,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SteamReply {
    Connected {
        steam_id: u64,
        app_name: Option<String>,
    },
    ConnectFailed(String),
    LibraryScan(Vec<GameSummary>),
    LibraryScanFailed(String),
    StatsRequested,
    RequestStatsFailed(String),
    AchievementsAndStats {
        achievements: Vec<AchievementData>,
        stats: Vec<StatData>,
    },
    LoadFailed(String),
    ChangesSaved,
    SaveFailed(String),
    ResetDone,
    ResetFailed(String),
    IconUpdated {
        name: String,
        icon: AchievementIcon,
    },
    Callback(SteamCallback),
    Disconnected,
}

pub struct SteamWorker {
    sender: mpsc::SyncSender<SteamRequest>,
}

impl SteamWorker {
    pub fn spawn() -> (Self, mpsc::Receiver<SteamReply>) {
        let (req_tx, req_rx) = mpsc::sync_channel::<SteamRequest>(64);
        let (rep_tx, rep_rx) = mpsc::channel::<SteamReply>();

        thread::Builder::new()
            .name("steam-worker".into())
            .spawn(move || worker_loop(req_rx, rep_tx))
            .expect("failed to spawn steam-worker thread");

        (SteamWorker { sender: req_tx }, rep_rx)
    }

    pub fn send(&self, req: SteamRequest) {
        let _ = self.sender.try_send(req);
    }

    #[cfg(test)]
    pub fn new_disconnected() -> Self {
        let (req_tx, _req_rx) = mpsc::sync_channel::<SteamRequest>(1);
        SteamWorker { sender: req_tx }
    }
}

fn send_reply(tx: &mpsc::Sender<SteamReply>, reply: SteamReply) {
    let _ = tx.send(reply);
}

fn poll_idle_callbacks(client: &Option<Client>, tx: &mpsc::Sender<SteamReply>) {
    let Some(c) = client else { return };
    if let Ok(callbacks) = c.poll_callbacks() {
        forward_icon_callbacks(callbacks, c, tx);
    }
}

fn worker_loop(rx: mpsc::Receiver<SteamRequest>, tx: mpsc::Sender<SteamReply>) {
    let mut client: Option<Client> = None;
    let mut connected_app_id: Option<u32> = None;

    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(req) => match req {
                SteamRequest::ScanLibrary => match steamlens_core::scan_installed_games() {
                    Ok(games) => send_reply(&tx, SteamReply::LibraryScan(games)),
                    Err(e) => send_reply(&tx, SteamReply::LibraryScanFailed(e.to_string())),
                },

                SteamRequest::ConnectWithApp(app_id) => {
                    client = None;
                    connected_app_id = None;
                    match steamlens_core::connect(app_id) {
                        Ok(c) => {
                            let steam_id = c.steam_id();
                            let app_name = c.app_name();
                            connected_app_id = Some(app_id);
                            client = Some(c);
                            send_reply(&tx, SteamReply::Connected { steam_id, app_name });
                        }
                        Err(e) => send_reply(&tx, SteamReply::ConnectFailed(e.to_string())),
                    }
                }

                SteamRequest::RequestUserStats => {
                    let Some(c) = &client else {
                        send_reply(
                            &tx,
                            SteamReply::RequestStatsFailed("Not connected".to_owned()),
                        );
                        continue;
                    };
                    let app_id = connected_app_id.unwrap_or(0);
                    let steam_id = c.steam_id();
                    match c.user_stats().request_user_stats(steam_id) {
                        Ok(()) => {
                            send_reply(&tx, SteamReply::StatsRequested);
                            wait_for_stats_then_load(c, app_id, &tx);
                        }
                        Err(e) => send_reply(&tx, SteamReply::RequestStatsFailed(e.to_string())),
                    }
                }

                SteamRequest::ApplyChanges {
                    achievements_to_set,
                    achievements_to_clear,
                    stats_int,
                    stats_float,
                } => {
                    let Some(c) = &client else {
                        send_reply(&tx, SteamReply::SaveFailed("Not connected".to_owned()));
                        continue;
                    };
                    let stats_iface = c.user_stats();

                    let stage_result = (|| -> Result<(), String> {
                        for name in &achievements_to_set {
                            stats_iface
                                .set_achievement(name)
                                .map_err(|e| format!("SetAchievement({name}): {e}"))?;
                        }
                        for name in &achievements_to_clear {
                            stats_iface
                                .clear_achievement(name)
                                .map_err(|e| format!("ClearAchievement({name}): {e}"))?;
                        }
                        for (name, val) in &stats_int {
                            stats_iface
                                .set_stat_int(name, *val)
                                .map_err(|e| format!("SetStatInt({name}): {e}"))?;
                        }
                        for (name, val) in &stats_float {
                            stats_iface
                                .set_stat_float(name, *val)
                                .map_err(|e| format!("SetStatFloat({name}): {e}"))?;
                        }
                        stats_iface
                            .store_stats()
                            .map_err(|e| format!("StoreStats: {e}"))?;
                        Ok(())
                    })();

                    if let Err(e) = stage_result {
                        send_reply(&tx, SteamReply::SaveFailed(e));
                        continue;
                    }

                    let app_id = connected_app_id.unwrap_or(0);
                    let saved = wait_for_store_callback(c, &tx);
                    if saved {
                        load_achievements_and_stats(c, app_id, &tx);
                    } else {
                        send_reply(
                            &tx,
                            SteamReply::SaveFailed(
                                "Timed out waiting for StoreStats confirmation".to_owned(),
                            ),
                        );
                    }
                }

                SteamRequest::ResetAll {
                    scope,
                    stat_driven_progress_max,
                } => {
                    let Some(c) = &client else {
                        send_reply(&tx, SteamReply::ResetFailed("Not connected".to_owned()));
                        continue;
                    };
                    let stats_iface = c.user_stats();
                    let achievements_too = scope == ResetScope::StatsAndAchievements;
                    if let Err(e) = stats_iface.reset_all_stats(achievements_too) {
                        send_reply(&tx, SteamReply::ResetFailed(format!("ResetAllStats: {e}")));
                        continue;
                    }
                    if let Err(e) = stats_iface.store_stats() {
                        send_reply(
                            &tx,
                            SteamReply::ResetFailed(format!("StoreStats after reset: {e}")),
                        );
                        continue;
                    }
                    let stored = poll_until_store_confirmed(c, &tx);
                    if !stored {
                        send_reply(
                            &tx,
                            SteamReply::ResetFailed(
                                "Timed out waiting for StoreStats confirmation after reset"
                                    .to_owned(),
                            ),
                        );
                        continue;
                    }
                    let _ = stat_driven_progress_max;
                    send_reply(&tx, SteamReply::ResetDone);
                }

                SteamRequest::Disconnect => {
                    drop(client.take());
                    let _ = connected_app_id.take();
                    send_reply(&tx, SteamReply::Disconnected);
                    break;
                }
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                poll_idle_callbacks(&client, &tx);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn load_achievements_and_stats(c: &Client, app_id: u32, tx: &mpsc::Sender<SteamReply>) {
    let stats_iface = c.user_stats();

    let num = match stats_iface.num_achievements() {
        Ok(n) => n,
        Err(e) => {
            send_reply(tx, SteamReply::LoadFailed(e.to_string()));
            return;
        }
    };

    let mut achievements = Vec::with_capacity(num as usize);
    for i in 0..num {
        let id = match stats_iface.achievement_name(i) {
            Ok(n) => n,
            Err(_) => continue,
        };
        let display_name = stats_iface
            .achievement_display_attribute(&id, "name")
            .unwrap_or_else(|_| id.clone());
        let description = stats_iface
            .achievement_display_attribute(&id, "desc")
            .unwrap_or_default();
        let hidden_str = stats_iface
            .achievement_display_attribute(&id, "hidden")
            .unwrap_or_default();
        let is_hidden = hidden_str.trim() == "1";
        let (is_achieved, unlock_time) = stats_iface
            .achievement_and_unlock_time(&id)
            .unwrap_or((false, 0));
        let unlock_time = if unlock_time == 0 {
            None
        } else {
            Some(unlock_time)
        };

        let handle = stats_iface.achievement_icon(&id).unwrap_or(0);
        let icon = if handle == 0 {
            None
        } else {
            c.get_image(handle)
                .ok()
                .flatten()
                .map(|img| AchievementIcon {
                    width: img.width,
                    height: img.height,
                    rgba: img.rgba,
                })
        };

        achievements.push(AchievementData {
            id,
            display_name,
            description,
            is_hidden,
            is_achieved,
            unlock_time,
            permission: 0,
            icon,
        });
    }

    let descriptors = c.stat_descriptors(app_id).unwrap_or_default();

    let mut stats = Vec::with_capacity(descriptors.len());
    for desc in descriptors {
        let value = match desc.kind {
            StatKind::Int => {
                let v = stats_iface.get_stat_int(&desc.name).unwrap_or(0);
                StatValue::Int(v)
            }
            StatKind::Float => {
                let v = stats_iface.get_stat_float(&desc.name).unwrap_or(0.0);
                StatValue::Float(v)
            }
        };
        stats.push(StatData {
            display_name: desc.name.clone(),
            id: desc.name,
            value,
            original_value: value,
            max_value: desc.max_value,
            min_value: desc.min_value,
            default_value: desc.default_value,
            is_increment_only: false,
            permission: 0,
        });
    }

    send_reply(
        tx,
        SteamReply::AchievementsAndStats {
            achievements,
            stats,
        },
    );
}

fn route_non_stats_callback(cb: SteamCallback, c: &Client, tx: &mpsc::Sender<SteamReply>) {
    match cb {
        SteamCallback::UserAchievementIconFetched {
            achievement_name,
            icon_handle,
            ..
        } => {
            if let Some(icon) = try_resolve_icon(c, icon_handle) {
                let _ = tx.send(SteamReply::IconUpdated {
                    name: achievement_name,
                    icon,
                });
            }
        }
        other => send_reply(tx, SteamReply::Callback(other)),
    }
}

fn wait_for_stats_then_load(c: &Client, app_id: u32, tx: &mpsc::Sender<SteamReply>) {
    let expected_user = c.steam_id();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        match c.poll_callbacks() {
            Ok(callbacks) => {
                for cb in callbacks {
                    match &cb {
                        SteamCallback::UserStatsReceived {
                            result,
                            user_steam_id,
                            ..
                        } if *user_steam_id == expected_user => {
                            if result.is_ok() {
                                load_achievements_and_stats(c, app_id, tx);
                            } else {
                                send_reply(
                                    tx,
                                    SteamReply::LoadFailed(format!(
                                        "UserStatsReceived error: {}",
                                        result.raw()
                                    )),
                                );
                            }
                            return;
                        }
                        _ => route_non_stats_callback(cb, c, tx),
                    }
                }
            }
            Err(e) => {
                send_reply(tx, SteamReply::LoadFailed(format!("poll_callbacks: {e}")));
                return;
            }
        }
        if std::time::Instant::now() >= deadline {
            send_reply(
                tx,
                SteamReply::LoadFailed(
                    "Timed out waiting for UserStatsReceived callback".to_owned(),
                ),
            );
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn wait_for_store_callback(c: &Client, tx: &mpsc::Sender<SteamReply>) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match c.poll_callbacks() {
            Ok(callbacks) => {
                for cb in callbacks {
                    match &cb {
                        SteamCallback::UserStatsStored { result, .. } => {
                            if result.is_ok() {
                                send_reply(tx, SteamReply::ChangesSaved);
                            } else {
                                send_reply(
                                    tx,
                                    SteamReply::SaveFailed(format!(
                                        "UserStatsStored error: {}",
                                        result.raw()
                                    )),
                                );
                            }
                            return true;
                        }
                        _ => route_non_stats_callback(cb, c, tx),
                    }
                }
            }
            Err(e) => {
                send_reply(tx, SteamReply::SaveFailed(format!("poll_callbacks: {e}")));
                return true;
            }
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn poll_until_store_confirmed(c: &Client, tx: &mpsc::Sender<SteamReply>) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match c.poll_callbacks() {
            Ok(callbacks) => {
                for cb in callbacks {
                    if let SteamCallback::UserStatsStored { result, .. } = &cb {
                        return result.is_ok();
                    }
                    route_non_stats_callback(cb, c, tx);
                }
            }
            Err(_) => return false,
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(50));
    }
}
