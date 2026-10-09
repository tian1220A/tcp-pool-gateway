use std::sync::Arc;

use tcp_pool::Pool;

use crate::history::HistoryBuffer;
use crate::logging::{Counters, LogBuffer};

/// Shared application state handed to both the TCP proxy and the HTTP API.
pub struct AppState {
    pub pool: Arc<Pool>,
    pub logs: LogBuffer,
    pub counters: Counters,
    pub history: HistoryBuffer,
}

impl AppState {
    pub fn new(pool: Arc<Pool>) -> Self {
        Self {
            pool,
            logs: LogBuffer::new(512),
            counters: Counters::default(),
            history: HistoryBuffer::new(300),
        }
    }
}
