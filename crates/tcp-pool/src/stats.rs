use serde::Serialize;

/// Snapshot of pool state for monitoring and visualization.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PoolStats {
    /// Total live connections (idle + active).
    pub total: usize,
    /// Connections currently idle in the pool.
    pub idle: usize,
    /// Connections currently checked out.
    pub active: usize,
    /// Total number of times an idle connection was reused.
    pub reuse_count: u64,
    /// Current max-connections limit.
    pub max_connections: usize,
    /// Current idle timeout in milliseconds.
    pub idle_timeout_ms: u64,
    /// Current acquire timeout in milliseconds.
    pub acquire_timeout_ms: u64,
}
