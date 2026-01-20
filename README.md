# Garou - QUIC Chat Server

A high-performance chat server implementation using QUIC protocol for communication and JSON for serialization/deserialization.

## Features

- **QUIC Protocol**: Fast, secure, and reliable communication using Quinn
- **JSON Serialization**: Simple and efficient JSON-based message format
- **Real-time Messaging**: Instant message delivery to all connected clients
- **User Management**: Join/leave notifications and user list functionality
- **Connection Management**: Automatic cleanup and connection limits
- **TLS Security**: Self-signed certificates for development (customizable)

## Quick Start

The fastest way to see the chat server in action:

```bash
# Clone and build (this may take a few minutes due to cryptography dependencies)
git clone <repository-url>
cd garou
cargo build --release

# Run the demo to see everything working
cargo run --release -- demo
```

This will start a server and create multiple automated clients that chat with each other.

## Architecture

The chat server is built with a modular architecture:

- `ChatServer`: QUIC server handling client connections and message routing
- `ChatClient`: QUIC client for connecting to the server and sending messages
- JSON serialization for simple and efficient message exchange
- `User` and `ChatMessage`: Core data structures
- Custom error handling with detailed error types

## Requirements

- Rust 1.75+ with 2024 edition support
- Optional: FlatBuffers compiler (`flatc`) for schema regeneration

## Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd garou
```

2. Build the project:
```bash
cargo build --release
```

## Usage

### Running the Server

Start the chat server:

```bash
cargo run -- server
```

The server will start listening on `127.0.0.1:4433` by default.

### Running a Client

Connect to the server with a username:

```bash
cargo run -- client Alice
```

Multiple clients can connect simultaneously. Type messages and press Enter to send them to all connected users.

### Running the Demo

See the system in action with multiple automated clients:

```bash
cargo run -- demo
```

This will start a server and create several clients that automatically send messages.

### Commands

Once connected as a client:
- Type any message and press Enter to send it to all users
- Type `quit` to disconnect and exit

## Configuration

### Server Configuration

```rust
let config = ChatConfig {
    bind_addr: "127.0.0.1:4433".parse()?,
    max_connections: 100,
    idle_timeout_secs: 300,
    max_message_size: 1024 * 1024, // 1MB
};
```

### Client Configuration

```rust
let config = ClientConfig {
    server_addr: "127.0.0.1:4433".parse()?,
    bind_addr: "0.0.0.0:0".parse()?,
    connect_timeout_secs: 10,
    keep_alive_secs: 30,
    max_message_size: 1024 * 1024,
};
```

## Message Types

The system supports several message types:

- **Text**: Regular chat messages
- **Join**: User join notifications
- **Leave**: User leave notifications
- **UserList**: Request/response for current online users
- **Error**: Error notifications

## FlatBuffers Schema

The message schema is defined in `schemas/chat.fbs`. To regenerate the Rust code:

1. Install FlatBuffers compiler
2. Set environment variable: `export GAROU_USE_FLATC=1`
3. Rebuild: `cargo clean && cargo build`

## Development

### Running Tests

```bash
cargo test
```

### Enabling Debug Logs

```bash
RUST_LOG=garou=debug cargo run -- server
```

### Project Structure

```
garou/
├── src/
│   ├── main.rs          # CLI application entry point
│   ├── lib.rs           # Library root and core types
│   ├── server.rs        # QUIC chat server implementation
│   ├── client.rs        # QUIC chat client implementation
│   ├── message.rs       # FlatBuffers serialization
│   ├── error.rs         # Error handling
│   └── chat_generated.rs # Generated FlatBuffers code
├── schemas/
│   └── chat.fbs         # FlatBuffers schema definition
├── build.rs             # Build script for code generation
└── Cargo.toml           # Dependencies and project config
```

## Security Considerations

⚠️ **Important**: The current implementation uses self-signed certificates and accepts any certificate for development purposes. For production use:

1. Replace the certificate generation with proper CA-signed certificates
2. Implement proper certificate validation
3. Add authentication and authorization mechanisms
4. Consider rate limiting and abuse prevention

## Performance

The system is designed for high performance:

- QUIC provides better performance than TCP in many scenarios
- FlatBuffers offers zero-copy deserialization
- Async/await with Tokio for efficient concurrency
- Connection pooling and reuse

## Troubleshooting

### Common Issues

1. **"Address already in use"**: Another process is using port 4433
   - Change the port in configuration or stop the conflicting process

2. **Connection refused**: Server is not running
   - Start the server first before connecting clients

3. **Certificate errors**: TLS certificate issues
   - This is expected with self-signed certificates in development

### Debugging

Enable detailed logging:

```bash
RUST_LOG=garou=trace,quinn=debug cargo run -- server
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Submit a pull request

## License

[Add your license information here]

## Acknowledgments

- [Quinn](https://github.com/quinn-rs/quinn) for QUIC implementation
- [FlatBuffers](https://flatbuffers.dev/) for efficient serialization
- [Tokio](https://tokio.rs/) for async runtime