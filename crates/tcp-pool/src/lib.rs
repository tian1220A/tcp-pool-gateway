//! Self-built asynchronous TCP connection pool on top of Tokio.
//!
//! Core building blocks:
//! - [`Pool`] — the pool: lazily creates, reuses (LIFO), health-checks and
//!   evicts connections.
//! - [`PooledConn`] — an RAII-guarded checked-out connection that returns
//!   itself on drop (a broken one is closed instead of reused).
//! - [`PoolConfig`] / [`PoolStats`] / [`PoolError`].

mod config;
mod conn;
mod error;
mod pool;
mod stats;

pub use config::PoolConfig;
pub use error::PoolError;
pub use pool::{Pool, PooledConn};
pub use stats::PoolStats;
