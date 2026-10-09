//! Minimal length-prefixed binary frame protocol shared by all services.
//!
//! Frame layout:
//! ```text
//! +----------------+-----------------------+
//! | u32 BE length  |        payload        |
//! +----------------+-----------------------+
//! ```

use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Maximum frame payload size (1 MiB).
pub const MAX_FRAME_LEN: usize = 1024 * 1024;

/// Reads one frame. Returns `Ok(None)` on a clean EOF between frames.
pub async fn read_frame<S: AsyncReadExt + Unpin>(s: &mut S) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    // Read the first byte separately so a clean EOF is distinguishable from a
    // truncated header.
    let n = s.read(&mut len_buf[..1]).await?;
    if n == 0 {
        return Ok(None);
    }
    s.read_exact(&mut len_buf[1..]).await?;

    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_LEN {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }

    let mut payload = vec![0u8; len];
    s.read_exact(&mut payload).await?;
    Ok(Some(payload))
}

/// Writes one frame: a 4-byte big-endian length prefix followed by the payload.
pub async fn write_frame<S: AsyncWriteExt + Unpin>(s: &mut S, payload: &[u8]) -> io::Result<()> {
    let len = payload.len() as u32;
    s.write_all(&len.to_be_bytes()).await?;
    s.write_all(payload).await?;
    Ok(())
}
