use std::io::ErrorKind;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use steamlens_core::ipc::{
    WorkerCommand, WorkerResponse, decode_frame, encode_frame, parse_header,
};

use super::error::WorkerError;

pub(super) async fn read_command(
    stdin: &mut (impl AsyncReadExt + Unpin),
) -> Result<Option<WorkerCommand>, WorkerError> {
    let mut header = [0u8; 4];
    match stdin.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(WorkerError::Io(e)),
    }
    let len = parse_header(header)?;
    let mut buf = vec![0u8; len];
    stdin.read_exact(&mut buf).await?;
    let cmd = decode_frame::<WorkerCommand>(&buf)?;
    Ok(Some(cmd))
}

pub(super) async fn write_response(msg: &WorkerResponse) -> Result<(), WorkerError> {
    let framed = match encode_frame(msg) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("write_response: encode_frame failed: {e}");
            return Err(WorkerError::Frame(e));
        }
    };
    let mut stdout = tokio::io::stdout();
    if let Err(e) = stdout.write_all(&framed).await {
        return Err(WorkerError::Io(e));
    }
    stdout.flush().await?;
    Ok(())
}
