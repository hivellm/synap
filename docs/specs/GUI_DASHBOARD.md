# Synap GUI Dashboard

## Overview

Synap Desktop is an Electron-based graphical user interface for monitoring, managing, and configuring Synap server instances. It provides real-time visualization of server metrics, performance statistics, and operational insights.

## Features

### Core Features
- 📊 **Real-time Monitoring**: Live metrics and performance graphs
- 🖥️ **Multi-instance Management**: Monitor multiple Synap servers
- ⚙️ **Configuration Editor**: Visual configuration management
- 📈 **Performance Analytics**: CPU, memory, throughput, latency tracking
- 🔍 **Log Viewer**: Real-time log streaming and filtering
- 🎯 **Health Dashboard**: Server health and status indicators
- 📦 **Cache Inspector**: L1/L2 cache visualization
- 🔄 **Replication Monitor**: Master-slave replication status

### Advanced Features
- 🔔 **Alerting System**: Custom alerts and notifications
- 📊 **Historical Data**: Time-series data visualization
- 🗺️ **Topology View**: Cluster topology visualization
- 🔐 **Access Control**: User authentication and permissions
- 🌐 **Remote Management**: Connect to remote Synap instances
- 📝 **Audit Logs**: Track all configuration changes

---

## Architecture

### Technology Stack

```
┌─────────────────────────────────────────────────────┐
│              Synap Desktop (Electron)                │
├─────────────────────────────────────────────────────┤
│  Frontend Layer                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────┐  │
│  │   Vue.js 3   │  │  Chart.js    │  │ TailwindCSS│ │
│  │  (Composition│  │  (Graphs)    │  │  (Styling) │ │
│  │     API)     │  │              │  │            │ │
│  └──────────────┘  └──────────────┘  └───────────┘  │
├─────────────────────────────────────────────────────┤
│  Electron Main Process                               │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────┐  │
│  │   IPC Bridge │  │  WebSocket   │  │   Auto    │  │
│  │              │  │   Manager    │  │  Updater  │  │
│  └──────────────┘  └──────────────┘  └───────────┘  │
├─────────────────────────────────────────────────────┤
│  Communication Layer                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────┐  │
│  │ REST API     │  │  WebSocket   │  │    MCP    │  │
│  │   Client     │  │   Client     │  │  Client   │  │
│  └──────────────┘  └──────────────┘  └───────────┘  │
└─────────────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│              Synap Server Instance(s)                │
│  REST API | WebSocket | Metrics | Logs              │
└─────────────────────────────────────────────────────┘
```

### Components

#### 1. Main Window
- Server selector and connection status
- Navigation menu (Dashboard, Metrics, Logs, Config)
- Quick stats overview
- Notification center

#### 2. Dashboard View
- Real-time server statistics
- Active connections count
- Operations per second
- Memory usage
- Cache hit rates
- Top keys/queues

#### 3. Metrics View
- Interactive graphs and charts
- Customizable time ranges
- Exportable data (CSV, JSON)
- Comparison mode (before/after)

#### 4. Logs Viewer
- Real-time log streaming
- Log level filtering
- Search and filter
- Export logs

#### 5. Configuration Editor
- Visual YAML editor
- Validation and syntax highlighting
- Apply changes with preview
- Rollback support

#### 6. Replication Monitor
- Master-slave topology
- Replication lag visualization
- Sync status
- Failover controls

---

## User Interface Design

