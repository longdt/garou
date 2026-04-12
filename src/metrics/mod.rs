//! OpenTelemetry telemetry initialisation and metric instruments for Garou.
//!
//! Call [`init_telemetry`] once at startup.  It returns a [`TelemetryGuard`]
//! that flushes all providers when dropped.
//!
//! Metric instruments are exposed through [`Metrics`], which is created from
//! the global [`opentelemetry::global`] meter after `init_telemetry` has run.

use opentelemetry::metrics::{Counter, Histogram, Meter, UpDownCounter};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{KeyValue, global};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

/// Holds all OTel providers so they are shut down gracefully on drop.
pub struct TelemetryGuard {
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
    logger_provider: SdkLoggerProvider,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Err(e) = self.tracer_provider.shutdown() {
            eprintln!("tracer provider shutdown error: {e}");
        }
        if let Err(e) = self.meter_provider.shutdown() {
            eprintln!("meter provider shutdown error: {e}");
        }
        if let Err(e) = self.logger_provider.shutdown() {
            eprintln!("logger provider shutdown error: {e}");
        }
    }
}

/// Initialise OpenTelemetry (traces, metrics, logs) with OTLP/gRPC export.
///
/// `otlp_endpoint` overrides `OTEL_EXPORTER_OTLP_ENDPOINT` when non-empty.
/// Pass an empty string to rely solely on the environment variable.
pub fn init_telemetry(service_name: &'static str, otlp_endpoint: &str) -> TelemetryGuard {
    let resource = Resource::builder()
        .with_attribute(KeyValue::new(SERVICE_NAME, service_name))
        .build();

    // --- Tracer provider ---
    let mut span_builder = SpanExporter::builder().with_tonic();
    if !otlp_endpoint.is_empty() {
        span_builder = span_builder.with_endpoint(otlp_endpoint);
    }
    let span_exporter = span_builder
        .build()
        .expect("failed to build OTLP span exporter");

    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(resource.clone())
        .with_batch_exporter(span_exporter)
        .build();

    global::set_tracer_provider(tracer_provider.clone());

    // --- Meter provider ---
    let mut metric_builder = MetricExporter::builder().with_tonic();
    if !otlp_endpoint.is_empty() {
        metric_builder = metric_builder.with_endpoint(otlp_endpoint);
    }
    let metric_exporter = metric_builder
        .build()
        .expect("failed to build OTLP metric exporter");

    let meter_provider = SdkMeterProvider::builder()
        .with_resource(resource.clone())
        .with_periodic_exporter(metric_exporter)
        .build();

    global::set_meter_provider(meter_provider.clone());

    // --- Logger provider ---
    let mut log_builder = LogExporter::builder().with_tonic();
    if !otlp_endpoint.is_empty() {
        log_builder = log_builder.with_endpoint(otlp_endpoint);
    }
    let log_exporter = log_builder
        .build()
        .expect("failed to build OTLP log exporter");

    let logger_provider = SdkLoggerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(log_exporter)
        .build();

    // --- tracing subscriber ---
    // Bridges `tracing` spans → OTel traces and `tracing` events → OTel logs.
    let tracer = tracer_provider.tracer(service_name);
    let log_bridge = OpenTelemetryTracingBridge::new(&logger_provider);

    Registry::default()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .with(OpenTelemetryLayer::new(tracer))
        .with(log_bridge)
        .init();

    TelemetryGuard {
        tracer_provider,
        meter_provider,
        logger_provider,
    }
}

// ---------------------------------------------------------------------------
// Metric instruments
// ---------------------------------------------------------------------------

/// All metric instruments for the Garou server.
///
/// Create once with [`Metrics::new`] after [`init_telemetry`] has been called.
pub struct Metrics {
    pub connections_total: Counter<u64>,
    pub connections_active: UpDownCounter<i64>,
    pub authenticated_connections: Counter<u64>,
    pub messages_total: Counter<u64>,
    pub message_latency_ms: Histogram<f64>,
    pub rooms_active: UpDownCounter<i64>,
    pub errors_total: Counter<u64>,
}

impl Metrics {
    pub fn new(meter: &Meter) -> Self {
        Self {
            connections_total: meter
                .u64_counter("garou_connections_total")
                .with_description("Total QUIC connections accepted since startup")
                .build(),
            connections_active: meter
                .i64_up_down_counter("garou_connections_active")
                .with_description("Number of currently active QUIC connections")
                .build(),
            authenticated_connections: meter
                .u64_counter("garou_authenticated_connections_total")
                .with_description("Total connections that completed JWT authentication")
                .build(),
            messages_total: meter
                .u64_counter("garou_messages_total")
                .with_description("Total chat messages processed")
                .build(),
            message_latency_ms: meter
                .f64_histogram("garou_message_latency_ms")
                .with_description(
                    "End-to-end message latency from frame received to ACK sent, in milliseconds",
                )
                .with_unit("ms")
                .build(),
            rooms_active: meter
                .i64_up_down_counter("garou_rooms_active")
                .with_description("Number of currently active chat rooms")
                .build(),
            errors_total: meter
                .u64_counter("garou_errors_total")
                .with_description("Total internal errors by kind")
                .build(),
        }
    }
}

impl Metrics {
    pub fn increment_connections_total(&self) {
        self.connections_total.add(1, &[]);
        self.connections_active.add(1, &[]);
    }

    pub fn decrement_connections_active(&self) {
        self.connections_active.add(-1, &[]);
    }

    pub fn increment_authenticated_connections(&self) {
        self.authenticated_connections.add(1, &[]);
    }

    pub fn increment_messages_total(&self) {
        self.messages_total.add(1, &[]);
    }

    pub fn record_message_latency_ms(&self, ms: f64) {
        self.message_latency_ms.record(ms, &[]);
    }

    pub fn set_rooms_delta(&self, delta: i64) {
        self.rooms_active.add(delta, &[]);
    }

    pub fn increment_errors_total(&self, kind: &str) {
        self.errors_total
            .add(1, &[KeyValue::new("kind", kind.to_string())]);
    }
}
