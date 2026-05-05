use std::process::{Command, Stdio};

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