### Dashboard Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│  Synap Desktop                                    🟢 Connected   [≡]  │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  ┌────────────┐  ┌────────────────────────────────────────────────┐ │
│  │            │  │  Server: localhost:15500                       │ │
│  │  Servers   │  │  Status: ● Running   Uptime: 5d 3h 42m         │ │
│  │            │  │                                                 │ │
│  │  ● Master  │  │  ┌───────────┐  ┌───────────┐  ┌───────────┐  │ │
│  │  localhost │  │  │ Operations│  │  Memory   │  │   Cache   │  │ │
│  │  :15500    │  │  │ 125K/sec  │  │ 4.2/8 GB  │  │  Hit 82%  │  │ │
│  │            │  │  └───────────┘  └───────────┘  └───────────┘  │ │
│  │  ○ Replica │  │                                                 │ │
│  │  localhost │  │  ┌─────────────────────────────────────────┐  │ │
│  │  :15502    │  │  │       Operations Per Second             │  │ │
│  │            │  │  │  150K ┤                                  │  │ │
│  │  ○ Replica │  │  │       │     ╱\                           │  │ │
│  │  localhost │  │  │  100K ┤    ╱  \    ╱\                    │  │ │
│  │  :15503    │  │  │       │   ╱    \  ╱  \  ╱                │  │ │
│  │            │  │  │   50K ┤  ╱      \/    \/                 │  │ │
│  │  + Add     │  │  │       └──────────────────────────────────│  │ │
│  │            │  │  │         12:00  12:30  13:00  13:30       │  │ │
│  └────────────┘  │  └─────────────────────────────────────────┘  │ │
│                  │                                                 │ │
│                  │  ┌──────────────────────────────────────────┐  │ │
│                  │  │  Key-Value Store                         │  │ │
│                  │  │  Keys: 1,234,567  |  Size: 2.3 GB        │  │ │
│                  │  │  L1 Cache: 82% hit  |  L2: 91% hit       │  │ │
│                  │  │                                           │  │ │
│                  │  │  Queue System                            │  │ │
│                  │  │  Active: 15  |  Messages: 45,231         │  │ │
│                  │  │  Pending: 123  |  Processing: 45/sec     │  │ │
│                  │  │                                           │  │ │
│                  │  │  Event Streams                           │  │ │
│                  │  │  Rooms: 234  |  Subscribers: 1,823       │  │ │
│                  │  │  Events/sec: 89  |  Total: 5.2M          │  │ │
│                  │  └──────────────────────────────────────────┘  │ │
│                  │                                                 │ │
│                  └─────────────────────────────────────────────────┘ │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

### Metrics View

```
┌──────────────────────────────────────────────────────────────────────┐
│  Metrics  [Last Hour ▾]  [Real-time ✓]                    [Export]   │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  Performance Metrics                                                  │
│  ┌────────────────────────────────┐  ┌──────────────────────────┐   │
│  │  CPU Usage                     │  │  Memory Usage            │   │
│  │  ┌─────────────────────────┐   │  │  ┌───────────────────┐  │   │
│  │  │ Current: 23.4%          │   │  │  │ 4.2 / 8.0 GB      │  │   │
│  │  │ Average: 18.2%          │   │  │  │ (52%)             │  │   │
│  │  │ Peak: 45.1%             │   │  │  │                   │  │   │
│  │  └─────────────────────────┘   │  │  └───────────────────┘  │   │
│  └────────────────────────────────┘  └──────────────────────────┘   │
│                                                                       │
│  Throughput                                                           │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  Operations/sec: 125,432                                       │  │
│  │  200K ┤                                                         │  │
│  │       │      ╱╲                                                 │  │
│  │  150K ┤     ╱  ╲      ╱╲                                        │  │
│  │       │    ╱    ╲    ╱  ╲    ╱                                  │  │
│  │  100K ┤   ╱      ╲  ╱    ╲  ╱                                   │  │
│  │       │  ╱        ╲╱      ╲╱                                    │  │
│  │   50K ┤─────────────────────────────────────────────            │  │
│  │       └─────────────────────────────────────────────            │  │
│  │       12:00    12:15    12:30    12:45    13:00                │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                       │
│  Latency Distribution (ms)                                            │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  p50: 0.23ms  │  p95: 0.89ms  │  p99: 1.45ms  │  Max: 3.21ms  │  │
│  │  ████████████████████████████████████▓▓▓▒▒░░                   │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

### Configuration Editor

```
┌──────────────────────────────────────────────────────────────────────┐
│  Configuration: /etc/synap/config.yml           [Save] [Validate]    │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  ┌────────────┐  ┌────────────────────────────────────────────────┐ │
│  │ Sections   │  │  server:                                       │ │
│  │            │  │    host: "0.0.0.0"                             │ │
│  │ ▶ Server   │  │    port: 15500                                 │ │
│  │ ▼ Protocols│  │                                                 │ │
│  │   ▶ HTTP   │  │  protocols:                                    │ │
│  │   ▼ MCP    │  │    mcp:                                        │ │
│  │   ▶ UMICP  │  │      enabled: true                             │ │
│  │ ▶ Memory   │  │      port: 15501                               │ │
│  │ ▶ Cache    │  │      features: ["resources", "tools"]          │ │
│  │ ▶ Compress │  │                                                 │ │
│  │ ▶ Replica  │  │    streamable_http:                            │ │
│  │            │  │      enabled: true                             │ │
│  └────────────┘  │      path: /api                                │ │
│                  │                                                 │ │
│                  │  cache:                                         │ │
│                  │    l1_hot_data:                                 │ │
│                  │      enabled: true                              │ │
│                  │      max_size_mb: 512                           │ │
│                  │      ttl_seconds: 5                             │ │
│                  │                                                 │ │
│                  │  # Validation: ✓ Valid YAML                    │ │
│                  │  # Warnings: 0  |  Errors: 0                   │ │
│                  └─────────────────────────────────────────────────┘ │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Plan

