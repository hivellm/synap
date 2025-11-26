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
- ⚙️ **Configuration Editor** - YAML editor with live preview
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

Edit server configuration:
- YAML-based editor
- Live preview panel
- Configuration history with rollback

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

