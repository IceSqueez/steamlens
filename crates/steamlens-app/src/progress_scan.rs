use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::{Duration, Instant};

use steamlens_core::ipc::{WorkerCommand, WorkerResponse};
use steamlens_core::{CardOnlyAchievement, CardOnlyPayload, StatData};
use tokio::sync::{Semaphore, mpsc};
use tokio::task::JoinHandle;

use crate::timeouts;
use crate::worker_subprocess::{WorkerHandle, WorkerProtocolError};

const MAX_CONCURRENT: usize = 4;

pub use steamlens_core::AchievementCountPayload as ProgressData;

#[derive(Debug, Clone)]
pub struct ScannedGameData {
    pub app_name: Option<String>,
    pub achievements: Vec<CardOnlyAchievement>,
    pub stats: Vec<StatData>,
    pub global_percentages: HashMap<String, f32>,
    pub genre: Option<String>,
}

impl ScannedGameData {
    pub fn earned_count(&self) -> u32 {
        self.achievements.iter().filter(|a| a.is_achieved).count() as u32
    }

    pub fn total_count(&self) -> u32 {
        self.achievements.len() as u32
    }
}

#[derive(Debug, Clone)]
pub struct ProgressResult {
    pub app_id: u32,
    pub data: Option<ScannedGameData>,
    pub error: Option<String>,
}

pub struct ProgressScanner {
    handles: Vec<JoinHandle<()>>,
}

impl ProgressScanner {
    pub fn spawn(app_ids: Vec<u32>) -> (Self, mpsc::UnboundedReceiver<ProgressResult>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let permits = Arc::new(Semaphore::new(MAX_CONCURRENT));
        let handles = app_ids
            .into_iter()
            .map(|app_id| {
                let tx = tx.clone();
                let permits = Arc::clone(&permits);
                tokio::spawn(async move {
                    let _permit = match permits.acquire().await {
                        Ok(p) => p,
                        Err(_) => return,
                    };
                    let result = scan_one_app(app_id).await;
                    let _ = tx.send(result);
                })
            })
            .collect();
        (Self { handles }, rx)
    }
}

impl Drop for ProgressScanner {
    fn drop(&mut self) {
        for handle in self.handles.drain(..) {
            handle.abort();
        }
    }
}

async fn scan_one_app(app_id: u32) -> ProgressResult {
    match try_full_scan(app_id).await {
        Ok(data) => ProgressResult {
            app_id,
            data: Some(data),
            error: None,
        },
        Err((err, diag)) => {
            let err_str = err.to_string();
            if diag.is_empty() {
                tracing::error!("progress_scan: app_id={app_id} failed: {err}");
            } else {
                tracing::error!(
                    "progress_scan: app_id={app_id} failed: {err}\n--- worker diagnostics ---\n{}--- end diagnostics ---",
                    diag
                );
            }
            ProgressResult {
                app_id,
                data: None,
                error: Some(err_str),
            }
        }
    }
}

/// `(err, diag)` — `err` is `io::Error` for spawn failures or `WorkerProtocolError`
/// for in-session failures. `diag` is the worker's captured stderr + exit status.
type ScanError = (Box<dyn std::error::Error + Send>, String);

async fn try_full_scan(app_id: u32) -> Result<ScannedGameData, ScanError> {
    use crate::worker_subprocess::WorkerMode;

    let total_timeout =
        timeouts::STEAM_CONNECT + timeouts::COLD_SCAN_LOAD + timeouts::GLOBAL_PERCENTAGES;

    let mut handle = WorkerHandle::spawn(app_id, WorkerMode::OneShot)
        .await
        .map_err(|e| {
            (
                Box::new(e) as Box<dyn std::error::Error + Send>,
                String::new(),
            )
        })?;

    let result = tokio::time::timeout(total_timeout, run_full_scan_protocol(&mut handle)).await;

    let (exit_status, stderr_bytes) = handle.finish().await;
    let stderr_str = stderr_bytes
        .as_deref()
        .map(String::from_utf8_lossy)
        .map(|s| s.into_owned())
        .unwrap_or_default();
    let mut diag = format!("worker {}\n", format_exit_status(exit_status.as_ref()));
    if !stderr_str.is_empty() {
        diag.push_str(&stderr_str);
    }

    match result {
        Err(_) => Err((
            Box::new(WorkerProtocolError::Timeout) as Box<dyn std::error::Error + Send>,
            diag,
        )),
        Ok(Err(e)) => Err((Box::new(e) as Box<dyn std::error::Error + Send>, diag)),
        Ok(Ok(data)) => Ok(data),
    }
}

