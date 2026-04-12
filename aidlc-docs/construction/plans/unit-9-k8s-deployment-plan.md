# Unit 9: Kubernetes Deployment — Code Generation Plan

## Status: PENDING

## Unit Context
- **Stories Implemented**: FR-016 (K8s deployment), FR-011 (Auto-scaling)
- **Dependencies**: All Units 1-8 must be complete
- **Scope**: Multi-stage Dockerfile, K8s manifests (Deployment, Services, ConfigMap, Secret, HPA, ServiceMonitor), deployment documentation
- **Deployment Target**: Kubernetes cluster with NATS, Redis, and Prometheus observability stack

## Steps

- [ ] Create `Dockerfile` (multi-stage build):
  - [ ] **Stage 1 — Builder**:
    - [ ] Base image: `rust:1.78-slim` (or check actual MSRV from `Cargo.toml`)
    - [ ] Install build dependencies: `apt-get update && apt-get install -y libssl-dev pkg-config protobuf-compiler`
    - [ ] Copy `Cargo.toml`, `Cargo.lock`, `build.rs`, `fbs/` to builder
    - [ ] Run `cargo build --release` (produces binary in `/target/release/garou`)
    - [ ] Ensure all tests pass: `cargo test --release` (optional, can remove for faster builds)
  - [ ] **Stage 2 — Runtime**:
    - [ ] Base image: `debian:bookworm-slim`
    - [ ] Install runtime dependencies: `ca-certificates` (for TLS cert verification)
    - [ ] Create non-root user: `useradd -m -u 1001 garou`
    - [ ] Copy binary from builder: `COPY --from=builder /usr/local/cargo/target/release/garou /usr/local/bin/garou`
    - [ ] Copy TLS certs (optional stub): `COPY --from=builder /etc/ssl/certs /etc/ssl/certs`
    - [ ] Set working directory: `WORKDIR /app`
    - [ ] Change ownership: `chown -R garou:garou /app`
    - [ ] Switch user: `USER garou`
    - [ ] Expose port 4433 (QUIC) and 9090 (health/metrics)
    - [ ] Default entrypoint: `ENTRYPOINT ["garou"]`
    - [ ] Default cmd: `CMD ["--config", "/etc/garou/config.toml"]`
  - [ ] Add health check (optional): `HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 CMD curl -f http://localhost:9090/health/live || exit 1`

- [ ] Create `deploy/` directory structure:
  - [ ] `deploy/namespace.yaml` — K8s Namespace for garou
  - [ ] `deploy/configmap.yaml` — ConfigMap with default config.toml template
  - [ ] `deploy/secret.yaml` — Secret template for JWT secret/public key and TLS certificates
  - [ ] `deploy/deployment.yaml` — Deployment manifest with containers, probes, resource limits
  - [ ] `deploy/service-quic.yaml` — LoadBalancer Service for QUIC (UDP 4433)
  - [ ] `deploy/service-metrics.yaml` — ClusterIP Service for metrics (TCP 9090)
  - [ ] `deploy/hpa.yaml` — HorizontalPodAutoscaler based on connection gauge metric
  - [ ] `deploy/servicemonitor.yaml` — ServiceMonitor for Prometheus scraping (if kube-prometheus-stack installed)
  - [ ] `deploy/README.md` — deployment guide and customization instructions

- [ ] Create `deploy/namespace.yaml`:
  - [ ] `apiVersion: v1`
  - [ ] `kind: Namespace`
  - [ ] `metadata.name: garou`
  - [ ] Optional labels: `app.kubernetes.io/name: garou`, `app.kubernetes.io/version: 1.0`

