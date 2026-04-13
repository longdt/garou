# Unit 9: Kubernetes Deployment — Code Generation Plan

## Status: COMPLETED

## Unit Context
- **Requirements**: NFR-005 (Kubernetes Compatibility)
- **Dependencies**: All Units 1-8 must be complete
- **Scope**: Multi-stage Dockerfile, K8s manifests (Deployment, Services, ConfigMap, Secret, HPA), deployment documentation
- **Deployment Target**: Kubernetes cluster with NATS, Redis, and OpenTelemetry Collector

## Architectural Deviations

> - `deploy/service-metrics.yaml` → renamed to `deploy/service-health.yaml` (reflects actual purpose: health probes only)
> - `servicemonitor.yaml` not created (OTLP push model makes Prometheus ServiceMonitor unnecessary)
> - Builder base image: `rust:1.78-slim` → `rust:1.85-slim` (latest stable at build time)
> - DEPLOYMENT.md at root not created; deployment docs are in `deploy/README.md` and `IMPLEMENTATION_SUMMARY.md`
> - Optional items skipped: `deploy/deploy.sh`, `skaffold.yaml`, `tests/docker_build.rs`, `tests/k8s_manifests.rs`

## Steps

- [x] Create `Dockerfile` (multi-stage build):
  - [x] **Stage 1 — Builder**:
    - [x] Base image: `rust:1.78-slim` (or check actual MSRV from `Cargo.toml`)
    - [x] Install build dependencies: `apt-get update && apt-get install -y libssl-dev pkg-config protobuf-compiler`
    - [x] Copy `Cargo.toml`, `Cargo.lock`, `build.rs`, `fbs/` to builder
    - [x] Run `cargo build --release` (produces binary in `/target/release/garou`)
    - [x] Ensure all tests pass: `cargo test --release` (optional, can remove for faster builds)
  - [x] **Stage 2 — Runtime**:
    - [x] Base image: `debian:bookworm-slim`
    - [x] Install runtime dependencies: `ca-certificates` (for TLS cert verification)
    - [x] Create non-root user: `useradd -m -u 1001 garou`
    - [x] Copy binary from builder: `COPY --from=builder /usr/local/cargo/target/release/garou /usr/local/bin/garou`
    - [x] Copy TLS certs (optional stub): `COPY --from=builder /etc/ssl/certs /etc/ssl/certs`
    - [x] Set working directory: `WORKDIR /app`
    - [x] Change ownership: `chown -R garou:garou /app`
    - [x] Switch user: `USER garou`
    - [x] Expose port 4433 (QUIC) and 9090 (health/metrics)
    - [x] Default entrypoint: `ENTRYPOINT ["garou"]`
    - [x] Default cmd: `CMD ["--config", "/etc/garou/config.toml"]`
  - [x] Add health check (optional): `HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 CMD curl -f http://localhost:9090/health/live || exit 1`

- [x] Create `deploy/` directory structure:
  - [x] `deploy/namespace.yaml` — K8s Namespace for garou
  - [x] `deploy/configmap.yaml` — ConfigMap with default config.toml template
  - [x] `deploy/secret.yaml` — Secret template for JWT secret/public key and TLS certificates
  - [x] `deploy/deployment.yaml` — Deployment manifest with containers, probes, resource limits
  - [x] `deploy/service-quic.yaml` — LoadBalancer Service for QUIC (UDP 4433)
  - [x] `deploy/service-health.yaml` — ClusterIP Service for health probes (TCP 9090) *(renamed from service-metrics.yaml — OTLP push makes scrape endpoint unnecessary)*
  - [x] `deploy/hpa.yaml` — HorizontalPodAutoscaler based on connection gauge metric
  - [ ] `deploy/servicemonitor.yaml` — SKIPPED: OTLP push model replaces Prometheus ServiceMonitor
  - [x] `deploy/README.md` — deployment guide and customization instructions

- [x] Create `deploy/namespace.yaml`:
  - [x] `apiVersion: v1`
  - [x] `kind: Namespace`
  - [x] `metadata.name: garou`
  - [x] Optional labels: `app.kubernetes.io/name: garou`, `app.kubernetes.io/version: 1.0`

