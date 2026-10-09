//! Core asynchronous TCP connection pool implementation.

use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio::time;

use crate::config::PoolConfig;
use crate::conn::Conn;
use crate::error::PoolError;
use crate::stats::PoolStats;

/// Internal, mutex-protected pool state.
struct Inner {
    /// Idle connections, kept as a LIFO stack (newest reused first).
    idle: Vec<Conn>,
    /// Total connections tracked by the pool (idle + active).
    total: usize,
    /// Number of times an idle connection was successfully reused.
    reuse_count: u64,
    /// Whether the pool has been closed.
    closed: bool,
}

/// An asynchronous, self-managing TCP connection pool.
///
/// # Design
/// - **LIFO reuse**: idle connections are popped from the top of the stack,
///   keeping hot connections alive and letting cold ones expire.
/// - **Health check**: every checkout probes the socket non-blockingly; a
///   closed (`0` bytes) or errored connection is discarded.
/// - **Idle eviction**: a background reaper reclaims connections idle longer
///   than `idle_timeout`, returning capacity to the pool.
/// - **Bounded size**: `max_connections` caps the number of live connections.
/// - **Timed acquire**: `get` waits (with notification wakeups) up to
///   `acquire_timeout` before returning [`PoolError::Timeout`].
/// - **RAII return**: [`PooledConn`] returns itself on drop; a connection
///   marked broken is closed instead of reused.
///
/// The mutex is held only while touching the idle stack or counters; all I/O
/// and connection establishment happen outside the lock.
pub struct Pool {
    inner: Mutex<Inner>,
    target: SocketAddr,
    connect_timeout: Duration,
    max_connections: AtomicUsize,
    idle_timeout_ms: AtomicU64,
    acquire_timeout_ms: AtomicU64,
    closed: AtomicBool,
    notify: Notify,
}

/// Result of an attempt to obtain a connection without waiting.
enum TryGet {
    Got(PooledConn),
    Full,
    Closed,
}

impl Pool {
    /// Creates the pool, starts its background reaper, and optionally
    /// pre-establishes `min_connections` connections ("预创建").
    ///
    /// Must be called from within a Tokio runtime.
    pub fn new(config: PoolConfig) -> Arc<Self> {
        let pool = Arc::new(Self {
            inner: Mutex::new(Inner {
                idle: Vec::new(),
                total: 0,
                reuse_count: 0,
                closed: false,
            }),
            target: config.target_addr,
            connect_timeout: config.connect_timeout,
            max_connections: AtomicUsize::new(config.max_connections),
            idle_timeout_ms: AtomicU64::new(config.idle_timeout.as_millis() as u64),
            acquire_timeout_ms: AtomicU64::new(config.acquire_timeout.as_millis() as u64),
            closed: AtomicBool::new(false),
            notify: Notify::new(),
        });

        let reaper = pool.clone();
        tokio::spawn(async move { reaper.reaper_loop().await });

        if config.min_connections > 0 {
            let warmer = pool.clone();
            tokio::spawn(async move { warmer.prewarm(config.min_connections).await });
        }

        pool
    }

