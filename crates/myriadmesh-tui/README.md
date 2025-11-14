# MyriadMesh TUI - Terminal User Interface

A powerful terminal-based user interface for managing MyriadMesh nodes.

## Features

- **Dashboard**: Real-time node status, adapter metrics, and network overview
- **Message Management**: Send, receive, and track message delivery
- **Configuration Editor**: Edit node configuration in-place
- **Log Viewer**: Real-time log streaming with filtering and search
- **Keyboard Navigation**: Full keyboard control for remote server management

## Usage

```bash
# Start TUI connecting to local node
myriadmesh-tui

# Connect to remote node
myriadmesh-tui --api-url http://remote-node:4000

# Specify custom config
myriadmesh-tui --config /path/to/config.yaml
```

## Keyboard Shortcuts

### Global
- `Tab` / `Shift+Tab` - Navigate between views
- `q` / `Ctrl+C` - Quit
- `?` - Show help

### Dashboard
- `r` - Refresh data
- `a` - View adapters
- `n` - View nodes

### Messages
- `s` - Send message
- `d` - Delete message
- `/` - Search messages

### Configuration
- `e` - Edit mode
- `s` - Save changes
- `Esc` - Cancel edit

### Logs
- `f` - Toggle follow mode
- `/` - Search logs
- `c` - Clear logs
- `e` - Export logs

## Architecture

```
myriadmesh-tui/
├── src/
│   ├── main.rs          # Entry point and CLI
│   ├── app.rs           # Application state machine
│   ├── api_client.rs    # MyriadNode API client
│   ├── events.rs        # Event handling (keyboard, timers)
│   └── ui/
│       ├── mod.rs       # UI coordinator
│       ├── dashboard.rs # Dashboard view
│       ├── messages.rs  # Message management view
│       ├── config.rs    # Configuration editor view
│       └── logs.rs      # Log viewer
```

## API Integration

The TUI connects to the MyriadNode REST API:

- `GET /health` - Node health check
- `GET /api/v1/node/info` - Node information
- `GET /api/v1/node/status` - Node status
- `GET /api/v1/adapters` - Adapter list
- `GET /api/v1/messages/list` - Message list
- `POST /api/v1/messages/send` - Send message
- `GET /api/v1/dht/nodes` - DHT nodes
- `GET /api/v1/logs` - Log stream

## Development

```bash
# Run in development mode
cargo run --package myriadmesh-tui

# Build release
cargo build --release --package myriadmesh-tui

# Run tests
cargo test --package myriadmesh-tui
```

## Requirements

- Rust 1.70+
- MyriadNode running with API enabled
- Terminal with UTF-8 and 256-color support

## License

MIT OR Apache-2.0