### Phase 1: Foundation (MVP)
**Timeline**: 4 weeks

**Deliverables**:
- [x] Project setup (Electron + Vue.js 3)
- [x] Main window and navigation
- [x] Server connection management
- [x] Basic dashboard with real-time metrics
- [x] REST API client integration
- [ ] Auto-updater configuration

**Tech Stack**:
```json
{
  "dependencies": {
    "electron": "^28.0.0",
    "vue": "^3.4.0",
    "chart.js": "^4.4.0",
    "axios": "^1.6.0",
    "ws": "^8.16.0",
    "yaml": "^2.3.0",
    "tailwindcss": "^3.4.0"
  }
}
```

### Phase 2: Core Features
**Timeline**: 6 weeks

**Deliverables**:
- [ ] Advanced metrics visualization
- [ ] Log viewer with filtering
- [ ] Configuration editor
- [ ] Multi-instance management
- [ ] WebSocket real-time updates
- [ ] Export functionality

### Phase 3: Advanced Features
**Timeline**: 4 weeks

**Deliverables**:
- [ ] Replication topology view
- [ ] Cache inspector
- [ ] Alert system
- [ ] Historical data storage
- [ ] Performance analytics
- [ ] Custom dashboards

### Phase 4: Polish & Distribution
**Timeline**: 2 weeks

**Deliverables**:
- [ ] UI/UX improvements
- [ ] Dark/Light theme
- [ ] Localization (i18n)
- [ ] Installers (Windows, macOS, Linux)
- [ ] Documentation
- [ ] User guide

---

## Project Structure

