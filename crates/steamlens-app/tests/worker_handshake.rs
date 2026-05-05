use std::io::{Read, Write};
use std::process::{Command, Stdio};

use steamlens_core::ipc::{
    WorkerCommand, WorkerResponse, decode_frame, encode_frame, parse_header,
};

fn read_one_response(stdout: &mut impl Read) -> WorkerResponse {
    let mut header = [0u8; 4];
    stdout.read_exact(&mut header).expect("read header");
    let len = parse_header(header).expect("parse header");
    let mut payload = vec![0u8; len];
    stdout.read_exact(&mut payload).expect("read payload");
    decode_frame::<WorkerResponse>(&payload).expect("decode response")
}

/// Spawn the worker binary, read the spontaneous SteamConnected, send Shutdown,
/// assert clean exit.
///
/// Requires a running Steam client and app_id 105600 (Terraria) to be
/// installed. Run with:
///
/// ```text
/// cargo test --test worker_handshake -- --ignored
/// ```
#[test]
#[ignore]
fn worker_handshake_terraria() {
    let binary = env!("CARGO_BIN_EXE_steamlens-app");
    let mut child = Command::new(binary)
        .arg("--worker")
        .arg("105600")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn worker");

    let mut stdin = child.stdin.take().expect("child stdin");
    let mut stdout = child.stdout.take().expect("child stdout");

    let connected = read_one_response(&mut stdout);
    assert!(
        matches!(connected, WorkerResponse::SteamConnected { .. }),
        "first message must be SteamConnected, got {:?}",
        std::mem::discriminant(&connected)
    );

    if let WorkerResponse::SteamConnected { steam_id, app_name } = &connected {
        assert_ne!(*steam_id, 0, "steam_id must be non-zero");
        eprintln!("worker SteamConnected: steam_id={steam_id} app_name={app_name:?}");
    }

    let shutdown_bytes = encode_frame(&WorkerCommand::Shutdown).expect("encode Shutdown");
    stdin.write_all(&shutdown_bytes).expect("write Shutdown");
    stdin.flush().expect("flush");
    drop(stdin);

    let disconnected = read_one_response(&mut stdout);
    assert!(
        matches!(disconnected, WorkerResponse::Disconnected),
        "response to Shutdown must be Disconnected, got {:?}",
        std::mem::discriminant(&disconnected)
    );

    let status = child.wait().expect("wait for child");
    assert!(status.success(), "worker must exit 0, got {status}");
}

/// Spawn the worker with an invalid app_id string — the binary must exit 2.
///
/// Does not require Steam.
#[test]
fn worker_bad_app_id_exits_2() {
    let binary = env!("CARGO_BIN_EXE_steamlens-app");
    let status = Command::new(binary)
        .arg("--worker")
        .arg("not-a-number")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn");
    assert_eq!(
        status.code(),
        Some(2),
        "invalid app_id must produce exit code 2"
    );
}
