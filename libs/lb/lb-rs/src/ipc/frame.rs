//! Length-prefixed framing for the IPC wire.
//!
//! Each frame on the wire is `[u32 LE length][payload]`. Payloads are
//! bincode-serialized [`crate::ipc::protocol::Frame`] values.
//!
//! Read/write helpers are generic over `AsyncRead`/`AsyncWrite` so the same
//! code drives `UnixStream` halves on both ends and is easy to fuzz over an
//! in-memory `tokio::io::duplex`.
//!
//! There is no size cap on a frame — the only ceiling is the `u32` length
//! prefix (4 GiB). The peer is trusted (same-uid local process gated by db
//! folder permissions), so a malicious oversize frame isn't a threat model
//! we're defending against here.

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Write one frame: 4-byte little-endian length + `payload` bytes, then flush.
pub async fn write_frame<W>(w: &mut W, payload: &[u8]) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let len: u32 = payload.len().try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame {} bytes does not fit in a u32 length prefix", payload.len()),
        )
    })?;
    w.write_all(&len.to_le_bytes()).await?;
    w.write_all(payload).await?;
    w.flush().await
}

/// Read one frame, returning the payload (length prefix stripped).
pub async fn read_frame<R>(r: &mut R) -> io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
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
}
