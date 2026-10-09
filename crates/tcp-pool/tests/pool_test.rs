//! Integration tests for the TCP connection pool.
//!
//! Each test spins up a tiny in-process echo server and exercises the pool
//! against it, covering the six core mechanisms from the design doc.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use tcp_pool::{Pool, PoolConfig, PoolError, PooledConn};

/// Starts an uppercase-echo server. Payload `HANGUP` closes the connection
/// without replying (used to simulate a peer going away).
async fn spawn_echo_server() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                loop {
                    let mut len_buf = [0u8; 4];
                    if sock.read_exact(&mut len_buf).await.is_err() {
                        break;
                    }
                    let len = u32::from_be_bytes(len_buf) as usize;
                    let mut payload = vec![0u8; len];
                    if sock.read_exact(&mut payload).await.is_err() {
                        break;
                    }
                    if payload == b"HANGUP" {
                        break; // close without responding
                    }
                    let upper: Vec<u8> = payload.iter().map(|b| b.to_ascii_uppercase()).collect();
                    let lb = (upper.len() as u32).to_be_bytes();
                    if sock.write_all(&lb).await.is_err() || sock.write_all(&upper).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    addr
}

fn cfg(addr: std::net::SocketAddr) -> PoolConfig {
    PoolConfig {
        target_addr: addr,
        max_connections: 8,
        idle_timeout: Duration::from_secs(30),
        acquire_timeout: Duration::from_secs(2),
        min_connections: 0,
        connect_timeout: Duration::from_secs(2),
    }
}

/// Performs one length-prefixed round-trip over a pooled connection.
async fn roundtrip(conn: &mut PooledConn, payload: &[u8]) -> Vec<u8> {
    let lb = (payload.len() as u32).to_be_bytes();
    conn.write_all(&lb).await.unwrap();
    conn.write_all(payload).await.unwrap();

    let mut rl = [0u8; 4];
    conn.read_exact(&mut rl).await.unwrap();
    let n = u32::from_be_bytes(rl) as usize;
    let mut out = vec![0u8; n];
    conn.read_exact(&mut out).await.unwrap();
    out
}

#[tokio::test]
async fn echo_and_reuse() {
    let addr = spawn_echo_server().await;
    let pool = Pool::new(cfg(addr));

    {
        let mut c = pool.get().await.unwrap();
        assert_eq!(roundtrip(&mut c, b"hello").await, b"HELLO");
        assert_eq!(pool.stats().active, 1);
    } // dropped -> returned to idle

    assert_eq!(pool.stats().idle, 1);
    assert_eq!(pool.stats().total, 1);

    // Same connection should be reused (LIFO), incrementing reuse_count.
    let mut c = pool.get().await.unwrap();
    assert_eq!(roundtrip(&mut c, b"world").await, b"WORLD");
    assert_eq!(pool.stats().reuse_count, 1);
}

#[tokio::test]
async fn max_connections_then_timeout() {
    let addr = spawn_echo_server().await;
    let mut c = cfg(addr);
    c.max_connections = 1;
    c.acquire_timeout = Duration::from_millis(300);
    let pool = Pool::new(c);

    let held = pool.get().await.unwrap(); // occupies the only slot
    assert_eq!(pool.stats().total, 1);
    assert_eq!(pool.stats().idle, 0);

    let start = std::time::Instant::now();
    let err = match pool.get().await {
        Err(e) => e,
        Ok(_) => panic!("expected timeout"),
    };
    assert!(matches!(err, PoolError::Timeout { .. }));
    assert!(start.elapsed() >= Duration::from_millis(250));
    drop(held);
}

#[tokio::test]
async fn release_unblocks_waiter() {
    let addr = spawn_echo_server().await;
    let mut c = cfg(addr);
    c.max_connections = 1;
    c.acquire_timeout = Duration::from_secs(5);
    let pool = Pool::new(c);

    let held = pool.get().await.unwrap();
    let pool2 = pool.clone();
    let waiter = tokio::spawn(async move { pool2.get().await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!waiter.is_finished());

    drop(held); // returns the connection -> notifies the waiter
    let conn = waiter.await.unwrap().unwrap();
    drop(conn);
    assert_eq!(pool.stats().reuse_count, 1);
}

#[tokio::test]
async fn lazy_idle_eviction() {
    let addr = spawn_echo_server().await;
    let mut c = cfg(addr);
    c.idle_timeout = Duration::from_millis(300);
    let pool = Pool::new(c);

    {
        let _c = pool.get().await.unwrap();
    }
    assert_eq!(pool.stats().idle, 1);

    tokio::time::sleep(Duration::from_millis(400)).await;
    // Lazy eviction on next get: the stale connection is discarded, not reused.
    let c2 = pool.get().await.unwrap();
    assert_eq!(pool.stats().reuse_count, 0);
    assert_eq!(pool.stats().total, 1);
    drop(c2);
}

#[tokio::test]
async fn reaper_evicts_idle() {
    let addr = spawn_echo_server().await;
    let mut c = cfg(addr);
    c.idle_timeout = Duration::from_millis(200);
    let pool = Pool::new(c);

    {
        let _c = pool.get().await.unwrap();
    }
    assert_eq!(pool.stats().idle, 1);

    // The background reaper runs every second.
    tokio::time::sleep(Duration::from_millis(1300)).await;
    assert_eq!(pool.stats().idle, 0);
    assert_eq!(pool.stats().total, 0);
}

#[tokio::test]
async fn broken_conn_not_returned() {
    let addr = spawn_echo_server().await;
    let pool = Pool::new(cfg(addr));

    {
        let mut c = pool.get().await.unwrap();
        c.mark_broken();
    }
    assert_eq!(pool.stats().total, 0);

    let _c = pool.get().await.unwrap();
    assert_eq!(pool.stats().total, 1);
}

#[tokio::test]
async fn health_check_detects_dead_conn() {
    let addr = spawn_echo_server().await;
    let pool = Pool::new(cfg(addr));

    {
        let mut c = pool.get().await.unwrap();
        assert_eq!(roundtrip(&mut c, b"ping").await, b"PING");
        // Tell the server to close this connection.
        let lb = 6u32.to_be_bytes();
        c.write_all(&lb).await.unwrap();
        c.write_all(b"HANGUP").await.unwrap();
        // Drop it back to the pool without observing the close ourselves.
    }
    assert_eq!(pool.stats().idle, 1);

    tokio::time::sleep(Duration::from_millis(300)).await; // let the FIN arrive

    // Next acquire must detect the dead connection and create a fresh one.
    let mut c = pool.get().await.unwrap();
    assert_eq!(pool.stats().reuse_count, 0, "dead conn must not be reused");
    assert_eq!(roundtrip(&mut c, b"alive").await, b"ALIVE");
    drop(c);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_acquire() {
    let addr = spawn_echo_server().await;
    let mut c = cfg(addr);
    c.max_connections = 4;
    c.acquire_timeout = Duration::from_secs(10);
    let pool = Pool::new(c);

    let mut tasks = Vec::new();
    for i in 0..100u32 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            let mut conn = pool.get().await.unwrap();
            let payload = format!("m{i}").into_bytes();
            let out = roundtrip(&mut conn, &payload).await;
            let expected: Vec<u8> = payload.iter().map(|b| b.to_ascii_uppercase()).collect();
            assert_eq!(out, expected);
            drop(conn);
        }));
    }
    for t in tasks {
        t.await.unwrap();
    }

    let s = pool.stats();
    assert!(s.total <= 4, "total={} exceeded max=4", s.total);
    assert!(s.reuse_count > 0, "expected reuse under concurrency");
}
