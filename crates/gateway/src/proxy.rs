//! TCP proxy: accepts client connections and forwards each request to the
//! backend through the connection pool.

use std::io;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};

use protocol::{read_frame, write_frame};
use tcp_pool::{PoolError, PooledConn};

use crate::state::AppState;

/// Serves the TCP gateway: for each incoming client connection, spawn a task
/// that pumps request/response frames through the pool.
pub async fn run_tcp_server(listen: SocketAddr, state: Arc<AppState>) -> io::Result<()> {
    let listener = TcpListener::bind(listen).await?;
    state.logs.push(
        "info",
        format!("TCP gateway listening on {}", listener.local_addr()?),
    );
    loop {
        let (client, _peer) = listener.accept().await?;
        client.set_nodelay(true)?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(client, state).await {
                // Error already accounted/logged inside; ignore the value here.
                let _ = e;
            }
        });
    }
}

async fn handle_client(mut client: TcpStream, state: Arc<AppState>) -> io::Result<()> {
    loop {
        // Read one binary request frame from the client.
        let Some(req) = read_frame(&mut client).await? else {
            break; // clean client disconnect
        };
        state.counters.total_requests.fetch_add(1, Ordering::Relaxed);

        // Acquire a pooled backend connection.
        let mut conn = match state.pool.get().await {
            Ok(c) => c,
            Err(PoolError::Timeout { waited }) => {
                state.counters.pool_timeouts.fetch_add(1, Ordering::Relaxed);
                state
                    .logs
                    .push("warn", format!("pool acquire timeout after {waited:?}"));
                return Err(io::Error::new(io::ErrorKind::TimedOut, "pool acquire timeout"));
            }
            Err(e) => {
                state.counters.acquire_errors.fetch_add(1, Ordering::Relaxed);
                state.logs.push("error", format!("pool acquire error: {e}"));
                return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
            }
        };

        // Forward to backend, write the response back to the client.
        match forward(&mut conn, &req).await {
            Ok(resp) => {
                if let Err(e) = write_frame(&mut client, &resp).await {
                    state.logs.push("warn", format!("write back to client failed: {e}"));
                    return Err(e);
                }
            }
            Err(e) => {
                conn.mark_broken();
                state.counters.backend_errors.fetch_add(1, Ordering::Relaxed);
                state.logs.push("error", format!("backend exchange failed: {e}"));
                return Err(e);
            }
        }
        // `conn` drops here -> returned to the pool (RAII).
    }
    Ok(())
}

/// One request/response exchange over a pooled connection.
async fn forward(conn: &mut PooledConn, req: &[u8]) -> io::Result<Vec<u8>> {
    let stream = conn.stream_mut();
    write_frame(stream, req).await?;
    read_frame(stream).await?.ok_or_else(|| {
        io::Error::new(io::ErrorKind::UnexpectedEof, "backend closed the connection")
    })
}