    /// Acquires a connection, waiting up to `acquire_timeout` if the pool is
    /// saturated. Returns a [`PooledConn`] that is returned automatically on drop.
    pub async fn get(self: &Arc<Self>) -> Result<PooledConn, PoolError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(PoolError::PoolClosed);
        }
        let acquire_timeout =
            Duration::from_millis(self.acquire_timeout_ms.load(Ordering::Acquire));
        let start = Instant::now();
        let deadline = start + acquire_timeout;

        loop {
            // Arm the notification *before* the check to avoid lost wakeups.
            let notified = self.notify.notified();

            match self.try_get().await? {
                TryGet::Got(conn) => return Ok(conn),
                TryGet::Closed => return Err(PoolError::PoolClosed),
                TryGet::Full => {}
            }

            let now = Instant::now();
            if now >= deadline {
                return Err(PoolError::Timeout { waited: start.elapsed() });
            }
            tokio::select! {
                _ = notified => {}
                _ = time::sleep(deadline - now) => {
                    return Err(PoolError::Timeout { waited: start.elapsed() });
                }
            }
        }
    }

    /// Non-blocking attempt to obtain a connection. Returns `Err` only for a
    /// genuine I/O failure while establishing a fresh connection.
    async fn try_get(self: &Arc<Self>) -> Result<TryGet, PoolError> {
        {
            let mut inner = self.inner.lock().unwrap();
            if inner.closed {
                return Ok(TryGet::Closed);
            }
            loop {
                if let Some(mut conn) = inner.idle.pop() {
                    let idle_timeout =
                        Duration::from_millis(self.idle_timeout_ms.load(Ordering::Acquire));
                    // Lazy idle-eviction + health check (both non-blocking).
                    if conn.idle_for() > idle_timeout || !Self::is_alive(&mut conn.stream) {
                        inner.total -= 1;
                        drop(conn);
                        self.notify.notify_one(); // freed a slot
                        continue;
                    }
                    inner.reuse_count += 1;
                    return Ok(TryGet::Got(PooledConn::new(conn.stream, self.clone())));
                }

                let max = self.max_connections.load(Ordering::Acquire);
                if inner.total >= max {
                    return Ok(TryGet::Full);
                }
                inner.total += 1; // reserve a slot for a new connection
                break;
            }
        } // lock released before any await

        match self.connect().await {
            Ok(stream) => Ok(TryGet::Got(PooledConn::new(stream, self.clone()))),
            Err(e) => {
                let mut inner = self.inner.lock().unwrap();
                inner.total -= 1; // release the reserved slot
                drop(inner);
                self.notify.notify_one();
                Err(PoolError::Io(e))
            }
        }
    }

    /// Returns a connection to the idle set (or discards it if broken/closed).
    fn return_conn(&self, stream: TcpStream, broken: bool) {
        let mut inner = self.inner.lock().unwrap();
        if broken || self.closed.load(Ordering::Acquire) {
            inner.total -= 1;
            drop(inner);
            drop(stream); // close the socket
            self.notify.notify_one();
            return;
        }
        inner.idle.push(Conn::new(stream)); // LIFO: newest on top
        drop(inner);
        self.notify.notify_one();
    }

    async fn connect(&self) -> io::Result<TcpStream> {
        let stream = match time::timeout(self.connect_timeout, TcpStream::connect(self.target)).await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(io::Error::new(io::ErrorKind::TimedOut, "connect timed out")),
        };
        // Disable Nagle: pooled connections carry request/response traffic,
        // where a small write would otherwise stall up to ~40ms on delayed ACK.
        stream.set_nodelay(true)?;
        Ok(stream)
    }

    /// Non-blocking liveness probe used on checkout.
    ///
    /// - `Ok(0)`        — peer closed → dead
    /// - `WouldBlock`   — no data pending → healthy
    /// - other `Err`    — I/O error → dead
    fn is_alive(stream: &mut TcpStream) -> bool {
        let mut buf = [0u8; 1];
        match stream.try_read(&mut buf) {
            Ok(0) => false,
            Ok(_) => true,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => true,
            Err(_) => false,
        }
    }

    async fn prewarm(&self, n: usize) {
        for _ in 0..n {
            if self.closed.load(Ordering::Acquire) {
                break;
            }
            {
                let mut inner = self.inner.lock().unwrap();
                if inner.total >= self.max_connections.load(Ordering::Acquire) {
                    break;
                }
                inner.total += 1;
            }
            match self.connect().await {
                Ok(stream) => {
                    let mut inner = self.inner.lock().unwrap();
                    inner.idle.push(Conn::new(stream));
                }
                Err(_) => {
                    let mut inner = self.inner.lock().unwrap();
                    inner.total -= 1;
                    break;
                }
            }
        }
    }

    async fn reaper_loop(self: Arc<Self>) {
        let mut ticker = time::interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            if self.closed.load(Ordering::Acquire) {
                break;
            }
            self.evict_expired();
        }
    }

    fn evict_expired(&self) {
        let idle_timeout = Duration::from_millis(self.idle_timeout_ms.load(Ordering::Acquire));
        let mut inner = self.inner.lock().unwrap();
        let mut removed = 0usize;
        inner.idle.retain(|c| {
            if c.idle_for() > idle_timeout {
                removed += 1;
                false
            } else {
                true
            }
        });
        if removed > 0 {
            inner.total -= removed;
            for _ in 0..removed {
                self.notify.notify_one();
            }
        }
    }

    /// Closes the pool: rejects new acquisitions and closes idle connections.
    /// Outstanding [`PooledConn`]s are destroyed when they are returned.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        let mut inner = self.inner.lock().unwrap();
        inner.closed = true;
        let idle = std::mem::take(&mut inner.idle);
        inner.total -= idle.len();
        drop(inner);
        drop(idle); // close all idle sockets
        self.notify.notify_waiters();
    }

    /// Snapshots the pool state for monitoring.
    pub fn stats(&self) -> PoolStats {
        let inner = self.inner.lock().unwrap();
        PoolStats {
            total: inner.total,
            idle: inner.idle.len(),
            active: inner.total - inner.idle.len(),
            reuse_count: inner.reuse_count,
            max_connections: self.max_connections.load(Ordering::Acquire),
            idle_timeout_ms: self.idle_timeout_ms.load(Ordering::Acquire),
            acquire_timeout_ms: self.acquire_timeout_ms.load(Ordering::Acquire),
        }
    }

    // -- Dynamic configuration (atomic; safe to call at runtime) --

    /// Updates the max-connections limit. Raising it may unblock waiters;
    /// lowering it takes effect on future acquisitions.
    pub fn set_max_connections(&self, n: usize) {
        self.max_connections.store(n, Ordering::Release);
        self.notify.notify_waiters();
    }

    pub fn set_idle_timeout(&self, d: Duration) {
        self.idle_timeout_ms.store(d.as_millis() as u64, Ordering::Release);
    }

    pub fn set_acquire_timeout(&self, d: Duration) {
        self.acquire_timeout_ms.store(d.as_millis() as u64, Ordering::Release);
    }
}