- [ ] Create `deploy/configmap.yaml`:
  - [ ] `apiVersion: v1`
  - [ ] `kind: ConfigMap`
  - [ ] `metadata.namespace: garou`, `metadata.name: garou-config`
  - [ ] `data` section with `config.toml` key containing full config template:
    - [ ] `[server]`: host=0.0.0.0, port=4433, shutdown_timeout_secs=30
    - [ ] `[tls]`: cert_path=/etc/garou/tls/server.crt, key_path=/etc/garou/tls/server.key
    - [ ] `[auth]`: algorithm=HS256, secret=$(JWT_SECRET) (to be injected from Secret)
    - [ ] `[nats]`: server_url=nats://nats.nats:4222, default_stream=CHAT_MESSAGES
    - [ ] `[redis]`: server_url=redis://redis.redis:6379
    - [ ] `[observability]`: otlp_endpoint=http://otel-collector:4317, enabled=true
    - [ ] `[health]`: port=9090, enable_health_server=true
    - [ ] `[metrics]`: enable_prometheus_exporter=true
  - [ ] All values documented with comments

- [ ] Create `deploy/secret.yaml`:
  - [ ] `apiVersion: v1`
  - [ ] `kind: Secret`
  - [ ] `metadata.namespace: garou`, `metadata.name: garou-secrets`
  - [ ] `type: Opaque`
  - [ ] `data` section (base64-encoded):
    - [ ] `jwt-secret`: base64-encoded 32-byte random secret (or HS256 key)
    - [ ] `tls-cert`: base64-encoded server.crt (or use external cert-manager)
    - [ ] `tls-key`: base64-encoded server.key
  - [ ] **NOTE**: Mark as TEMPLATE — provide instructions to generate real secrets

- [ ] Create `deploy/deployment.yaml`:
  - [ ] `apiVersion: apps/v1`
  - [ ] `kind: Deployment`
  - [ ] `metadata.namespace: garou`, `metadata.name: garou-server`
  - [ ] `spec.replicas: 3` (default; can be overridden by HPA)
  - [ ] `spec.selector.matchLabels`: `app: garou`, `component: server`
  - [ ] `spec.template.metadata.labels`: `app: garou`, `component: server`
  - [ ] `spec.template.spec.containers[0]`:
    - [ ] `name: garou`
    - [ ] `image: garou:latest` (or specify registry/version)
    - [ ] `imagePullPolicy: IfNotPresent`
    - [ ] `ports`: 
      - [ ] `containerPort: 4433`, `protocol: UDP`, `name: quic`
      - [ ] `containerPort: 9090`, `protocol: TCP`, `name: metrics`
    - [ ] `env`:
      - [ ] `JWT_SECRET` from `garou-secrets` secret (optional, can read from config)
      - [ ] `RUST_LOG: info` (or `debug` for troubleshooting)
    - [ ] `volumeMounts`:
      - [ ] `/etc/garou/config.toml` from `garou-config` ConfigMap
      - [ ] `/etc/garou/tls/` from `garou-secrets` Secret (certs)
    - [ ] `livenessProbe`:
      - [ ] `httpGet.path: /health/live`, `httpGet.port: 9090`
      - [ ] `initialDelaySeconds: 10`, `periodSeconds: 10`, `timeoutSeconds: 3`, `failureThreshold: 3`
    - [ ] `readinessProbe`:
      - [ ] `httpGet.path: /health/ready`, `httpGet.port: 9090`
      - [ ] `initialDelaySeconds: 5`, `periodSeconds: 5`, `timeoutSeconds: 3`, `failureThreshold: 1`
    - [ ] `resources`:
      - [ ] `requests.cpu: 500m`, `requests.memory: 512Mi` (conservative)
      - [ ] `limits.cpu: 2`, `limits.memory: 2Gi` (allow burst for message spikes)
    - [ ] `securityContext`:
      - [ ] `runAsNonRoot: true`, `runAsUser: 1001`
      - [ ] `readOnlyRootFilesystem: true`
      - [ ] `allowPrivilegeEscalation: false`
      - [ ] `capabilities.drop: ["ALL"]`
  - [ ] `spec.template.spec.volumes`:
    - [ ] `configMap.name: garou-config` (mount config)
    - [ ] `secret.name: garou-secrets` (mount TLS certs)
  - [ ] `spec.strategy.type: RollingUpdate` (default)
  - [ ] `spec.strategy.rollingUpdate.maxUnavailable: 1`, `maxSurge: 1`
  - [ ] `spec.template.spec.affinity`:
    - [ ] Pod anti-affinity (soft): prefer different nodes for resilience
    - [ ] Optional: node affinity to specific pool (if using node pools)
  - [ ] `spec.template.spec.terminationGracePeriodSeconds: 35` (must exceed config.server.shutdown_timeout_secs)
  - [ ] Optional: `spec.template.metadata.annotations`: Prometheus scrape annotations if not using ServiceMonitor

