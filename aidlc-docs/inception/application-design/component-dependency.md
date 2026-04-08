# Component Dependencies

## Dependency Matrix

| Component | Depends On |
|-----------|-----------|
| Config | (none — root dependency) |
| Auth | Config, Redis (optional) |
| NATS Storage | Config |
| Redis Cache | Config |
| Metrics | (none — global registry) |
| Health | NATS Storage, Redis Cache, Metrics |
| Protocol | (none — pure data transformation) |
| Transport | Config |
| Connection Handler | Config, Auth, Protocol, Transport, Metrics |
| Room Manager | Protocol (message types) |
| Multi-Stream Server | Config, Auth, NATS Storage, Redis Cache, Room Manager, Transport, Metrics, Connection Handler |

## Dependency Graph (text)

```
Config ──────────────────────────────────────────────────────────────┐
  │                                                                   │
  ├──> Auth ──────────────────────────────────────────────────────┐  │
  │      └──> Redis Cache (optional) ─────────────────────────┐  │  │
  │                                                            │  │  │
  ├──> NATS Storage ──────────────────────────────────────┐   │  │  │
  │                                                       │   │  │  │
  ├──> Redis Cache ────────────────────────────────────┐  │   │  │  │
  │                                                    │  │   │  │  │
  │   Protocol (no config dep) ──────────────────────┐ │  │   │  │  │
  │                                                  │ │  │   │  │  │
  ├──> Transport ────────────────────────────────────┼─┼──┼───┼──┼──┤
  │                                                  │ │  │   │  │  │
  └──> Connection Handler ←──────────────────────────┘ │  │   │  │  │
         (uses Auth, Protocol, Transport, Metrics)      │  │   │  │  │
                                                        │  │   │  │  │
Health ←─────────────────────────────────────────────── │──┘   │  │  │
  (monitors NATS + Redis, exposes Metrics)               │      │  │  │
                                                         │      │  │  │
Multi-Stream Server ←────────────────────────────────────┘──────┘──┘──┘
  (orchestrates everything)
  └──> Room Manager (local cache, depends on Protocol types)
```

## Communication Patterns

### Synchronous (direct async calls)
- `MultiStreamServer` → `NatsClient.publish_message()` (on every SendMessage)
- `ConnectionHandler` → `AuthValidator.validate()` (once per connection handshake)
- `HealthServer` → `NatsClient.health_check()` + `RedisClient.health_check()` (on /health/ready)
- `MultiStreamServer` → `RedisClient.get_roster()` (for broadcast optimization)

### Asynchronous (channels / subscriptions)
- `ConnectionHandler` → `MultiStreamServer`: `mpsc::channel<ServerEvent>` (bounded, 1024)
- `MultiStreamServer` → `ConnectionHandler`: `mpsc::channel<ConnectionCommand>` (bounded, 1024)
- `NatsClient` subscription → `MultiStreamServer`: cross-node message delivery stream

### Shared State (Arc<RwLock<...>>)
- `connections`: `Arc<RwLock<HashMap<ConnId, ActiveConnection>>>` — read by broadcast, written on connect/disconnect
- `user_connections`: `Arc<RwLock<HashMap<UserId, ConnId>>>` — read by routing, written on auth/disconnect
- `RoomManager`: `Arc<RoomManager>` — shared across connection handlers

## Arc Ownership Model

```
Arc<MultiStreamServer>           ← Created once in main, passed to all connection tasks
  ├── Arc<Config>                ← Shared read-only
  ├── Arc<AuthValidator>         ← Shared, holds Arc<RedisClient>
  ├── Arc<NatsClient>            ← Shared, owns connection pool
  ├── Arc<RedisClient>           ← Shared, owns connection pool
  ├── Arc<RoomManager>           ← Shared, internal RwLocks
  ├── Arc<ShardRouter>           ← Shared, internal atomics + RwLocks
  └── Arc<RwLock<connections>>   ← Shared mutable state

Per-connection (ConnectionHandler, NOT shared):
  ├── Arc<AuthValidator>         ← Cloned from server
  ├── Arc<ShardRouter>           ← Cloned from server
  └── quinn::Connection          ← Owned by this handler
```

## Data Flow Diagram: Message Lifecycle

```
Client Pod A                    Server Pod 1 (garou)            Server Pod 2 (garou)
     │                               │                               │
     │── ChatCommands stream ────────►│                               │
     │   [SendMessage FlatBuffer]     │                               │
     │                               │ ConnectionHandler decodes      │
     │                               │ emit ServerEvent::SendMessage  │
     │                               │                               │
     │                               │ MultiStreamServer              │
     │                               │  validate membership           │
     │                               │  build RoomMessage             │
     │                               │  NatsClient.publish() ─────────────────────► NATS JetStream
     │                               │                               │              (persist)
     │                               │  MessageAck → sender ◄────────│
     │                               │                               │
     │◄── Shard/HotRoom stream ───────│                               │
     │   [RoomMessage FlatBuffer]     │ broadcast to local members    │
     │                               │                               │
     │                        NATS subscription fires ───────────────►│
     │                               │                               │ MultiStreamServer
     │                               │                               │ broadcast to local members
     │                               │                               │    │
     │                               │                          Client Pod B
     │                               │                         (connected to Pod 2)
     │                               │                         receives RoomMessage
```
