use std::fmt;
use std::time::Duration;

/// Errors that can occur while acquiring a connection from the pool.
#[derive(Debug)]
pub enum PoolError {
    /// Timed out waiting for a connection to become available.
    Timeout { waited: Duration },
    /// The pool has been closed.
    PoolClosed,
    /// An I/O error occurred while establishing a new connection.
    Io(std::io::Error),
}

impl fmt::Display for PoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PoolError::Timeout { waited } => write!(f, "acquire timed out after {:?}", waited),
            PoolError::PoolClosed => write!(f, "connection pool is closed"),
            PoolError::Io(e) => write!(f, "io error: {}", e),
        }
    }
}

impl std::error::Error for PoolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PoolError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PoolError {
    fn from(e: std::io::Error) -> Self {
        PoolError::Io(e)
    }
}
