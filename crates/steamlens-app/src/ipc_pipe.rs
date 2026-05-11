use steamlens_core::ipc::{
    WorkerCommand, WorkerResponse, decode_frame, encode_frame, parse_header,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub(crate) async fn write_command<W: AsyncWrite + Unpin>(
    writer: &mut W,
    cmd: &WorkerCommand,
) -> std::io::Result<()> {
    let framed = encode_frame(cmd)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    writer.write_all(&framed).await?;
    writer.flush().await
}

pub(crate) async fn read_response<R: AsyncRead + Unpin>(reader: &mut R) -> Option<WorkerResponse> {
    let mut header = [0u8; 4];
    reader.read_exact(&mut header).await.ok()?;
    let len = parse_header(header).ok()?;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await.ok()?;
    decode_frame::<WorkerResponse>(&buf).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use steamlens_core::ipc::{WorkerCommand, WorkerErrorKind, encode_frame};
    use tokio::io::AsyncWriteExt;

    async fn write_frame(writer: &mut (impl tokio::io::AsyncWrite + Unpin), resp: &WorkerResponse) {
        let bytes = encode_frame(resp).expect("encode");
        writer.write_all(&bytes).await.expect("write");
    }

    #[tokio::test]
    async fn write_then_read_command_round_trip() {
        let (mut tx, mut rx) = tokio::io::duplex(4096);
        write_command(&mut tx, &WorkerCommand::Shutdown)
            .await
            .expect("write");
        drop(tx);

        let mut header = [0u8; 4];
        rx.read_exact(&mut header).await.expect("read header");
        let len = parse_header(header).expect("parse header");
        let mut buf = vec![0u8; len];
        rx.read_exact(&mut buf).await.expect("read payload");
        let decoded = decode_frame::<WorkerCommand>(&buf).expect("decode");
        assert!(matches!(decoded, WorkerCommand::Shutdown));
    }

    #[tokio::test]
    async fn round_trip_steam_connected() {
        let (mut tx, mut rx) = tokio::io::duplex(4096);
        write_frame(
            &mut tx,
            &WorkerResponse::SteamConnected {
                steam_id: 76561198000000042,
                app_name: Some("Terraria".to_owned()),
            },
        )
        .await;
        drop(tx);

        let resp = read_response(&mut rx).await.expect("decoded");
        match resp {
            WorkerResponse::SteamConnected { steam_id, app_name } => {
                assert_eq!(steam_id, 76561198000000042);
                assert_eq!(app_name.as_deref(), Some("Terraria"));
            }
            other => panic!("expected SteamConnected, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn round_trip_error_frame() {
        let (mut tx, mut rx) = tokio::io::duplex(4096);
        write_frame(
            &mut tx,
            &WorkerResponse::Error {
                kind: WorkerErrorKind::Connect,
                message: "no pipe".to_owned(),
            },
        )
        .await;
        drop(tx);

        let resp = read_response(&mut rx).await.expect("decoded");
        match resp {
            WorkerResponse::Error { kind, message } => {
                assert_eq!(kind, WorkerErrorKind::Connect);
                assert_eq!(message, "no pipe");
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn round_trip_two_frames_sequential() {
        let (mut tx, mut rx) = tokio::io::duplex(4096);
        write_frame(&mut tx, &WorkerResponse::Ack).await;
        write_frame(&mut tx, &WorkerResponse::Stored).await;
        drop(tx);

        let first = read_response(&mut rx).await.expect("first decoded");
        assert!(matches!(first, WorkerResponse::Ack));
        let second = read_response(&mut rx).await.expect("second decoded");
        assert!(matches!(second, WorkerResponse::Stored));
    }

    #[tokio::test]
    async fn returns_none_on_eof_before_header() {
        let (tx, mut rx) = tokio::io::duplex(4096);
        drop(tx);
        assert!(read_response(&mut rx).await.is_none());
    }

    #[tokio::test]
    async fn returns_none_on_truncated_payload() {
        let (mut tx, mut rx) = tokio::io::duplex(4096);
        let bytes = encode_frame(&WorkerResponse::Ack).expect("encode");
        tx.write_all(&bytes[..bytes.len() - 1])
            .await
            .expect("write truncated");
        drop(tx);
        assert!(read_response(&mut rx).await.is_none());
    }
}
