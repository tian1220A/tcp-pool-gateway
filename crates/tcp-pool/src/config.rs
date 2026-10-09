use std::net::SocketAddr;
use std::time::Duration;

/// Global configuration for a [`crate::Pool`].
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of live connections (idle + active) the pool may hold.
    pub max_connections: usize,
    /// A connection idle longer than this is evicted (lazily + by the reaper).
    pub idle_timeout: Duration,
    /// How long [`crate::Pool::get`] waits before returning
    /// [`crate::PoolError::Timeout`] when the pool is saturated.
    pub acquire_timeout: Duration,
    /// Backend address new connections dial.
    pub target_addr: SocketAddr,
    /// Number of connections to pre-establish eagerly ("预创建" warm-up).
    pub min_connections: usize,
    /// Per-connection TCP connect timeout.
    pub connect_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 32,
            idle_timeout: Duration::from_secs(30),
            acquire_timeout: Duration::from_secs(5),
            target_addr: "127.0.0.1:9000".parse().unwrap(),
            min_connections: 0,
            connect_timeout: Duration::from_secs(3),
        }
    }
}
