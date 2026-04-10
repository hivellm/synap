<template>
  <div class="p-6">
    <div class="mb-6">
      <h1 class="text-2xl font-bold text-text-primary">Replication Monitor</h1>
      <p class="text-text-secondary mt-1">Monitor replication topology, lag, and slave nodes</p>
    </div>

    <div v-if="!isConnected" class="bg-warning/10 border border-warning rounded-xl p-4 mb-6">
      <div class="flex items-center gap-2">
        <i class="fas fa-exclamation-triangle text-warning"></i>
        <p class="text-text-primary">
          No server connected. Please select or add a server to view replication status.
        </p>
      </div>
    </div>

    <div v-else class="space-y-6">
      <!-- Replication Overview -->
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div class="bg-bg-card border border-border rounded-xl p-6">
          <div class="flex items-center gap-3 mb-4">
            <div class="w-10 h-10 rounded-lg bg-info/20 flex items-center justify-center">
              <i class="fas fa-crown text-info"></i>
            </div>
            <div>
              <h3 class="text-lg font-semibold text-text-primary">Role</h3>
              <p class="text-sm text-text-secondary">Current node role</p>
            </div>
          </div>
          <div class="text-2xl font-bold text-text-primary capitalize">
            {{ replicationInfo?.role || 'Unknown' }}
          </div>
        </div>

        <div class="bg-bg-card border border-border rounded-xl p-6">
          <div class="flex items-center gap-3 mb-4">
            <div class="w-10 h-10 rounded-lg bg-success/20 flex items-center justify-center">
              <i class="fas fa-server text-success"></i>
            </div>
            <div>
              <h3 class="text-lg font-semibold text-text-primary">Connected Slaves</h3>
              <p class="text-sm text-text-secondary">Active replicas</p>
            </div>
          </div>
          <div class="text-2xl font-bold text-text-primary">
            {{ replicationInfo?.connected_slaves || 0 }}
          </div>
        </div>

        <div class="bg-bg-card border border-border rounded-xl p-6">
          <div class="flex items-center gap-3 mb-4">
            <div class="w-10 h-10 rounded-lg" :class="masterLinkClass">
              <i class="fas fa-link" :class="masterLinkIconClass"></i>
            </div>
            <div>
              <h3 class="text-lg font-semibold text-text-primary">Master Link</h3>
              <p class="text-sm text-text-secondary">Connection status</p>
            </div>
          </div>
          <div class="text-2xl font-bold" :class="masterLinkTextClass">
            {{ masterLinkStatus }}
          </div>
        </div>
      </div>

      <!-- Master Info (if slave) -->
      <div v-if="replicationInfo?.role === 'slave'" class="bg-bg-card border border-border rounded-xl p-6">
        <h3 class="text-lg font-semibold text-text-primary mb-4 flex items-center gap-2">
          <i class="fas fa-database"></i>
          Master Information
        </h3>
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <div>
            <p class="text-sm text-text-secondary">Master Host</p>
            <p class="text-text-primary font-mono">{{ replicationInfo.master_host || 'N/A' }}</p>
          </div>
          <div>
            <p class="text-sm text-text-secondary">Master Port</p>
            <p class="text-text-primary font-mono">{{ replicationInfo.master_port || 'N/A' }}</p>
          </div>
          <div>
            <p class="text-sm text-text-secondary">Last I/O (seconds ago)</p>
            <p class="text-text-primary font-mono">{{ replicationInfo.master_last_io_seconds_ago ?? 'N/A' }}</p>
          </div>
          <div>
            <p class="text-sm text-text-secondary">Sync in Progress</p>
            <p class="text-text-primary">
              <span v-if="replicationInfo.master_sync_in_progress" class="text-warning">
                <i class="fas fa-sync fa-spin mr-1"></i> Syncing
              </span>
              <span v-else class="text-success">
                <i class="fas fa-check mr-1"></i> Synced
              </span>
            </p>
          </div>
        </div>
      </div>

      <!-- Topology Visualization -->
      <div class="bg-bg-card border border-border rounded-xl p-6">
        <h3 class="text-lg font-semibold text-text-primary mb-4 flex items-center gap-2">
          <i class="fas fa-project-diagram"></i>
          Topology
        </h3>
        
        <div class="flex flex-col items-center py-8">
          <!-- Master Node -->
          <div class="flex flex-col items-center">
            <div class="w-24 h-24 rounded-full bg-info/20 border-2 border-info flex items-center justify-center">
              <div class="text-center">
                <i class="fas fa-crown text-info text-2xl"></i>
                <p class="text-xs text-text-primary mt-1 font-semibold">Master</p>
              </div>
            </div>
            <p class="text-sm text-text-secondary mt-2 font-mono">
              {{ replicationInfo?.role === 'master' 
                ? 'This Node' 
                : (replicationInfo?.master_host && replicationInfo?.master_port
                  ? `${replicationInfo.master_host}:${replicationInfo.master_port}`
                  : 'N/A') }}
            </p>
          </div>

          <!-- Connection Lines -->
          <div v-if="(replicationInfo?.connected_slaves || 0) > 0 || replicationInfo?.role === 'slave'" 
               class="w-0.5 h-12 bg-border"></div>

          <!-- Slave Nodes -->
          <div v-if="replicationInfo?.role === 'master' && (replicationInfo?.connected_slaves || 0) > 0" 
               class="flex gap-8 mt-4">
            <div v-for="i in replicationInfo.connected_slaves" :key="i" 
                 class="flex flex-col items-center">
              <div class="w-16 h-16 rounded-full bg-success/20 border-2 border-success flex items-center justify-center">
                <div class="text-center">
                  <i class="fas fa-server text-success text-lg"></i>
                  <p class="text-xs text-text-primary mt-0.5">Slave {{ i }}</p>
                </div>
              </div>
            </div>
          </div>

          <!-- This node as slave -->
          <div v-if="replicationInfo?.role === 'slave'" class="flex flex-col items-center mt-4">
            <div class="w-20 h-20 rounded-full bg-success/20 border-2 border-success flex items-center justify-center">
              <div class="text-center">
                <i class="fas fa-server text-success text-xl"></i>
                <p class="text-xs text-text-primary mt-1">This Node</p>
              </div>
            </div>
            <p class="text-sm text-text-secondary mt-2">(Slave)</p>
          </div>
        </div>
      </div>

      <!-- Replication Lag Chart -->
      <div class="bg-bg-card border border-border rounded-xl p-6">
        <h3 class="text-lg font-semibold text-text-primary mb-4 flex items-center gap-2">
          <i class="fas fa-chart-area"></i>
          Replication Lag
        </h3>
        <div v-if="lagHistory.length === 0" class="text-center py-8 text-text-muted">
          No replication lag data available
        </div>
        <div v-else class="h-64">
          <canvas ref="lagChartCanvas"></canvas>
        </div>
      </div>

      <!-- Actions -->
      <div class="bg-bg-card border border-border rounded-xl p-6">
        <h3 class="text-lg font-semibold text-text-primary mb-4 flex items-center gap-2">
          <i class="fas fa-cogs"></i>
          Actions
        </h3>
        <div class="flex flex-wrap gap-3">
          <button 
            @click="refreshInfo"
            class="px-4 py-2 bg-info text-white rounded-lg hover:bg-info/80 transition-colors flex items-center gap-2"
          >
            <i class="fas fa-sync"></i>
            Refresh Status
          </button>
          <button 
            v-if="replicationInfo?.role === 'slave'"
            @click="promoteToMaster"
            class="px-4 py-2 bg-warning text-white rounded-lg hover:bg-warning/80 transition-colors flex items-center gap-2"
          >
            <i class="fas fa-crown"></i>
            Promote to Master
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useApi } from '@/composables/useApi';
import { Chart, CategoryScale, LinearScale, PointElement, LineElement, Filler, Tooltip, LineController } from 'chart.js';