- [ ] Create `deploy/service-quic.yaml`:
  - [ ] `apiVersion: v1`
  - [ ] `kind: Service`
  - [ ] `metadata.namespace: garou`, `metadata.name: garou-quic`
  - [ ] `spec.type: LoadBalancer` (or `NodePort` for dev)
  - [ ] `spec.selector`: `app: garou`, `component: server`
  - [ ] `spec.ports`:
    - [ ] `port: 4433`, `protocol: UDP`, `targetPort: 4433`, `name: quic`
  - [ ] `spec.sessionAffinity: ClientIP` (optional, for connection persistence)
  - [ ] `spec.externalTrafficPolicy: Local` (preserve source IP)

- [ ] Create `deploy/service-metrics.yaml`:
  - [ ] `apiVersion: v1`
  - [ ] `kind: Service`
  - [ ] `metadata.namespace: garou`, `metadata.name: garou-metrics`
  - [ ] `spec.type: ClusterIP` (internal only)
  - [ ] `spec.selector`: `app: garou`, `component: server`
  - [ ] `spec.ports`:
    - [ ] `port: 9090`, `protocol: TCP`, `targetPort: 9090`, `name: metrics`

- [ ] Create `deploy/hpa.yaml`:
  - [ ] `apiVersion: autoscaling/v2`
  - [ ] `kind: HorizontalPodAutoscaler`
  - [ ] `metadata.namespace: garou`, `metadata.name: garou-hpa`
  - [ ] `spec.scaleTargetRef.kind: Deployment`, `spec.scaleTargetRef.name: garou-server`
  - [ ] `spec.minReplicas: 3`, `spec.maxReplicas: 20` (tune based on resource budget)
  - [ ] `spec.metrics`:
    - [ ] Metric: `type: Resource`, `resource.name: cpu`, `target.type: Utilization`, `target.averageUtilization: 70`
    - [ ] Metric: `type: Resource`, `resource.name: memory`, `target.type: Utilization`, `target.averageUtilization: 80`
    - [ ] Metric: `type: Pods`, `pods.metric.name: garou_active_connections_gauge`, `target.type: AverageValue`, `target.averageValue: 5000` (scale on connection count)
  - [ ] `spec.behavior`:
    - [ ] `scaleDown.stabilizationWindowSeconds: 300` (prevent thrashing)
    - [ ] `scaleUp.stabilizationWindowSeconds: 60` (quick scale-up for spikes)

- [ ] Create `deploy/servicemonitor.yaml`:
  - [ ] `apiVersion: monitoring.coreos.com/v1`
  - [ ] `kind: ServiceMonitor`
  - [ ] `metadata.namespace: garou`, `metadata.name: garou-monitor`
  - [ ] `spec.selector.matchLabels`: `app: garou`, `component: server`
  - [ ] `spec.endpoints`:
    - [ ] `port: metrics`, `path: /metrics`, `interval: 30s`, `scrapeTimeout: 10s`
  - [ ] **NOTE**: Requires kube-prometheus-stack; provide alternative (manual Prometheus scrape config) if not available
  - [ ] `spec.relabelings` (optional): add labels from pod annotations or metadata

