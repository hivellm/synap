<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <h3 class="text-lg font-semibold text-text-primary">{{ title }}</h3>
      <div class="flex items-center space-x-2">
        <select
          v-model="selectedTimeRange"
          @change="updateTimeRange"
          class="text-sm border border-border rounded-lg px-2 py-1 bg-bg-tertiary text-text-primary"
        >
          <option value="1h">Last Hour</option>
          <option value="6h">Last 6 Hours</option>
          <option value="24h">Last 24 Hours</option>
          <option value="7d">Last 7 Days</option>
        </select>
      </div>
    </div>
    <div class="h-64">
      <canvas ref="chartCanvas"></canvas>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import {
  Chart,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  LineController,
  BarController,
  Title,
  Tooltip,
  Legend,
  Filler,
} from 'chart.js';
import type { ChartConfiguration, ChartData } from 'chart.js';

interface Props {
  title: string;
  data: Array<{ timestamp: number; value: number }>;
  type?: 'line' | 'bar';
  color?: string;
  unit?: string;
}

const props = withDefaults(defineProps<Props>(), {
  type: 'line',
  color: '#3b82f6',
  unit: '',
});

const chartCanvas = ref<HTMLCanvasElement | null>(null);
const selectedTimeRange = ref('1h');
let chartInstance: Chart | null = null;

// Register Chart.js components
Chart.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  LineController,
  BarController,
  Title,
  Tooltip,
  Legend,
  Filler
);

function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
}

function getFilteredData() {
  const now = Date.now();
  const ranges: Record<string, number> = {
    '1h': 60 * 60 * 1000,
    '6h': 6 * 60 * 60 * 1000,
    '24h': 24 * 60 * 60 * 1000,
    '7d': 7 * 24 * 60 * 60 * 1000,
  };

  const range = ranges[selectedTimeRange.value] || ranges['1h'];
  const cutoff = now - range;

  return props.data.filter((point) => point.timestamp >= cutoff);
}

function updateChart() {
  if (!chartCanvas.value) return;

  const filteredData = getFilteredData();
  const labels = filteredData.map((point) => formatTimestamp(point.timestamp));
  const values = filteredData.map((point) => point.value);

  const chartData: ChartData = {
    labels,
    datasets: [
      {
        label: props.title,
        data: values,
        borderColor: props.color,
        backgroundColor: props.type === 'line' 
          ? `${props.color}20` 
          : props.color,
        fill: props.type === 'line',
        tension: 0.4,
        pointRadius: 2,
        pointHoverRadius: 4,
      },
    ],
  };

  const config: ChartConfiguration = {
    type: props.type,
    data: chartData,
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: {
          display: false,
        },
        tooltip: {
          mode: 'index',
          intersect: false,
          callbacks: {
            label: (context) => {
              const value = typeof context.parsed.y === 'number' 
                ? context.parsed.y.toFixed(2) 
                : context.parsed.y;
              return `${value}${props.unit}`;
            },
          },
        },
      },
      scales: {
        x: {
          display: true,
          grid: {
            display: true,
            color: 'rgba(255, 255, 255, 0.05)',
          },
          ticks: {
            maxTicksLimit: 10,
            color: 'rgba(255, 255, 255, 0.6)',
          },
        },
        y: {
          display: true,
          grid: {
            color: 'rgba(255, 255, 255, 0.05)',
          },
          ticks: {
            callback: (value) => {
              const numValue = typeof value === 'number' ? value : parseFloat(String(value));
              const formatted = isNaN(numValue) ? value : numValue.toFixed(2);
              return `${formatted}${props.unit}`;
            },
            color: 'rgba(255, 255, 255, 0.6)',
          },
        },
      },
      interaction: {
        mode: 'nearest',
        axis: 'x',
        intersect: false,
      },
    },
  };

  if (chartInstance) {
    chartInstance.data = chartData;
    chartInstance.update();
  } else {
    chartInstance = new Chart(chartCanvas.value, config);
  }
}

function updateTimeRange() {
  updateChart();
}

watch(() => props.data, () => {
  updateChart();
}, { deep: true });

onMounted(() => {
  updateChart();
});

onUnmounted(() => {
  if (chartInstance) {
    chartInstance.destroy();
    chartInstance = null;
  }
});
</script>