/// A checked-out connection. Returns itself to the pool on [`Drop`]; a
/// connection marked broken is closed instead of reused (RAII, zero-leak).
pub struct PooledConn {
    pool: Arc<Pool>,
    stream: Option<TcpStream>,
    broken: bool,
}

impl PooledConn {
    fn new(stream: TcpStream, pool: Arc<Pool>) -> Self {
        Self {
            pool,
            stream: Some(stream),
            broken: false,
        }
    }

    /// Marks the connection broken so it is closed (not reused) on drop.
    pub fn mark_broken(&mut self) {
        self.broken = true;
    }

    pub fn is_broken(&self) -> bool {
        self.broken
    }

    /// Borrows the underlying TCP stream for direct use.
    pub fn stream(&self) -> &TcpStream {
        self.stream.as_ref().expect("connection present")
    }

    pub fn stream_mut(&mut self) -> &mut TcpStream {
        self.stream.as_mut().expect("connection present")
    }
}

impl std::ops::Deref for PooledConn {
    type Target = TcpStream;
    fn deref(&self) -> &Self::Target {
        self.stream()
    }
}

impl std::ops::DerefMut for PooledConn {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.stream_mut()
    }
}

impl std::fmt::Debug for PooledConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledConn")
            .field("broken", &self.broken)
            .finish()
    }
}

impl Drop for PooledConn {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            self.pool.return_conn(stream, self.broken);
        }
    }
}