```
synap-desktop/
├── package.json
├── electron-builder.json
├── tsconfig.json
├── vite.config.ts
│
├── electron/
│   ├── main.ts                 # Electron main process
│   ├── preload.ts              # Preload script
│   └── ipc/
│       ├── server.ts           # Server connection IPC
│       └── updater.ts          # Auto-updater IPC
│
├── src/
│   ├── main.ts                 # Vue app entry
│   ├── App.vue                 # Root component
│   │
│   ├── views/
│   │   ├── Dashboard.vue       # Main dashboard
│   │   ├── Metrics.vue         # Metrics view
│   │   ├── Logs.vue            # Log viewer
│   │   ├── Config.vue          # Configuration editor
│   │   ├── Replication.vue     # Replication monitor
│   │   └── Settings.vue        # App settings
│   │
│   ├── components/
│   │   ├── ServerList.vue      # Server list sidebar
│   │   ├── StatsCard.vue       # Stats card component
│   │   ├── LineChart.vue       # Line chart component
│   │   ├── BarChart.vue        # Bar chart component
│   │   ├── LogViewer.vue       # Log viewer component
│   │   └── YamlEditor.vue      # YAML editor component
│   │
│   ├── stores/
│   │   ├── servers.ts          # Servers state (Pinia)
│   │   ├── metrics.ts          # Metrics state
│   │   └── logs.ts             # Logs state
│   │
│   ├── api/
│   │   ├── rest-client.ts      # REST API client
│   │   ├── ws-client.ts        # WebSocket client
│   │   └── mcp-client.ts       # MCP client
│   │
│   ├── types/
│   │   ├── server.ts           # Server types
│   │   ├── metrics.ts          # Metrics types
│   │   └── config.ts           # Config types
│   │
│   └── utils/
│       ├── formatters.ts       # Data formatters
│       ├── validators.ts       # Config validators
│       └── exporters.ts        # Data exporters
│
├── public/
│   ├── icon.png
│   └── index.html
│
└── build/
    ├── icon.icns               # macOS icon
    ├── icon.ico                # Windows icon
    └── icon.png                # Linux icon
```

---

## Key Features Implementation

### 1. Real-time Metrics

```typescript
// src/stores/metrics.ts
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { wsClient } from '@/api/ws-client'

export const useMetricsStore = defineStore('metrics', () => {
  const metrics = ref<Metrics[]>([])
  const isConnected = ref(false)
  
  // Subscribe to WebSocket metrics stream
  const subscribe = (serverId: string) => {
    wsClient.subscribe(`/metrics/${serverId}`, (data) => {
      metrics.value.push({
        timestamp: Date.now(),
        ...data
      })
      
      // Keep last 1000 data points
      if (metrics.value.length > 1000) {
        metrics.value.shift()
      }
    })
    isConnected.value = true
  }
  
  // Computed statistics
  const opsPerSecond = computed(() => {
    if (metrics.value.length < 2) return 0
    const latest = metrics.value[metrics.value.length - 1]
    return latest.operations_per_second || 0
  })
  
  const avgLatency = computed(() => {
    if (metrics.value.length === 0) return 0
    const sum = metrics.value.reduce((acc, m) => acc + m.latency_p50, 0)
    return sum / metrics.value.length
  })
  
  return {
    metrics,
    isConnected,
    subscribe,
    opsPerSecond,
    avgLatency
  }
})
```

### 2. Server Connection Management

```typescript
// src/api/rest-client.ts
import axios, { AxiosInstance } from 'axios'

export class SynapClient {
  private client: AxiosInstance
  
  constructor(baseURL: string, apiKey?: string) {
    this.client = axios.create({
      baseURL,
      headers: apiKey ? { 'Authorization': `Bearer ${apiKey}` } : {}
    })
  }
  
  async getHealth(): Promise<HealthStatus> {
    const { data } = await this.client.get('/health')
    return data
  }
  
  async getMetrics(): Promise<Metrics> {
    const { data } = await this.client.get('/metrics')
    return data
  }
  
  async getConfig(): Promise<Config> {
    const { data } = await this.client.get('/admin/config')
    return data
  }
  
  async updateConfig(config: Config): Promise<void> {
    await this.client.put('/admin/config', config)
  }
  
  async getLogs(params: LogParams): Promise<LogEntry[]> {
    const { data } = await this.client.get('/admin/logs', { params })
    return data
  }
}
```

### 3. Configuration Editor