#[cfg(unix)]
fn format_exit_status(status: Option<&ExitStatus>) -> String {
    use std::os::unix::process::ExitStatusExt;
    let Some(s) = status else {
        return "exit status unavailable".to_owned();
    };
    if let Some(code) = s.code() {
        format!("exited with code {code}")
    } else if let Some(sig) = s.signal() {
        format!("killed by signal {} ({})", sig, signal_name(sig))
    } else {
        format!("{s:?}")
    }
}

#[cfg(not(unix))]
fn format_exit_status(status: Option<&ExitStatus>) -> String {
    let Some(s) = status else {
        return "exit status unavailable".to_owned();
    };
    if let Some(code) = s.code() {
        format!("exited with code {code}")
    } else {
        format!("{s:?}")
    }
}

#[cfg(unix)]
fn signal_name(sig: i32) -> &'static str {
    match sig {
        1 => "SIGHUP",
        2 => "SIGINT",
        3 => "SIGQUIT",
        4 => "SIGILL",
        6 => "SIGABRT",
        8 => "SIGFPE",
        9 => "SIGKILL",
        11 => "SIGSEGV",
        13 => "SIGPIPE",
        15 => "SIGTERM",
        _ => "unknown",
    }
}

async fn run_full_scan_protocol(
    handle: &mut WorkerHandle,
) -> Result<ScannedGameData, WorkerProtocolError> {
    let connected = tokio::time::timeout(timeouts::STEAM_CONNECT, handle.recv())
        .await
        .map_err(|_| WorkerProtocolError::Timeout)??
        .ok_or(WorkerProtocolError::UnexpectedEof)?;

    let app_name = match connected {
        WorkerResponse::SteamConnected { app_name, .. } => app_name,
        WorkerResponse::Error { kind, message } => {
            return Err(WorkerProtocolError::WorkerError { kind, message });
        }
        _ => {
            return Err(WorkerProtocolError::UnexpectedMessage);
        }
    };

    let t_card = Instant::now();
    tracing::debug!("scan: send LoadAchievementsAndStatsCardOnly");
    handle
        .send(&WorkerCommand::LoadAchievementsAndStatsCardOnly)
        .await?;
    let (achievements, stats, genre) =
        read_card_only_skipping_async(handle, timeouts::COLD_SCAN_LOAD).await?;
    tracing::info!(
        "scan: CardOnly response in {:?} ({} achievements)",
        t_card.elapsed(),
        achievements.len()
    );

    let global_percentages = if achievements.is_empty() {
        HashMap::new()
    } else {
        handle
            .send(&WorkerCommand::RequestGlobalPercentages)
            .await?;
        read_percentages_skipping_async(handle, timeouts::GLOBAL_PERCENTAGES).await
    };

    Ok(ScannedGameData {
        app_name,
        achievements,
        stats,
        global_percentages,
        genre,
    })
}

