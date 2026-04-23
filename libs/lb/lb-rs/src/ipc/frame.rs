//! Length-prefixed framing for the IPC wire.
//!
//! Each frame on the wire is `[u32 LE length][payload]`. Payloads are
//! bincode-serialized [`crate::ipc::protocol::Frame`] values.
//!
//! Read/write helpers are generic over `AsyncRead`/`AsyncWrite` so the same
//! code drives `UnixStream` halves on both ends and is easy to fuzz over an
//! in-memory `tokio::io::duplex`.

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Hard cap on the size of a single frame.
///
/// 16 MiB is comfortably above any realistic encrypted-document chunk we
/// pass through the API today and small enough that a malicious or
/// corrupted length prefix can't OOM us. If we ever stream documents larger
/// than this (e.g., big SVGs) the API needs chunking, not a bigger cap.
pub const MAX_FRAME_LEN: usize = 16 * 1024 * 1024;

/// Write one frame: 4-byte little-endian length + `payload` bytes, then flush.
pub async fn write_frame<W>(w: &mut W, payload: &[u8]) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    if payload.len() > MAX_FRAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame {} bytes exceeds cap of {} bytes", payload.len(), MAX_FRAME_LEN),
        ));
    }
    let len = payload.len() as u32;
    w.write_all(&len.to_le_bytes()).await?;
    w.write_all(payload).await?;
    w.flush().await
}

/// Read one frame, returning the payload (length prefix stripped).
///
/// Returns an error if the length prefix exceeds [`MAX_FRAME_LEN`] before
/// allocating the buffer.
pub async fn read_frame<R>(r: &mut R) -> io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame length {len} bytes exceeds cap of {MAX_FRAME_LEN} bytes"),
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn round_trip_small() {
        let (mut a, mut b) = duplex(64 * 1024);
        write_frame(&mut a, b"hello").await.unwrap();
        let got = read_frame(&mut b).await.unwrap();
        assert_eq!(got, b"hello");
    }

    #[tokio::test]
    async fn round_trip_empty() {
        let (mut a, mut b) = duplex(1024);
        write_frame(&mut a, b"").await.unwrap();
        let got = read_frame(&mut b).await.unwrap();
        assert!(got.is_empty());
    }

    #[tokio::test]
    async fn rejects_oversize_write() {
        let (mut a, _b) = duplex(1024);
        let payload = vec![0u8; MAX_FRAME_LEN + 1];
        let err = write_frame(&mut a, &payload).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn rejects_oversize_read() {
        let (mut a, mut b) = duplex(1024);
        let bad_len = (MAX_FRAME_LEN as u32 + 1).to_le_bytes();
        tokio::io::AsyncWriteExt::write_all(&mut a, &bad_len)
            .await
            .unwrap();
        let err = read_frame(&mut b).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
