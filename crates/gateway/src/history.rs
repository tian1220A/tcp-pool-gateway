//! Rolling history of pool stats, sampled by a background task for the
//! frontend trend chart.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tcp_pool::PoolStats;

/// One sampled snapshot of the pool state.
#[derive(Clone, Serialize)]
pub struct HistoryPoint {
    pub ts_ms: u64,
    pub total: usize,
    pub idle: usize,
    pub active: usize,
    pub reuse_count: u64,
}

impl HistoryPoint {
    pub fn sample(stats: &PoolStats) -> Self {
        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            ts_ms,
            total: stats.total,
            idle: stats.idle,
            active: stats.active,
            reuse_count: stats.reuse_count,
        }
    }
}

/// Bounded, thread-safe ring buffer of [`HistoryPoint`] samples.
pub struct HistoryBuffer {
    buf: Mutex<VecDeque<HistoryPoint>>,
    cap: usize,
}

impl HistoryBuffer {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: Mutex::new(VecDeque::with_capacity(cap)),
            cap,
        }
    }

    pub fn push(&self, p: HistoryPoint) {
        let mut buf = self.buf.lock().unwrap();
        buf.push_back(p);
        while buf.len() > self.cap {
            buf.pop_front();
        }
    }

    pub fn snapshot(&self) -> Vec<HistoryPoint> {
        self.buf.lock().unwrap().iter().cloned().collect()
    }
}
