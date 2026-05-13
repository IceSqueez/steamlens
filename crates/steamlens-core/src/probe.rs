use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::time::Duration;

use thiserror::Error;

use crate::ipc::{WorkerErrorKind, WorkerResponse, decode_frame, parse_header};
use crate::library::GameSummary;

#[derive(Debug, Clone)]
pub struct ProbedProfile {
    pub steam_id: u64,
    pub nickname: String,
    pub avatar_image: Option<Vec<u8>>,
    pub game_summaries: Vec<GameSummary>,
    pub steam_level: Option<u32>,
    pub steam_root: Option<std::path::PathBuf>,
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("Steam is not running")]
    SteamNotRunning,

    #[error("Steam is running but the user is not signed in")]
    NotLoggedIn,

    #[error("probe timed out")]
    Timeout,

    #[error("worker process error: {0}")]
    Worker(String),

    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

/// Spawns `current_exe() --probe`, reads one `WorkerResponse` frame
/// from the child's stdout, then kills it. `timeout` bounds the total
/// run (startup + pipe connect + avatar fetch).
pub async fn probe_steam(timeout: Duration) -> Result<ProbedProfile, ProbeError> {
    let exe = crate::process::current_exe_resilient()?;

    let mut child = Command::new(&exe)
        .arg("--probe")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let _job_guard =
        crate::process::associate_kill_on_parent_exit(child.id()).map_err(ProbeError::Io)?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("child stdout missing"))?;
    let stderr = child.stderr.take();

    let stderr_thread = stderr.map(|mut s| {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut buf = Vec::with_capacity(2048);
            let _ = s.read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).into_owned()
        })
    });

    let deadline = std::time::Instant::now() + timeout;

    let result = read_one_frame_blocking(stdout, deadline);

    let _ = child.kill();
    let _ = child.wait();

    if let Some(handle) = stderr_thread
        && let Ok(text) = handle.join()
    {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("ERROR ") {
                tracing::error!(target: "probe", "{rest}");
            } else if let Some(rest) = trimmed.strip_prefix("WARN ") {
                tracing::warn!(target: "probe", "{rest}");
            } else if let Some(rest) = trimmed.strip_prefix("DEBUG ") {
                tracing::debug!(target: "probe", "{rest}");
            } else if let Some(rest) = trimmed.strip_prefix("TRACE ") {
                tracing::trace!(target: "probe", "{rest}");
            } else {
                let msg = trimmed.strip_prefix("INFO ").unwrap_or(trimmed);
                tracing::info!(target: "probe", "{msg}");
            }
        }
    }

    let payload = match result {
        Err(ProbeReadError::TimedOut) => return Err(ProbeError::Timeout),
        Err(ProbeReadError::Io(e)) => return Err(ProbeError::Io(e)),
        Err(ProbeReadError::Frame(msg)) => return Err(ProbeError::Worker(msg)),
        Ok(bytes) => bytes,
    };

    let response: WorkerResponse =
        decode_frame(&payload).map_err(|e| ProbeError::Worker(e.to_string()))?;

    match response {
        WorkerResponse::ProbeResult {
            shm_path,
            region_bytes,
        } => {
            let path = std::path::PathBuf::from(&shm_path);
            let payload: crate::ipc::ProbeResultPayload =
                crate::ipc::shm::read_payload(&path, region_bytes)
                    .map_err(|e| ProbeError::Worker(format!("ProbeResult shm: {e}")))?;
            Ok(ProbedProfile {
                steam_id: payload.steam_id,
                nickname: payload.nickname,
                avatar_image: payload.avatar_png,
                game_summaries: payload.game_summaries,
                steam_level: payload.steam_level,
                steam_root: payload.steam_root,
            })
        }
        WorkerResponse::Error { kind, message } => match kind {
            WorkerErrorKind::Connect => Err(ProbeError::SteamNotRunning),
            WorkerErrorKind::NotLoggedIn => Err(ProbeError::NotLoggedIn),
            _ => Err(ProbeError::Worker(message)),
        },
        other => Err(ProbeError::Worker(format!(
            "unexpected response variant: {other:?}"
        ))),
    }
}

enum ProbeReadError {
    TimedOut,
    Io(io::Error),
    Frame(String),
}

fn read_one_frame_blocking(
    mut reader: impl Read,
    deadline: std::time::Instant,
) -> Result<Vec<u8>, ProbeReadError> {
    let mut header = [0u8; 4];
    read_with_deadline(&mut reader, &mut header, deadline)?;

    let payload_len = parse_header(header).map_err(|e| ProbeReadError::Frame(e.to_string()))?;

    let mut payload = vec![0u8; payload_len];
    read_with_deadline(&mut reader, &mut payload, deadline)?;

    Ok(payload)
}

fn read_with_deadline(
    reader: &mut impl Read,
    buf: &mut [u8],
    deadline: std::time::Instant,
) -> Result<(), ProbeReadError> {
    let mut filled = 0;
    while filled < buf.len() {
        if std::time::Instant::now() >= deadline {
            return Err(ProbeReadError::TimedOut);
        }
        match reader.read(&mut buf[filled..]) {
            Ok(0) => {
                return Err(ProbeReadError::Io(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "child closed stdout before sending a complete frame",
                )));
            }
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(ProbeReadError::Io(e)),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_error_display_not_running() {
        let e = ProbeError::SteamNotRunning;
        assert_eq!(e.to_string(), "Steam is not running");
    }

    #[test]
    fn probe_error_not_logged_in_display() {
        let e = ProbeError::NotLoggedIn;
        let s = format!("{e}");
        assert!(
            s.to_lowercase().contains("logged in") || s.to_lowercase().contains("signed in"),
            "NotLoggedIn display must mention login state, got: {s:?}"
        );
    }

    #[test]
    fn probe_error_display_timeout() {
        let e = ProbeError::Timeout;
        assert_eq!(e.to_string(), "probe timed out");
    }

    #[test]
    fn probe_error_display_worker() {
        let e = ProbeError::Worker("pipe broken".into());
        assert_eq!(e.to_string(), "worker process error: pipe broken");
    }

    #[test]
    fn probe_error_display_io() {
        let e = ProbeError::Io(io::Error::new(io::ErrorKind::NotFound, "no such file"));
        assert!(e.to_string().contains("io error"));
    }

    #[test]
    fn read_one_frame_blocking_eof_before_header_yields_io_error() {
        let empty: &[u8] = &[];
        let deadline = std::time::Instant::now() + Duration::from_millis(50);
        let result = read_one_frame_blocking(empty, deadline);
        assert!(
            matches!(result, Err(ProbeReadError::Io(_))),
            "EOF before any header byte must surface as Io error",
        );
    }

    #[test]
    fn read_one_frame_blocking_partial_header_yields_io_error() {
        let partial: &[u8] = &[0x00, 0x00];
        let deadline = std::time::Instant::now() + Duration::from_millis(50);
        let result = read_one_frame_blocking(partial, deadline);
        assert!(
            matches!(result, Err(ProbeReadError::Io(_))),
            "partial header followed by EOF must surface as Io error",
        );
    }
}
