//! Graceful shutdown: signal handling, connection draining, ordered teardown.
//!
//! ## Usage
//!
//! ```rust,ignore
//! let coordinator = ShutdownCoordinator::new(30);
//! let mut signal_rx = install_signal_handlers();
//!
//! tokio::select! {
//!     _ = signal_rx.recv() => {
//!         coordinator.initiate();
//!         coordinator.wait_drained().await;
//!         // flush telemetry, close storage, etc.
//!     }
//! }
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::broadcast;
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// ShutdownCoordinator
// ---------------------------------------------------------------------------

/// Coordinates graceful shutdown across the server and all connection tasks.
pub struct ShutdownCoordinator {
    /// Broadcast sender — all tasks listen to the corresponding receiver.
    tx: broadcast::Sender<()>,
    /// Number of active QUIC connections currently being served.
    active: Arc<AtomicUsize>,
    /// True once `initiate()` has been called.
    shutting_down: Arc<AtomicBool>,
    /// Maximum time to wait for connections to drain before forcing exit.
    drain_timeout: Duration,
}

impl ShutdownCoordinator {
    pub fn new(drain_timeout_secs: u64) -> Self {
        let (tx, _) = broadcast::channel(1);
        Self {
            tx,
            active: Arc::new(AtomicUsize::new(0)),
            shutting_down: Arc::new(AtomicBool::new(false)),
            drain_timeout: Duration::from_secs(drain_timeout_secs),
        }
    }

    /// Subscribe to the shutdown signal.  Each task should call this once and
    /// select on the returned receiver in its main loop.
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }

    /// Returns true once `initiate()` has been called.
    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
    }

    /// Broadcast the shutdown signal to all subscribers.
    pub fn initiate(&self) {
        self.shutting_down.store(true, Ordering::Release);
        // Ignore "no receivers" error — may happen if no connections are open.
        let _ = self.tx.send(());
        info!("Shutdown signal broadcast to all tasks");
    }

    /// Increment active connection count (call on each accepted connection).
    pub fn on_connect(&self) {
        self.active.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active connection count (call when a connection closes).
    pub fn on_disconnect(&self) {
        self.active.fetch_sub(1, Ordering::Relaxed);
    }

    /// Current active connection count.
    pub fn active_count(&self) -> usize {
        self.active.load(Ordering::Relaxed)
    }

    /// Wait until all connections have closed or the drain timeout expires.
    pub async fn wait_drained(&self) {
        let deadline = tokio::time::Instant::now() + self.drain_timeout;

        loop {
            if self.active.load(Ordering::Relaxed) == 0 {
                info!("All connections drained");
                return;
            }
            if tokio::time::Instant::now() >= deadline {
                warn!(
                    remaining = self.active.load(Ordering::Relaxed),
                    "Drain timeout expired — forcing shutdown"
                );
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

// ---------------------------------------------------------------------------
// Signal handler
// ---------------------------------------------------------------------------

/// Install OS signal handlers for SIGTERM and SIGINT.
///
/// Returns a broadcast receiver that fires once on the first signal received.
/// Subsequent signals are silently ignored so the drain window completes.
pub fn install_signal_handlers() -> broadcast::Receiver<()> {
    let (tx, rx) = broadcast::channel::<()>(1);

    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            let mut sigterm = signal(SignalKind::terminate())
                .expect("failed to register SIGTERM handler");
            let mut sigint = signal(SignalKind::interrupt())
                .expect("failed to register SIGINT handler");

            tokio::select! {
                _ = sigterm.recv() => info!("Received SIGTERM"),
                _ = sigint.recv()  => info!("Received SIGINT"),
            }
        }
        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to register Ctrl-C handler");
            info!("Received Ctrl-C");
        }

        let _ = tx.send(());
    });

    rx
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_counting() {
        let c = ShutdownCoordinator::new(5);
        assert_eq!(c.active_count(), 0);

        c.on_connect();
        c.on_connect();
        assert_eq!(c.active_count(), 2);

        c.on_disconnect();
        assert_eq!(c.active_count(), 1);

        c.on_disconnect();
        assert_eq!(c.active_count(), 0);
    }

    #[tokio::test]
    async fn test_initiate_sets_flag() {
        let c = ShutdownCoordinator::new(5);
        assert!(!c.is_shutting_down());
        c.initiate();
        assert!(c.is_shutting_down());
    }

    #[tokio::test]
    async fn test_wait_drained_immediate_when_zero() {
        let c = ShutdownCoordinator::new(5);
        // No connections — should return instantly.
        tokio::time::timeout(Duration::from_millis(200), c.wait_drained())
            .await
            .expect("wait_drained should complete instantly when no connections");
    }

    #[tokio::test]
    async fn test_wait_drained_timeout() {
        let c = ShutdownCoordinator::new(0); // 0s timeout
        c.on_connect(); // one connection, never released
        // Should return after timeout, not hang.
        tokio::time::timeout(Duration::from_millis(500), c.wait_drained())
            .await
            .expect("wait_drained should respect timeout");
    }

    #[tokio::test]
    async fn test_subscriber_receives_signal() {
        let c = ShutdownCoordinator::new(5);
        let mut rx = c.subscribe();
        c.initiate();
        tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("receiver should get signal")
            .expect("channel should not be closed");
    }
}
