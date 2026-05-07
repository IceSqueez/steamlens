use steamlens_core::ipc::{WorkerResponse, decode_frame, parse_header};
use tokio::io::{AsyncRead, AsyncReadExt};

pub async fn read_response<R: AsyncRead + Unpin>(reader: &mut R) -> Option<WorkerResponse> {
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
    use steamlens_core::ipc::{WorkerErrorKind, encode_frame};
    use tokio::io::AsyncWriteExt;

    async fn write_frame(writer: &mut (impl tokio::io::AsyncWrite + Unpin), resp: &WorkerResponse) {
        let bytes = encode_frame(resp).expect("encode");
        writer.write_all(&bytes).await.expect("write");
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
