//! Persistent storage backends for Garou.
//!
//! Unit 5: NATS JetStream for message persistence and cross-node pub/sub.
//! Unit 6: Redis for JWT caching, presence, and room rosters.

pub mod nats;
pub mod redis;

pub use nats::{NatsClient, RoomState, RoomSubscription};
pub use redis::RedisClient;
