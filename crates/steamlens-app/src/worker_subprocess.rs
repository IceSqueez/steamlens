use std::process::ExitStatus;

use steamlens_core::ipc::WorkerErrorKind;
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::task::JoinHandle;

/// Lifecycle mode for a spawned worker child.
#[derive(Debug, Clone, Copy)]
pub enum WorkerMode {
    /// Short-lived single-operation scan, kill-on-timeout.
    /// Drainer task forwards each stderr line AND captures into a buffer returned by `finish()`.
    OneShot,
}

/// Error variants covering all failure modes during worker spawn.
#[derive(Debug, thiserror::Error)]
pub enum WorkerSpawnError {
    #[error("could not resolve current executable: {0}")]
    ExeNotFound(std::io::Error),

    #[error("worker child spawn failed: {0}")]
    SpawnFailed(std::io::Error),

    #[error("spawned worker has no pid")]
    NoChildPid,

    #[error("kill-on-parent-exit guard failed: {0}")]
    LifetimeGuardFailed(std::io::Error),

    #[error("child stdin pipe unavailable after spawn")]
    StdinUnavailable,

    #[error("child stdout pipe unavailable after spawn")]
    StdoutUnavailable,

    /// Not in RFC-006 §Public API Surface; added because stderr is always piped
    /// and a missing pipe is the same class of failure as missing stdin/stdout.
    #[error("child stderr pipe unavailable after spawn")]
    StderrUnavailable,
}

/// Typed error for in-session protocol failures across both interactive and one-shot scan paths.
#[derive(Debug, thiserror::Error)]
pub enum WorkerProtocolError {
    #[error("worker error: {kind:?}: {message}")]
    WorkerError {
        kind: WorkerErrorKind,
        message: String,
    },

    #[error("unexpected response variant")]
    UnexpectedMessage,

    #[error("operation timed out")]
    Timeout,

    #[error("child stdout closed before protocol completion")]
    UnexpectedEof,

    #[error("frame decode failed: {0}")]
    Decode(std::io::Error),

    #[error("write to child stdin failed: {0}")]
    Write(std::io::Error),
}

/// Unified handle to a spawned `--worker <app_id>` child process.
///
/// Call `finish` exactly once when done with the handle. `finish` is mode-aware:
/// it either drains stderr (OneShot) or sends a graceful Shutdown command (Interactive)
/// before returning the exit status and any captured stderr bytes.
pub struct WorkerHandle {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr_task: Option<JoinHandle<Vec<u8>>>,
    _guard: steamlens_core::ChildLifetimeGuard,
}

impl WorkerHandle {
    /// Spawn a `--worker <app_id>` child in the given mode.
    pub async fn spawn(app_id: u32, _mode: WorkerMode) -> Result<Self, WorkerSpawnError> {
        let exe = std::env::current_exe().map_err(WorkerSpawnError::ExeNotFound)?;
        let mut child = tokio::process::Command::new(exe)
            .arg("--worker")
            .arg(app_id.to_string())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(WorkerSpawnError::SpawnFailed)?;

        let pid = child.id().ok_or(WorkerSpawnError::NoChildPid)?;
        let guard = steamlens_core::associate_kill_on_parent_exit(pid)
            .inspect_err(|_| {
                let _ = child.start_kill();
            })
            .map_err(WorkerSpawnError::LifetimeGuardFailed)?;

        let stdin = child
            .stdin
            .take()
            .ok_or(WorkerSpawnError::StdinUnavailable)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(WorkerSpawnError::StdoutUnavailable)?;
        let stderr = child
            .stderr
            .take()
            .ok_or(WorkerSpawnError::StderrUnavailable)?;

        let stderr_task = tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut buf = Vec::new();
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                crate::log!("worker[app_id={app_id}] stderr: {line}");
                buf.extend_from_slice(line.as_bytes());
                buf.push(b'\n');
            }
            buf
        });

        Ok(Self {
            child,
            stdin,
            stdout,
            stderr_task: Some(stderr_task),
            _guard: guard,
        })
    }

    /// Send a command frame to the worker's stdin.
    pub async fn send(
        &mut self,
        cmd: &steamlens_core::ipc::WorkerCommand,
    ) -> Result<(), WorkerProtocolError> {
        crate::ipc_pipe::write_command(&mut self.stdin, cmd)
            .await
            .map_err(WorkerProtocolError::Write)
    }

    /// Read one response frame from the worker's stdout.
    ///
    /// Returns `None` on EOF or decode failure; callers that treat EOF as an error
    /// should map `None` to `WorkerProtocolError::UnexpectedEof`.
    pub async fn recv(
        &mut self,
    ) -> Result<Option<steamlens_core::ipc::WorkerResponse>, WorkerProtocolError> {
        Ok(crate::ipc_pipe::read_response(&mut self.stdout).await)
    }

    /// Kills the child immediately and drains the stderr capture buffer within `STDERR_DRAIN`.
    ///
    /// Returns `(Some(exit_status), Some(stderr_bytes))`. Exit status is `None` only if the
    /// `CHILD_KILL` timeout elapsed before the child exited.
    pub async fn finish(mut self) -> (Option<ExitStatus>, Option<Vec<u8>>) {
        let _ = self.child.start_kill();
        let status = tokio::time::timeout(crate::timeouts::CHILD_KILL, self.child.wait())
            .await
            .ok()
            .and_then(|r| r.ok());
        let bytes = match self.stderr_task {
            Some(task) => tokio::time::timeout(crate::timeouts::STDERR_DRAIN, task)
                .await
                .ok()
                .and_then(|r| r.ok()),
            None => None,
        };
        (status, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_protocol_error_display_includes_kind_and_message() {
        let err = WorkerProtocolError::WorkerError {
            kind: WorkerErrorKind::Connect,
            message: "no pipe".to_owned(),
        };
        let s = format!("{err}");
        assert!(s.contains("Connect"));
        assert!(s.contains("no pipe"));
    }

    #[test]
    fn worker_protocol_error_variants_constructible() {
        let _: WorkerProtocolError = WorkerProtocolError::UnexpectedMessage;
        let _: WorkerProtocolError = WorkerProtocolError::Timeout;
        let _: WorkerProtocolError = WorkerProtocolError::UnexpectedEof;
        let _: WorkerProtocolError = WorkerProtocolError::Decode(std::io::Error::other("x"));
        let _: WorkerProtocolError = WorkerProtocolError::Write(std::io::Error::other("y"));
    }

    #[test]
    fn worker_mode_variants_copy() {
        let a = WorkerMode::OneShot;
        let _a2 = a;
        let _ = a;
    }

    #[test]
    fn worker_spawn_error_display() {
        let variants: &[(&str, WorkerSpawnError)] = &[
            (
                "executable",
                WorkerSpawnError::ExeNotFound(std::io::Error::other("no exe")),
            ),
            (
                "spawn failed",
                WorkerSpawnError::SpawnFailed(std::io::Error::other("denied")),
            ),
            ("no pid", WorkerSpawnError::NoChildPid),
            (
                "guard failed",
                WorkerSpawnError::LifetimeGuardFailed(std::io::Error::other("guard")),
            ),
            ("stdin", WorkerSpawnError::StdinUnavailable),
            ("stdout", WorkerSpawnError::StdoutUnavailable),
            ("stderr", WorkerSpawnError::StderrUnavailable),
        ];
        for (needle, err) in variants {
            let s = format!("{err}");
            assert!(
                s.to_lowercase().contains(needle),
                "expected {needle:?} in {s:?}"
            );
        }
    }
}
