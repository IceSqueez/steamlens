use steamlens_core::ipc::WorkerErrorKind;

/// Placeholder; M1 of RFC-006 will fill this with the unified WorkerHandle.
#[expect(dead_code)]
pub struct WorkerHandle;

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
}