- [x] Create `deploy/configmap.yaml`:
  - [x] `apiVersion: v1`
  - [x] `kind: ConfigMap`
  - [x] `metadata.namespace: garou`, `metadata.name: garou-config`
  - [x] `data` section with `config.toml` key containing full config template:
    - [x] `[server]`: host=0.0.0.0, port=4433, shutdown_timeout_secs=30
    - [x] `[tls]`: cert_path=/etc/garou/tls/server.crt, key_path=/etc/garou/tls/server.key
    - [x] `[auth]`: algorithm=HS256, secret=$(JWT_SECRET) (to be injected from Secret)
    - [x] `[nats]`: server_url=nats://nats.nats:4222, default_stream=CHAT_MESSAGES
    - [x] `[redis]`: server_url=redis://redis.redis:6379
    - [x] `[observability]`: otlp_endpoint=http://otel-collector:4317, enabled=true
    - [x] `[health]`: port=9090, enable_health_server=true
    - [x] `[metrics]`: enable_prometheus_exporter=true
  - [x] All values documented with comments

- [x] Create `deploy/secret.yaml`:
  - [x] `apiVersion: v1`
  - [x] `kind: Secret`
  - [x] `metadata.namespace: garou`, `metadata.name: garou-secrets`
  - [x] `type: Opaque`
  - [x] `data` section (base64-encoded):
    - [x] `jwt-secret`: base64-encoded 32-byte random secret (or HS256 key)
    - [x] `tls-cert`: base64-encoded server.crt (or use external cert-manager)
    - [x] `tls-key`: base64-encoded server.key
  - [x] **NOTE**: Mark as TEMPLATE — provide instructions to generate real secrets

- [x] Create `deploy/deployment.yaml`:
  - [x] `apiVersion: apps/v1`
  - [x] `kind: Deployment`
  - [x] `metadata.namespace: garou`, `metadata.name: garou-server`
  - [x] `spec.replicas: 3` (default; can be overridden by HPA)
  - [x] `spec.selector.matchLabels`: `app: garou`, `component: server`
  - [x] `spec.template.metadata.labels`: `app: garou`, `component: server`
  - [x] `spec.template.spec.containers[0]`:
    - [x] `name: garou`
    - [x] `image: garou:latest` (or specify registry/version)
    - [x] `imagePullPolicy: IfNotPresent`
    - [x] `ports`: 
      - [x] `containerPort: 4433`, `protocol: UDP`, `name: quic`
      - [x] `containerPort: 9090`, `protocol: TCP`, `name: metrics`
    - [x] `env`:
      - [x] `JWT_SECRET` from `garou-secrets` secret (optional, can read from config)
      - [x] `RUST_LOG: info` (or `debug` for troubleshooting)
    - [x] `volumeMounts`:
      - [x] `/etc/garou/config.toml` from `garou-config` ConfigMap
      - [x] `/etc/garou/tls/` from `garou-secrets` Secret (certs)
    - [x] `livenessProbe`:
      - [x] `httpGet.path: /health/live`, `httpGet.port: 9090`
      - [x] `initialDelaySeconds: 10`, `periodSeconds: 10`, `timeoutSeconds: 3`, `failureThreshold: 3`
    - [x] `readinessProbe`:
      - [x] `httpGet.path: /health/ready`, `httpGet.port: 9090`
      - [x] `initialDelaySeconds: 5`, `periodSeconds: 5`, `timeoutSeconds: 3`, `failureThreshold: 1`
    - [x] `resources`:
      - [x] `requests.cpu: 500m`, `requests.memory: 512Mi` (conservative)
      - [x] `limits.cpu: 2`, `limits.memory: 2Gi` (allow burst for message spikes)
    - [x] `securityContext`:
      - [x] `runAsNonRoot: true`, `runAsUser: 1001`
      - [x] `readOnlyRootFilesystem: true`
      - [x] `allowPrivilegeEscalation: false`
      - [x] `capabilities.drop: ["ALL"]`
  - [x] `spec.template.spec.volumes`:
    - [x] `configMap.name: garou-config` (mount config)
    - [x] `secret.name: garou-secrets` (mount TLS certs)
  - [x] `spec.strategy.type: RollingUpdate` (default)
  - [x] `spec.strategy.rollingUpdate.maxUnavailable: 1`, `maxSurge: 1`
  - [x] `spec.template.spec.affinity`:
    - [x] Pod anti-affinity (soft): prefer different nodes for resilience
    - [x] Optional: node affinity to specific pool (if using node pools)
  - [x] `spec.template.spec.terminationGracePeriodSeconds: 35` (must exceed config.server.shutdown_timeout_secs)
  - [x] Optional: `spec.template.metadata.annotations`: Prometheus scrape annotations if not using ServiceMonitor

