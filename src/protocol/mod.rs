//! Protocol layer for the QUIC chat server
//!
//! This module provides:
//! - Binary frame encoding/decoding
//! - Message type definitions
//! - Codec traits for serialization

pub mod codec;
pub mod frame;
pub mod messages;

// Re-export commonly used types
pub use codec::{Decodable, DecodedMessage, Encodable, decode, encode};
pub use frame::{FRAME_HEADER_SIZE, Frame, FrameCodec, FrameType, MAX_FRAME_SIZE};
pub use messages::*;