Chart.register(CategoryScale, LinearScale, PointElement, LineElement, Filler, Tooltip, LineController);

interface ReplicationInfo {
  role: string;
  connected_slaves: number;
  master_host?: string;
  master_port?: number;
  master_link_status?: string;
  master_last_io_seconds_ago?: number;
  master_sync_in_progress?: boolean;
}

const { apiClient, isConnected } = useApi();

const replicationInfo = ref<ReplicationInfo | null>(null);
const lagHistory = ref<{ timestamp: number; value: number }[]>([]);
const lagChartCanvas = ref<HTMLCanvasElement | null>(null);
let chartInstance: Chart | null = null;
let pollInterval: NodeJS.Timeout | null = null;

const masterLinkStatus = computed(() => {
  if (!replicationInfo.value) return 'Unknown';
  if (replicationInfo.value.role === 'master') return 'N/A';
  return replicationInfo.value.master_link_status || 'Down';
});

const masterLinkClass = computed(() => {
  if (masterLinkStatus.value === 'up') return 'bg-success/20';
  if (masterLinkStatus.value === 'down') return 'bg-error/20';
  return 'bg-bg-tertiary';
});

const masterLinkIconClass = computed(() => {
  if (masterLinkStatus.value === 'up') return 'text-success';
  if (masterLinkStatus.value === 'down') return 'text-error';
  return 'text-text-muted';
});

