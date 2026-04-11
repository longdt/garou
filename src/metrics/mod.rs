//! Prometheus metrics for Garou chat server.
//!
//! All metrics are registered once at startup via `init_metrics()` and updated
//! through the helper functions exported below.

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

/// Metric names (used as constants to avoid typos).
pub const CONNECTIONS_TOTAL: &str = "garou_connections_total";
pub const CONNECTIONS_ACTIVE: &str = "garou_connections_active";
pub const AUTHENTICATED_CONNECTIONS: &str = "garou_authenticated_connections_total";
pub const MESSAGES_TOTAL: &str = "garou_messages_total";
pub const MESSAGE_LATENCY_MS: &str = "garou_message_latency_ms";
pub const ROOMS_ACTIVE: &str = "garou_rooms_active";
pub const ERRORS_TOTAL: &str = "garou_errors_total";

/// Install the Prometheus recorder and return a handle for scraping.
///
/// Must be called once before any metric helpers are used.
pub fn init_metrics() -> PrometheusHandle {
    let builder = PrometheusBuilder::new();
    let handle = builder
        .install_recorder()
        .expect("failed to install Prometheus recorder");

    describe_counter!(
        CONNECTIONS_TOTAL,
        "Total number of QUIC connections accepted since startup"
    );
    describe_gauge!(
        CONNECTIONS_ACTIVE,
        "Number of currently active QUIC connections"
    );
    describe_counter!(
        AUTHENTICATED_CONNECTIONS,
        "Total number of connections that completed JWT authentication"
    );
    describe_counter!(MESSAGES_TOTAL, "Total number of chat messages processed");
    describe_histogram!(
        MESSAGE_LATENCY_MS,
        "End-to-end message latency from frame received to ACK sent, in milliseconds"
    );
    describe_gauge!(ROOMS_ACTIVE, "Number of currently active chat rooms");
    describe_counter!(
        ERRORS_TOTAL,
        "Total number of internal errors by kind"
    );

    handle
}

// ---------------------------------------------------------------------------
// Metric helper functions
// ---------------------------------------------------------------------------

/// Increment the total accepted connections counter and active gauge.
pub fn increment_connections_total() {
    counter!(CONNECTIONS_TOTAL).increment(1);
    gauge!(CONNECTIONS_ACTIVE).increment(1.0);
}

/// Decrement the active connections gauge (called on disconnect).
pub fn decrement_connections_active() {
    gauge!(CONNECTIONS_ACTIVE).decrement(1.0);
}

/// Increment the authenticated connections counter.
pub fn increment_authenticated_connections() {
    counter!(AUTHENTICATED_CONNECTIONS).increment(1);
}

/// Increment the messages processed counter.
pub fn increment_messages_total() {
    counter!(MESSAGES_TOTAL).increment(1);
}

/// Record a message latency observation in milliseconds.
pub fn record_message_latency_ms(ms: f64) {
    histogram!(MESSAGE_LATENCY_MS).record(ms);
}

/// Set the active rooms gauge.
pub fn set_rooms_active(count: f64) {
    gauge!(ROOMS_ACTIVE).set(count);
}

/// Increment the errors counter with an error kind label.
pub fn increment_errors_total(kind: &'static str) {
    counter!(ERRORS_TOTAL, "kind" => kind).increment(1);
}
