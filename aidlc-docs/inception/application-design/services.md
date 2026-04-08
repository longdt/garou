# Services — Orchestration Patterns

## Service 1: Bootstrap Service (in `main.rs`)

**Responsibility**: Ordered initialization of all components at startup.

**Orchestration sequence**:
```
1. Parse CLI args
2. Config::load(path, cli_args)          → Arc<Config>
3. init_metrics()                        → PrometheusHandle
4. RedisClient::connect(config.redis)    → Arc<RedisClient>
5. NatsClient::connect(config.nats)      → Arc<NatsClient>
6. nats.init_infrastructure()            → ensure streams/KV exist
7. AuthValidator::new(config.auth, redis)→ Arc<AuthValidator>
8. MultiStreamServer::new(...)           → Arc<MultiStreamServer>
9. HealthServer::new(nats, redis, ...).start(shutdown_token) → Tokio task
10. install signal handlers (SIGTERM, SIGINT)
11. server.start().await                 → runs until shutdown signal
12. server.shutdown().await              → graceful drain
```

**Failure policy**: Any step 2–8 failure → log error + exit(1). All external deps (NATS, Redis) must be reachable at startup.

---

## Service 2: Message Processing Service (in `MultiStreamServer`)

**Responsibility**: Coordinate the full lifecycle of a sent message from receipt to broadcast.

**Orchestration flow for `SendMessage`**:
```
ConnectionHandler emits ServerEvent::SendMessage
  │
  ▼
MultiStreamServer.handle_send_message()
  │
  ├─1. Validate: user authenticated, is member of room_id
  │
  ├─2. Generate MessageId (AtomicU64::fetch_add)
  │
  ├─3. Build RoomMessage struct
  │
  ├─4. NatsClient.publish_message(room_id, &msg)   ← persist FIRST
  │     │ on Err → return Error frame to sender, abort
  │     │ on Ok  → continue
  │
  ├─5. Send MessageAck to sender connection
  │
  ├─6. RoomManager.add_message(msg.clone())         ← update local cache
  │
  ├─7. ShardRouter.route_message(room_id)           ← update rate stats
  │
  └─8. broadcast_to_room(room_id, msg)
          │
          ├─ Local: iterate connections map → send ConnectionCommand to each local member
          └─ Remote: NATS pub/sub delivers to other pod subscribers (handled by their sub loop)
```

**Failure modes**:
- NATS publish fails → `ChatError::Internal` returned to sender; message NOT broadcast
- Local broadcast channel full → drop with warning (backpressure; client will timeout)

---

## Service 3: Authentication Service (in `ConnectionHandler`)

**Responsibility**: Validate JWT during protocol handshake, block all commands until auth succeeds.

**Orchestration flow**:
```
Client sends Hello frame
  → ConnectionHandler sends HelloAck (with session_id, num_shards)
  → Server opens Shard streams to client

Client sends Auth{method: "jwt", credentials: <token>}
  → AuthValidator.validate(token)
      ├─ Redis cache hit? → return cached AuthClaims
      ├─ Redis miss/unavailable? → validate JWT inline
      └─ Invalid/expired? → return AuthFailed frame, close connection

Auth success:
  → Store AuthClaims in ConnectionHandler state
  → Emit ServerEvent::Authenticated{user_id, username}
  → Begin accepting ChatCommands, ACK frames
```

---

## Service 4: Cross-Node Delivery Service (in `NatsClient` + `MultiStreamServer`)

**Responsibility**: Deliver messages to users connected to other pod instances.

**Orchestration flow**:
```
When user joins room:
  → MultiStreamServer calls NatsClient.subscribe_room(room_id)
  → NATS subscription registered on this pod

When message published to NATS (from any pod):
  → This pod's subscription fires with RoomMessage
  → MultiStreamServer receives message via subscription channel
  → broadcast_to_local_members(room_id, msg)
      → Send to all connections on THIS pod that are in the room

When user leaves/disconnects:
  → Check if any other local users are still in that room
  → If no more local users → NatsClient.unsubscribe_room(room_id)
```

---

## Service 5: Graceful Shutdown Service (in `main.rs` + `MultiStreamServer`)

**Responsibility**: Drain connections and flush state on SIGTERM.

**Orchestration flow**:
```
SIGTERM / SIGINT received
  → CancellationToken.cancel()
  → HealthServer.ready = false (readiness probe returns 503 → K8s stops routing)
  → Wait config.server.shutdown_timeout_secs for in-flight messages
  → Send Goodbye frame to all connected clients
  → Close QUIC endpoint (stop accepting new connections)
  → NatsClient.close()       (flush pending publishes)
  → RedisClient.close()      (return connections to pool)
  → metrics flush (final scrape opportunity)
  → exit(0)
```
