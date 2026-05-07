use steamlens_core::ipc::{WorkerResponse, decode_frame, parse_header};
use tokio::io::AsyncReadExt;
use tokio::process::ChildStdout;

pub async fn read_response(stdout: &mut ChildStdout) -> Option<WorkerResponse> {
    let mut header = [0u8; 4];
    stdout.read_exact(&mut header).await.ok()?;
    let len = parse_header(header).ok()?;
    let mut buf = vec![0u8; len];
    stdout.read_exact(&mut buf).await.ok()?;
    decode_frame::<WorkerResponse>(&buf).ok()
}