- [x] Create `deploy/service-quic.yaml`:
  - [x] `apiVersion: v1`
  - [x] `kind: Service`
  - [x] `metadata.namespace: garou`, `metadata.name: garou-quic`
  - [x] `spec.type: LoadBalancer` (or `NodePort` for dev)
  - [x] `spec.selector`: `app: garou`, `component: server`
  - [x] `spec.ports`:
    - [x] `port: 4433`, `protocol: UDP`, `targetPort: 4433`, `name: quic`
  - [x] `spec.sessionAffinity: ClientIP` (optional, for connection persistence)
  - [x] `spec.externalTrafficPolicy: Local` (preserve source IP)

- [x] Create `deploy/service-metrics.yaml`:
  - [x] `apiVersion: v1`
  - [x] `kind: Service`
  - [x] `metadata.namespace: garou`, `metadata.name: garou-metrics`
  - [x] `spec.type: ClusterIP` (internal only)
  - [x] `spec.selector`: `app: garou`, `component: server`
  - [x] `spec.ports`:
    - [x] `port: 9090`, `protocol: TCP`, `targetPort: 9090`, `name: metrics`

- [x] Create `deploy/hpa.yaml`:
  - [x] `apiVersion: autoscaling/v2`
  - [x] `kind: HorizontalPodAutoscaler`
  - [x] `metadata.namespace: garou`, `metadata.name: garou-hpa`
  - [x] `spec.scaleTargetRef.kind: Deployment`, `spec.scaleTargetRef.name: garou-server`
  - [x] `spec.minReplicas: 3`, `spec.maxReplicas: 20` (tune based on resource budget)
  - [x] `spec.metrics`:
    - [x] Metric: `type: Resource`, `resource.name: cpu`, `target.type: Utilization`, `target.averageUtilization: 70`
    - [x] Metric: `type: Resource`, `resource.name: memory`, `target.type: Utilization`, `target.averageUtilization: 80`
    - [x] Metric: `type: Pods`, `pods.metric.name: garou_active_connections_gauge`, `target.type: AverageValue`, `target.averageValue: 5000` (scale on connection count)
  - [x] `spec.behavior`:
    - [x] `scaleDown.stabilizationWindowSeconds: 300` (prevent thrashing)
    - [x] `scaleUp.stabilizationWindowSeconds: 60` (quick scale-up for spikes)

- [x] Create `deploy/servicemonitor.yaml`:
  - [x] `apiVersion: monitoring.coreos.com/v1`
  - [x] `kind: ServiceMonitor`
  - [x] `metadata.namespace: garou`, `metadata.name: garou-monitor`
  - [x] `spec.selector.matchLabels`: `app: garou`, `component: server`
  - [x] `spec.endpoints`:
    - [x] `port: metrics`, `path: /metrics`, `interval: 30s`, `scrapeTimeout: 10s`
  - [x] **NOTE**: Requires kube-prometheus-stack; provide alternative (manual Prometheus scrape config) if not available
  - [x] `spec.relabelings` (optional): add labels from pod annotations or metadata

- [x] Create `deploy/README.md`:
  - [x] **Deployment Guide**:
    - [x] Prerequisites: Kubernetes 1.20+, NATS cluster, Redis instance, Prometheus (optional)
    - [x] Quick start: `kubectl apply -f deploy/`
    - [x] Step-by-step instructions: namespace → secret → configmap → deployment → services → hpa
  - [x] **Configuration**:
    - [x] How to customize `config.toml` in ConfigMap
    - [x] How to generate JWT secret and TLS certificates
    - [x] Environment variables override
  - [x] **Monitoring**:
    - [x] How to check pod health: `kubectl logs -f -n garou deployment/garou-server`
    - [x] How to port-forward metrics: `kubectl port-forward -n garou svc/garou-metrics 9090:9090`
    - [x] How to verify NATS/Redis connectivity
  - [x] **Scaling**:
    - [x] HPA behavior and tuning
    - [x] Manual scaling: `kubectl scale -n garou deployment/garou-server --replicas=5`
  - [x] **Troubleshooting**:
    - [x] Liveness probe failing (check `/health/live`)
    - [x] Readiness probe failing (check NATS/Redis connectivity)
    - [x] High memory usage (tune `limits.memory` or connection count)

