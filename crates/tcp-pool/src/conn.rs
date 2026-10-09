use std::time::{Duration, Instant};

use tokio::net::TcpStream;

/// A pooled connection: the underlying stream plus the instant it became idle.
pub(crate) struct Conn {
    pub(crate) stream: TcpStream,
    idle_since: Instant,
}

impl Conn {
    pub(crate) fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            idle_since: Instant::now(),
        }
    }

    /// How long this connection has been idle.
    pub(crate) fn idle_for(&self) -> Duration {
        self.idle_since.elapsed()
    }
}