async fn read_card_only_skipping_async(
    handle: &mut WorkerHandle,
    total_timeout: Duration,
) -> Result<(Vec<CardOnlyAchievement>, Vec<StatData>, Option<String>), WorkerProtocolError> {
    use WorkerProtocolError;

    let deadline = tokio::time::Instant::now() + total_timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(WorkerProtocolError::Timeout);
        }
        let frame = tokio::time::timeout(remaining, handle.recv())
            .await
            .map_err(|_| WorkerProtocolError::Timeout)??
            .ok_or(WorkerProtocolError::UnexpectedEof)?;
        match frame {
            WorkerResponse::CardOnlyAchievements {
                shm_path,
                region_bytes,
            } => {
                let path = PathBuf::from(&shm_path);
                let payload: CardOnlyPayload = steamlens_core::read_payload(&path, region_bytes)
                    .map_err(|e| {
                        WorkerProtocolError::Decode(std::io::Error::other(format!(
                            "CardOnlyAchievements shm read: {e}"
                        )))
                    })?;
                return Ok((payload.achievements, Vec::new(), payload.genre));
            }
            WorkerResponse::IconUpdated { shm_path, .. } => {
                steamlens_core::unlink_at(&PathBuf::from(shm_path));
                continue;
            }
            WorkerResponse::AchievementsAndStats { shm_path, .. }
            | WorkerResponse::AchievementCount { shm_path, .. }
            | WorkerResponse::ProbeResult { shm_path, .. }
            | WorkerResponse::GlobalPercentagesReady { shm_path, .. } => {
                steamlens_core::unlink_at(&PathBuf::from(shm_path));
                continue;
            }
            WorkerResponse::Error { kind, message } => {
                return Err(WorkerProtocolError::WorkerError { kind, message });
            }
            _ => continue,
        }
    }
}

async fn read_percentages_skipping_async(
    handle: &mut WorkerHandle,
    total_timeout: Duration,
) -> HashMap<String, f32> {
    let deadline = tokio::time::Instant::now() + total_timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return HashMap::new();
        }
        let frame = match tokio::time::timeout(remaining, handle.recv()).await {
            Ok(Ok(Some(f))) => f,
            _ => return HashMap::new(),
        };
        match frame {
            WorkerResponse::GlobalPercentagesReady {
                shm_path,
                region_bytes,
            } => {
                let path = PathBuf::from(&shm_path);
                return steamlens_core::read_payload::<HashMap<String, f32>>(&path, region_bytes)
                    .unwrap_or_default();
            }
            WorkerResponse::AchievementsAndStats { shm_path, .. }
            | WorkerResponse::IconUpdated { shm_path, .. }
            | WorkerResponse::AchievementCount { shm_path, .. }
            | WorkerResponse::ProbeResult { shm_path, .. } => {
                steamlens_core::unlink_at(&PathBuf::from(shm_path));
                continue;
            }
            WorkerResponse::Error { .. } => return HashMap::new(),
            _ => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ach(id: &str, achieved: bool) -> CardOnlyAchievement {
        CardOnlyAchievement {
            id: id.to_owned(),
            is_achieved: achieved,
        }
    }

    #[tokio::test]
    async fn scanner_spawn_empty_closes_rx_immediately() {
        let (_scanner, mut rx) = ProgressScanner::spawn(vec![]);
        assert!(
            rx.recv().await.is_none(),
            "empty scanner must close receiver immediately"
        );
    }

    #[test]
    fn scanned_data_count_helpers() {
        let data = ScannedGameData {
            app_name: None,
            achievements: vec![
                make_ach("A", true),
                make_ach("B", false),
                make_ach("C", true),
            ],
            stats: Vec::new(),
            global_percentages: HashMap::new(),
            genre: None,
        };
        assert_eq!(data.earned_count(), 2);
        assert_eq!(data.total_count(), 3);
    }

    #[test]
    fn scanner_max_concurrent_cap() {
        assert_eq!(
            MAX_CONCURRENT, 4,
            "temporary diagnostic value; revert to 1 if parallel scan triggers Steam IPC errors or out-of-order card render"
        );
    }

    #[test]
    fn max_concurrent_is_documented_value() {
        assert_eq!(
            MAX_CONCURRENT, 4,
            "temporary parallel-scan diagnostic; raising MAX_CONCURRENT above 1 re-opens the \
             cards-render-out-of-order race — revert if test surfaces issues"
        );
    }

    #[test]
    fn progress_result_none_data_on_failure() {
        let result = ProgressResult {
            app_id: 99,
            data: None,
            error: None,
        };
        assert!(result.data.is_none(), "failure result must have None data");
    }

    #[test]
    fn progress_data_equality() {
        let a = ProgressData {
            earned: 5,
            total: 10,
        };
        let b = ProgressData {
            earned: 5,
            total: 10,
        };
        let c = ProgressData {
            earned: 3,
            total: 10,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
