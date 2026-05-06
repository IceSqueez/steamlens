use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::time::Duration;

use thiserror::Error;

use crate::ipc::{WorkerResponse, decode_frame, parse_header};
use crate::library::GameSummary;

#[derive(Debug, Clone)]
pub struct ProbedProfile {
    pub steam_id: u64,
    pub persona_name: String,
    pub avatar_image: Option<Vec<u8>>,
    pub game_summaries: Vec<GameSummary>,
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("Steam is not running")]
    SteamNotRunning,

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
    let exe = std::env::current_exe()?;

    let mut child = Command::new(&exe)
        .arg("--probe")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("child stdout missing"))?;

    let deadline = std::time::Instant::now() + timeout;

    let result = read_one_frame_blocking(stdout, deadline);

    let _ = child.kill();
    let _ = child.wait();

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
            steam_id,
            persona_name,
            avatar_png,
            game_summaries: games,
        } => Ok(ProbedProfile {
            steam_id,
            persona_name,
            avatar_image: avatar_png,
            game_summaries: games,
        }),
        WorkerResponse::Error { message, .. } => {
            if is_not_running_message(&message) {
                Err(ProbeError::SteamNotRunning)
            } else {
                Err(ProbeError::Worker(message))
            }
        }
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

fn is_not_running_message(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("not running")
        || lower.contains("steam not running")
        || lower.contains("createsteampipe")
        || lower.contains("connecttoglobaluser")
        || lower.contains("steampipe returned 0")
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
    fn is_not_running_detects_known_messages() {
        assert!(is_not_running_message(
            "Steam client is not running. Please start Steam and try again."
        ));
        assert!(is_not_running_message("CreateSteamPipe returned 0"));
        assert!(is_not_running_message("ConnectToGlobalUser failed"));
        assert!(!is_not_running_message("GetPersonaName returned null"));
        assert!(!is_not_running_message("timed out"));
    }

    #[test]
    fn probe_child_killed_early_does_not_panic() {
        use std::process::{Command, Stdio};

        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(_) => return,
        };
        let mut child = match Command::new(&exe)
            .arg("--probe")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return,
        };

        let stdout = child.stdout.take().expect("stdout");
        let _ = child.kill();
        let _ = child.wait();

        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        let result = read_one_frame_blocking(stdout, deadline);
        match result {
            Err(ProbeReadError::Io(_) | ProbeReadError::TimedOut | ProbeReadError::Frame(_)) => {}
            Ok(_) => {}
        }
    }
}
