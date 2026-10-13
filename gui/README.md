# Synap Desktop

GUI Dashboard for Synap Server - Built with Electron, Vue.js 3, and TypeScript.

![Synap Desktop](https://img.shields.io/badge/version-0.1.0-blue)
![License](https://img.shields.io/badge/license-Apache--2.0-green)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)

## Features

- 🖥️ **Cross-platform** - Windows, macOS, and Linux support
- 📊 **Real-time Dashboard** - Live metrics with auto-refresh
- 🔌 **Multi-server Management** - Connect to multiple Synap instances
- 📈 **Interactive Charts** - Performance graphs with Chart.js
- 🗄️ **Data Inspectors** - Browse KV Store, Hash, List, Set, Sorted Set
- 📬 **Queue Monitor** - View message queues and DLQ
- 📡 **Stream Viewer** - Monitor event streams and partitions
- 📢 **Pub/Sub Dashboard** - Topic management and subscriptions
- 🔄 **Replication Monitor** - Topology visualization and lag tracking
- ⚙️ **Configuration Viewer** - The server's effective configuration, secrets redacted, with copy and download as YAML
- 📝 **Log Viewer** - Real-time log streaming with filters
- 🎨 **Dark Theme** - Modern dark UI inspired by professional tools

## Quick Start

### Prerequisites

- Node.js 18+
- npm or yarn

### Development

```bash
# Navigate to gui directory
cd gui

# Install dependencies
npm install

# Start development server with Electron
npm run electron:dev
```

### Building

```bash
# Build for current platform
npm run build:electron

# Build for specific platforms
npm run build:win      # Windows (NSIS installer + portable)
npm run build:mac      # macOS (DMG + ZIP)
npm run build:linux    # Linux (AppImage + DEB)
```

Built applications are output to the `release/` directory.

## User Guide

### Connecting to a Server

1. Click on "Select Server" in the sidebar
2. Click "Manage Servers" or the "+" button
3. Enter server details:
   - **Name**: Display name for the server
   - **URL**: Server address (e.g., `http://localhost`)
   - **Port**: Server port (default: 15500)
   - **API Key**: Optional authentication key
4. Click "Test" to verify connection
5. Click "Add" to save

### Dashboard

The dashboard provides an overview of your Synap server:
- **Operations/sec**: Current throughput
- **Memory Usage**: RAM consumption
- **Cache Hit Rate**: Cache efficiency
- **Active Connections**: Current client connections
- **Performance Graphs**: Historical metrics visualization

### KV Store Inspector

Browse and manage key-value pairs:
- Search keys by pattern
- View key details (type, TTL, value)
- Edit or delete keys
- Monitor memory usage per key

### Data Structures

Inspect Redis-compatible data structures:
- **Hash**: View and edit hash fields
- **List**: Browse list elements with push/pop
- **Set**: Manage set members
- **Sorted Set**: View ranked members with scores

Inspecting a key shows its type (`key.type`) and memory usage
(`memory.usage`) before its contents. A key that does not exist, or that holds
a different structure than the selected tab, is reported as such. Every read
and write goes through the server's `/api/v1/command` envelope, so key names
with `/`, spaces, `?`, `#`, `%` or non-ASCII characters work unchanged.

### Queues

Monitor message queues:
- View queue sizes and message counts
- Inspect message contents
- Monitor dead letter queues (DLQ)

### Streams

Event streaming management:
- View rooms and partitions
- Monitor consumer groups
- Track message offsets

### Pub/Sub

Publish/Subscribe system:
- List active topics
- View subscriber counts
- Publish test messages

### Replication

Monitor replication topology:
- View master/slave roles
- Track replication lag
- Visualize cluster topology

### Configuration

Shows the configuration the server is running with: the config file plus every
environment and command-line override. It comes from the admin-only
`admin.config` command, so the connection's API key needs admin permission
(`admin:*`). Passwords, API keys, the Hub service key and any other secret
value appear as `[redacted]`.

- Read-only YAML of the effective configuration, plus a per-section view
- **Copy YAML** and **Download YAML**
- A failed or denied load shows the server's error and no configuration

The view cannot change settings: most of them only take effect at startup, and
a remote write would be a privileged attack surface. Edit the server's config
file and restart it instead.

### Logs

Real-time log streaming:
- Filter by level (DEBUG, INFO, WARN, ERROR)
- Search logs
- Export to JSON

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+R` / `Cmd+R` | Refresh current view |
| `Ctrl+,` / `Cmd+,` | Open settings |
| `Ctrl+L` / `Cmd+L` | Focus log search |
| `Escape` | Close modal/dialog |

## Project Structure

```
gui/
├── build/             # Build resources (icons)
├── electron/          # Electron main process
│   ├── main.ts       # Main process entry
│   └── preload.ts    # Preload script (IPC)
├── src/
│   ├── components/   # Reusable Vue components
│   ├── composables/  # Vue composables
│   ├── router/       # Vue Router configuration
│   ├── services/     # API and WebSocket clients
│   ├── stores/       # Pinia state stores
│   ├── types/        # TypeScript definitions
│   └── views/        # Page components
├── dist/             # Built renderer (generated)
├── dist-electron/    # Built main process (generated)
└── release/          # Packaged apps (generated)
```

## Testing

```bash
# Build the server the tests run against (from the repository root)
cargo build -p synap-server --bin synap-server

# From gui/
npm test
```

`npm test` runs Vitest. `tests/contract/` checks the API client
(`src/services/api.ts`) against a real server: the configuration command,
key type, memory usage, missing keys, permission errors and encoded key names.
`tests/smoke/` creates a hash, list, set and sorted set through the server and
inspects each one in the mounted Data Structures view. Each test file starts
its own `synap-server` on a free port with authentication required and a
temporary data directory (`tests/support/server.ts`). The binary is
`SYNAP_SERVER_BIN` when set, otherwise the newest `target/debug` or
`target/release` build.

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `VITE_DEV_SERVER_URL` | Dev server URL | `http://localhost:17001` |

### Build Configuration

Build settings are in `package.json` under the `build` key:
- `appId`: Application identifier
- `productName`: Display name
- `win/mac/linux`: Platform-specific settings
- `publish`: Auto-update configuration

## Troubleshooting

### App won't start
- Ensure Node.js 18+ is installed
- Run `npm install` to install dependencies
- Check for port conflicts on 17001

### Connection failed
- Verify Synap server is running
- Check firewall settings
- Confirm URL and port are correct

### Build errors
- Clear `node_modules` and reinstall
- Ensure all dev dependencies are installed
- Check TypeScript errors with `npm run lint`

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run linting: `npm run lint`
5. Submit a pull request

## License

Apache-2.0 - See [LICENSE](../LICENSE) for details.