- [ ] Create `deploy/README.md`:
  - [ ] **Deployment Guide**:
    - [ ] Prerequisites: Kubernetes 1.20+, NATS cluster, Redis instance, Prometheus (optional)
    - [ ] Quick start: `kubectl apply -f deploy/`
    - [ ] Step-by-step instructions: namespace → secret → configmap → deployment → services → hpa
  - [ ] **Configuration**:
    - [ ] How to customize `config.toml` in ConfigMap
    - [ ] How to generate JWT secret and TLS certificates
    - [ ] Environment variables override
  - [ ] **Monitoring**:
    - [ ] How to check pod health: `kubectl logs -f -n garou deployment/garou-server`
    - [ ] How to port-forward metrics: `kubectl port-forward -n garou svc/garou-metrics 9090:9090`
    - [ ] How to verify NATS/Redis connectivity
  - [ ] **Scaling**:
    - [ ] HPA behavior and tuning
    - [ ] Manual scaling: `kubectl scale -n garou deployment/garou-server --replicas=5`
  - [ ] **Troubleshooting**:
    - [ ] Liveness probe failing (check `/health/live`)
    - [ ] Readiness probe failing (check NATS/Redis connectivity)
    - [ ] High memory usage (tune `limits.memory` or connection count)

- [ ] Create `config.toml.example` (if not already created in earlier units):
  - [ ] Document all `[server]`, `[tls]`, `[auth]`, `[nats]`, `[redis]`, `[observability]`, `[health]`, `[metrics]` sections
  - [ ] Provide example values for local development (localhost) and K8s deployment (internal service names)
  - [ ] Include comments explaining each setting

- [ ] Create deployment script: `deploy/deploy.sh` (optional):
  - [ ] Bash script to automate deployment sequence
  - [ ] Check dependencies (kubectl, docker)
  - [ ] Build Docker image: `docker build -t garou:latest .`
  - [ ] Push to registry (optional)
  - [ ] Apply K8s manifests in order
  - [ ] Wait for deployment rollout: `kubectl rollout status -n garou deployment/garou-server`
  - [ ] Port-forward metrics for local access (optional)

- [ ] Create Skaffold config: `skaffold.yaml` (optional, for local development):
  - [ ] Configure local K8s (minikube, kind, docker-desktop)
  - [ ] Build: `docker build` inside K8s cluster
  - [ ] Deploy: apply manifests automatically on code change
  - [ ] Port-forward health + metrics endpoints

- [ ] Update root-level `Cargo.toml` (if needed):
  - [ ] Ensure all dependencies are vendorable or require active internet (no air-gap constraints)

- [ ] Update `.gitignore` (if not present):
  - [ ] Ignore `deploy/secret.yaml` (never commit real secrets)
  - [ ] Ignore `config.toml` (use `config.toml.example`)
  - [ ] Ignore `logs/` directory
  - [ ] Ignore `.env` files

- [ ] Create `DEPLOYMENT.md` (top-level documentation):
  - [ ] Link to `deploy/README.md`
  - [ ] High-level overview of K8s architecture
  - [ ] Diagram: K8s pods ↔ NATS cluster ↔ Redis ↔ Prometheus
  - [ ] Reference external dependencies (NATS, Redis, OpenTelemetry Collector setup)

- [ ] Create integration test: `tests/docker_build.rs` (optional):
  - [ ] Verify Dockerfile builds without error
  - [ ] Verify binary is present and executable: `docker run garou:latest --help`
  - [ ] Verify health endpoints respond in container

- [ ] Create integration test: `tests/k8s_manifests.rs` (optional):
  - [ ] Parse all YAML manifests
  - [ ] Verify required labels and annotations
  - [ ] Verify probes are configured (liveness, readiness)
  - [ ] Verify resource requests/limits are set
  - [ ] Verify no hardcoded secrets or API keys in manifests

- [ ] Verify all manifests pass `kubectl apply --dry-run=client -f deploy/`

- [ ] Verify `cargo test` passes (all deployment tests + existing tests)

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