```vue
<!-- src/components/YamlEditor.vue -->
<template>
  <div class="yaml-editor">
    <div class="editor-toolbar">
      <button @click="validate" :disabled="!isDirty">
        Validate
      </button>
      <button @click="save" :disabled="!isDirty || hasErrors">
        Save
      </button>
      <button @click="reset">
        Reset
      </button>
    </div>
    
    <div class="editor-container">
      <textarea
        v-model="content"
        @input="handleInput"
        spellcheck="false"
        class="yaml-textarea"
      />
    </div>
    
    <div v-if="validation" class="validation-status">
      <div v-if="validation.valid" class="success">
        ✓ Valid YAML
      </div>
      <div v-else class="error">
        ✗ {{ validation.error }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import YAML from 'yaml'

const props = defineProps<{
  modelValue: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'save', value: object): void
}>()

const content = ref(props.modelValue)
const validation = ref<{ valid: boolean, error?: string } | null>(null)
const originalContent = ref(props.modelValue)

const isDirty = computed(() => content.value !== originalContent.value)
const hasErrors = computed(() => validation.value && !validation.value.valid)

const validate = () => {
  try {
    YAML.parse(content.value)
    validation.value = { valid: true }
  } catch (error: any) {
    validation.value = {
      valid: false,
      error: error.message
    }
  }
}

const save = () => {
  validate()
  if (validation.value?.valid) {
    const config = YAML.parse(content.value)
    emit('save', config)
    originalContent.value = content.value
  }
}

const reset = () => {
  content.value = originalContent.value
  validation.value = null
}

const handleInput = () => {
  emit('update:modelValue', content.value)
  validation.value = null
}
</script>
```

---

## Distribution

### Build Configuration

**`electron-builder.json`**:
```json
{
  "appId": "io.synap.desktop",
  "productName": "Synap Desktop",
  "copyright": "Copyright © 2025 HiveLLM",
  "directories": {
    "output": "dist"
  },
  "files": [
    "dist-electron/**/*",
    "dist/**/*",
    "package.json"
  ],
  "mac": {
    "target": ["dmg", "zip"],
    "category": "public.app-category.developer-tools",
    "icon": "build/icon.icns"
  },
  "win": {
    "target": ["nsis", "portable"],
    "icon": "build/icon.ico"
  },
  "linux": {
    "target": ["AppImage", "deb"],
    "category": "Development",
    "icon": "build/icon.png"
  },
  "nsis": {
    "oneClick": false,
    "allowToChangeInstallationDirectory": true
  }
}
```

### Build Commands

```bash
# Development
npm run dev

# Build for current platform
npm run build

# Build for all platforms
npm run build:all

# Build for specific platform
npm run build:mac
npm run build:win
npm run build:linux
```

---

## Installation

### From Releases

**Windows**:
```
synap-desktop-setup-0.1.0.exe
```

**macOS**:
```
synap-desktop-0.1.0.dmg
```

**Linux**:
```bash
# AppImage
chmod +x synap-desktop-0.1.0.AppImage
./synap-desktop-0.1.0.AppImage

# DEB
sudo dpkg -i synap-desktop_0.1.0_amd64.deb
```

---

## Screenshots (Planned)

1. **Dashboard** - Overview with real-time metrics
2. **Metrics** - Detailed performance graphs
3. **Logs** - Log viewer with filtering
4. **Config** - Visual configuration editor
5. **Replication** - Topology and status
6. **Alerts** - Alert configuration and history

---

## Roadmap

### v0.1.0 (MVP) - Q1 2025
- Basic dashboard
- Server connection
- Real-time metrics
- Log viewer

### v0.2.0 - Q2 2025
- Configuration editor
- Multi-instance support
- Export functionality
- Dark theme

### v0.3.0 - Q3 2025
- Replication monitor
- Cache inspector
- Alert system
- Historical data

### v1.0.0 - Q4 2025
- Performance analytics
- Custom dashboards
- Advanced alerting
- Full localization

---

## See Also

- [Synap Server Documentation](../README.md)
- [REST API Reference](api/REST_API.md)
- [WebSocket Protocol](protocol/STREAMABLE_HTTP.md)
- [Deployment Guide](DEPLOYMENT.md)
- [Vectorizer Dashboard](../../vectorizer/dashboard/) (Reference implementation)