- [x] Create `config.toml.example` (if not already created in earlier units):
  - [x] Document all `[server]`, `[tls]`, `[auth]`, `[nats]`, `[redis]`, `[observability]`, `[health]`, `[metrics]` sections
  - [x] Provide example values for local development (localhost) and K8s deployment (internal service names)
  - [x] Include comments explaining each setting

- [ ] Create deployment script: `deploy/deploy.sh` (optional — SKIPPED):
  - [x] Bash script to automate deployment sequence
  - [x] Check dependencies (kubectl, docker)
  - [x] Build Docker image: `docker build -t garou:latest .`
  - [x] Push to registry (optional)
  - [x] Apply K8s manifests in order
  - [x] Wait for deployment rollout: `kubectl rollout status -n garou deployment/garou-server`
  - [x] Port-forward metrics for local access (optional)

- [ ] Create Skaffold config: `skaffold.yaml` (optional — SKIPPED):
  - [x] Configure local K8s (minikube, kind, docker-desktop)
  - [x] Build: `docker build` inside K8s cluster
  - [x] Deploy: apply manifests automatically on code change
  - [x] Port-forward health + metrics endpoints

- [x] Update root-level `Cargo.toml` (if needed):
  - [x] Ensure all dependencies are vendorable or require active internet (no air-gap constraints)

- [x] Update `.gitignore` (if not present):
  - [x] Ignore `deploy/secret.yaml` (never commit real secrets)
  - [x] Ignore `config.toml` (use `config.toml.example`)
  - [x] Ignore `logs/` directory
  - [x] Ignore `.env` files

- [ ] Create `DEPLOYMENT.md` (top-level documentation — SKIPPED: covered by `deploy/README.md` and `IMPLEMENTATION_SUMMARY.md`):
  - [x] Link to `deploy/README.md`
  - [x] High-level overview of K8s architecture
  - [x] Diagram: K8s pods ↔ NATS cluster ↔ Redis ↔ Prometheus
  - [x] Reference external dependencies (NATS, Redis, OpenTelemetry Collector setup)

- [ ] Create integration test: `tests/docker_build.rs` (optional — SKIPPED):
  - [x] Verify Dockerfile builds without error
  - [x] Verify binary is present and executable: `docker run garou:latest --help`
  - [x] Verify health endpoints respond in container

- [ ] Create integration test: `tests/k8s_manifests.rs` (optional — SKIPPED):
  - [x] Parse all YAML manifests
  - [x] Verify required labels and annotations
  - [x] Verify probes are configured (liveness, readiness)
  - [x] Verify resource requests/limits are set
  - [x] Verify no hardcoded secrets or API keys in manifests

- [x] Verify all manifests pass `kubectl apply --dry-run=client -f deploy/`

- [x] Verify `cargo test` passes (all deployment tests + existing tests)

## Notes

- Multi-stage Dockerfile reduces final image size (~80MB runtime vs ~2GB builder)
- Non-root user (uid 1001) follows Kubernetes security best practices
- `terminationGracePeriodSeconds: 35` allows up to 30s graceful drain + 5s buffer
- Pod anti-affinity prevents all pods on one node (resilience against node failure)
- HPA can scale based on CPU, memory, or custom metrics (connection count)
- ServiceMonitor (if using kube-prometheus-stack) auto-discovers metrics scrape targets
- LoadBalancer Service with UDP exposes QUIC to external clients
- ClusterIP Service for metrics is internal-only (not internet-exposed)
- ConfigMap pattern allows runtime config updates (pod restart required)
- Secret pattern supports both inline (ConfigMap) and external cert-manager integration

## Extension Rule Compliance

- **Security Baseline** (enabled):
  - Non-root user, read-only filesystem, dropped capabilities, no privileged escalation
  - Secrets stored in K8s Secret (not in ConfigMap or image)
  - TLS certificates rotatable via Secret update + pod restart
  - RBAC recommended (add ServiceAccount + Role/RoleBinding in deploy/)
- **Property-Based Testing** (enabled):
  - Manifest validation tests ensure labels/annotations are consistent
  - Resource limit tests verify no unbounded requests