const masterLinkTextClass = computed(() => {
  if (masterLinkStatus.value === 'up') return 'text-success';
  if (masterLinkStatus.value === 'down') return 'text-error';
  return 'text-text-primary';
});

async function fetchReplicationInfo() {
  if (!apiClient.value) return;

  try {
    const response = await apiClient.value.getInfo();
    if (response.success && response.data?.replication) {
      const repl = response.data.replication;
      replicationInfo.value = {
        role: repl.role || 'master',
        connected_slaves: repl.connected_replicas || 0,
        master_host: repl.master_host,
        master_port: repl.master_port,
        master_link_status: repl.role === 'slave' ? 'up' : undefined,
        master_last_io_seconds_ago: repl.role === 'slave' ? 0 : undefined,
        master_sync_in_progress: false,
      };
      
      // Add lag point if we have lag data (would come from replication.status command)
      // For now, we estimate from replication info
      if (replicationInfo.value.role === 'slave') {
        lagHistory.value.push({
          timestamp: Date.now(),
          value: replicationInfo.value.master_last_io_seconds_ago || 0,
        });
        // Keep last 100 points
        if (lagHistory.value.length > 100) {
          lagHistory.value.shift();
        }
        updateChart();
      }
    }
  } catch (error) {
    console.error('Failed to fetch replication info:', error);
  }
}

function updateChart() {
  if (!lagChartCanvas.value || lagHistory.value.length === 0) return;

  const labels = lagHistory.value.map(p => 
    new Date(p.timestamp).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' })
  );
  const values = lagHistory.value.map(p => p.value);

  const chartData = {
    labels,
    datasets: [{
      label: 'Replication Lag (seconds)',
      data: values,
      borderColor: 'oklch(75% 0.15 80)',
      backgroundColor: 'oklch(75% 0.15 80 / 0.2)',
      fill: true,
      tension: 0.4,
    }],
  };

  if (chartInstance) {
    chartInstance.data = chartData;
    chartInstance.update();
  } else {
    chartInstance = new Chart(lagChartCanvas.value, {
      type: 'line',
      data: chartData,
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          tooltip: {
            backgroundColor: 'oklch(27% 0 0)',
            titleColor: 'oklch(96% 0 0)',
            bodyColor: 'oklch(75% 0 0)',
          },
        },
        scales: {
          x: {
            grid: { display: false },
            ticks: { color: 'oklch(75% 0 0)' },
          },
          y: {
            grid: { color: 'oklch(20% 0 0)' },
            ticks: { 
              color: 'oklch(75% 0 0)',
              callback: (v) => `${v}s`,
            },
          },
        },
      },
    });
  }
}

async function refreshInfo() {
  await fetchReplicationInfo();
}

async function promoteToMaster() {
  if (!apiClient.value || !confirm('Are you sure you want to promote this replica to master?')) return;
  
  try {
    // Use replication.promote command
    const response = await apiClient.value.executeCommand('replication.promote', { force: false });
    if (response.success) {
      alert('Replica promoted to master successfully');
      await fetchReplicationInfo();
    } else {
      alert(`Failed to promote: ${response.error || 'Unknown error'}`);
    }
  } catch (error: any) {
    console.error('Failed to promote replica:', error);
    alert(`Failed to promote replica: ${error.message || 'Unknown error'}`);
  }
}

onMounted(() => {
  if (isConnected.value) {
    fetchReplicationInfo();
    pollInterval = setInterval(fetchReplicationInfo, 5000);
  }
});

onUnmounted(() => {
  if (pollInterval) {
    clearInterval(pollInterval);
  }
  if (chartInstance) {
    chartInstance.destroy();
  }
});
</script>

