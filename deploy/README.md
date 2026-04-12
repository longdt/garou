# Garou — Kubernetes Deployment Guide

## Prerequisites

- Kubernetes 1.25+
- NATS cluster (namespace `nats`, service `nats`)
- Redis instance (namespace `redis`, service `redis`)
- OpenTelemetry Collector (namespace `monitoring`, service `otel-collector`, port 4317)
- `kubectl` configured for your cluster

## Quick Start

```bash
# 1. Generate a real JWT secret and update deploy/secret.yaml
echo -n "$(openssl rand -hex 32)" | base64

# 2. Apply all manifests in order
kubectl apply -f deploy/namespace.yaml
kubectl apply -f deploy/secret.yaml
kubectl apply -f deploy/configmap.yaml
kubectl apply -f deploy/deployment.yaml
kubectl apply -f deploy/service-quic.yaml
kubectl apply -f deploy/service-health.yaml
kubectl apply -f deploy/hpa.yaml

# 3. Watch rollout
kubectl rollout status -n garou deployment/garou-server
```

## Configuration

Edit `deploy/configmap.yaml` to adjust NATS/Redis URLs, OTLP endpoint, and other settings.
After editing:
```bash
kubectl apply -f deploy/configmap.yaml
kubectl rollout restart -n garou deployment/garou-server
```

## Health Probes

| Path | Purpose |
|------|---------|
| `GET /health/live` | Liveness — always 200 while running |
| `GET /health/ready` | Readiness — 503 during startup, shutdown, or if NATS/Redis unhealthy |

Port-forward for local inspection:
```bash
kubectl port-forward -n garou svc/garou-health 9090:9090
curl http://localhost:9090/health/live
curl http://localhost:9090/health/ready
```

## Observability

Metrics, traces, and logs are pushed via OTLP gRPC to the configured collector.
No Prometheus scrape endpoint is exposed.

To verify export:
```bash
kubectl logs -n garou -l app=garou --tail=50
```

## Scaling

The HPA scales on CPU (70%) and memory (80%) between 3–20 replicas.

Manual override:
```bash
kubectl scale -n garou deployment/garou-server --replicas=5
```

## Secrets

Never commit real secrets. Rotate by updating `garou-secrets` and restarting:
```bash
kubectl create secret generic garou-secrets -n garou \
  --from-literal=jwt-secret="$(openssl rand -hex 32)" \
  --dry-run=client -o yaml | kubectl apply -f -
kubectl rollout restart -n garou deployment/garou-server
```

## Troubleshooting

| Symptom | Check |
|---------|-------|
| Liveness failing | `kubectl logs -n garou <pod>` for panic/startup error |
| Readiness failing | NATS/Redis connectivity; check `config.toml` URLs in ConfigMap |
| High memory | Tune `limits.memory` or reduce `max_connections` in ConfigMap |
| OTLP not exporting | Verify `otel-collector` is reachable at port 4317 |
