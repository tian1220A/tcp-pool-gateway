//! In-memory ring-buffer logs and counters, exposed via the HTTP admin API.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// A single structured log line.
#[derive(Clone, Serialize)]
pub struct LogEntry {
    pub ts_ms: u64,
    pub level: String,
    pub message: String,
}

/// Bounded, thread-safe in-memory log buffer (oldest entries evicted first).
pub struct LogBuffer {
    buf: Mutex<VecDeque<LogEntry>>,
    cap: usize,
}

impl LogBuffer {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: Mutex::new(VecDeque::with_capacity(cap)),
            cap,
        }
    }

    pub fn push(&self, level: &str, message: impl Into<String>) {
        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let entry = LogEntry {
            ts_ms,
            level: level.to_string(),
            message: message.into(),
        };
        let mut buf = self.buf.lock().unwrap();
        buf.push_back(entry);
        while buf.len() > self.cap {
            buf.pop_front();
        }
    }

    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.buf.lock().unwrap().iter().cloned().collect()
    }
}

/// Running error/request counters for observability.
#[derive(Default)]
pub struct Counters {
    pub total_requests: AtomicU64,
    pub backend_errors: AtomicU64,
    pub acquire_errors: AtomicU64,
    pub pool_timeouts: AtomicU64,
}

#[derive(Serialize)]
pub struct CounterSnapshot {
    pub total_requests: u64,
    pub backend_errors: u64,
    pub acquire_errors: u64,
    pub pool_timeouts: u64,
}

impl Counters {
    pub fn snapshot(&self) -> CounterSnapshot {
        CounterSnapshot {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            backend_errors: self.backend_errors.load(Ordering::Relaxed),
            acquire_errors: self.acquire_errors.load(Ordering::Relaxed),
            pool_timeouts: self.pool_timeouts.load(Ordering::Relaxed),
        }
    }
}
