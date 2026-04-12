//! Kubernetes health probe endpoints (ntex HTTP server).
//!
//! Two routes are served on a dedicated internal port (default 9090):
//!
//! - `GET /health/live`  — liveness:  always 200 while the process is running
//! - `GET /health/ready` — readiness: actively pings NATS and Redis;
//!                          returns 200 only when both respond within 2 s
//!
//! The server is intentionally **not** exposed via a LoadBalancer Service —
//! only the ClusterIP service is wired up so k8s probes can reach it while
//! external traffic cannot.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ntex::web::{self, App, HttpResponse, HttpServer};
use ntex::web::types::State;
use tracing::{error, info};

use crate::storage::nats::NatsClient;
use crate::storage::redis::RedisClient;

// ---------------------------------------------------------------------------
// HealthDeps — holds optional handles to the storage clients for probing
// ---------------------------------------------------------------------------

/// Shared dependencies injected into every health handler.
pub struct HealthDeps {
    pub nats: Option<Arc<NatsClient>>,
    pub redis: Option<Arc<RedisClient>>,
    /// Set to false when a graceful shutdown begins (makes /ready return 503).
    pub accepting: AtomicBool,
}

impl HealthDeps {
    pub fn new(
        nats: Option<Arc<NatsClient>>,
        redis: Option<Arc<RedisClient>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            nats,
            redis,
            accepting: AtomicBool::new(true),
        })
    }

    pub fn set_accepting(&self, v: bool) {
        self.accepting.store(v, Ordering::Relaxed);
    }

    /// Actively probe every configured dependency.
    ///
    /// - If NATS is configured it must respond to a flush within 2 s.
    /// - If Redis is configured it must respond to PING within 2 s.
    /// - Unconfigured deps are ignored (treated as healthy).
    /// - Returns false if the server is draining (shutdown in progress).
    pub async fn is_ready(&self) -> bool {
        if !self.accepting.load(Ordering::Relaxed) {
            return false;
        }

        if let Some(ref nats) = self.nats {
            if !nats.ping().await {
                return false;
            }
        }

        if let Some(ref redis) = self.redis {
            if !redis.ping().await {
                return false;
            }
        }

        true
    }
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// Liveness — always 200 while the process is alive.
async fn live() -> HttpResponse {
    HttpResponse::Ok().finish()
}

/// Readiness — actively probes NATS and Redis; 503 on any failure.
async fn ready(deps: State<Arc<HealthDeps>>) -> HttpResponse {
    if deps.is_ready().await {
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::ServiceUnavailable().finish()
    }
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Start the health HTTP server as a background Tokio task.
///
/// Spawns the ntex `HttpServer` inside the current tokio runtime.
/// Returns an error only if the bind address is already in use.
pub fn spawn_health_server(
    addr: SocketAddr,
    deps: Arc<HealthDeps>,
) -> std::io::Result<()> {
    // ntex 3.x: factory closure must return an async block that resolves to App.
    let server = HttpServer::new(move || {
        let deps = Arc::clone(&deps);
        async move {
            App::new()
                .state(deps)
                .route("/health/live", web::get().to(live))
                .route("/health/ready", web::get().to(ready))
        }
    })
    .bind(addr)?
    .run();

    info!("Health server listening on http://{addr}");

    tokio::spawn(async move {
        if let Err(e) = server.await {
            error!("Health server error: {e}");
        }
    });

    Ok(())
}
