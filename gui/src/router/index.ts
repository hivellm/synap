import { createRouter, createWebHistory } from 'vue-router';
import DashboardView from '@/views/DashboardView.vue';

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'Dashboard',
      component: DashboardView,
    },
    {
      path: '/metrics',
      name: 'Metrics',
      component: () => import('@/views/MetricsView.vue'),
    },
    {
      path: '/kv-store',
      name: 'KV Store',
      component: () => import('@/views/KVStoreView.vue'),
    },
    {
      path: '/queues',
      name: 'Queues',
      component: () => import('@/views/QueuesView.vue'),
    },
    {
      path: '/streams',
      name: 'Streams',
      component: () => import('@/views/StreamsView.vue'),
    },
    {
      path: '/pubsub',
      name: 'Pub/Sub',
      component: () => import('@/views/PubSubView.vue'),
    },
    {
      path: '/logs',
      name: 'Logs',
      component: () => import('@/views/LogsView.vue'),
    },
    {
      path: '/config',
      name: 'Configuration',
      component: () => import('@/views/ConfigView.vue'),
    },
    {
      path: '/replication',
      name: 'Replication',
      component: () => import('@/views/ReplicationView.vue'),
    },
    {
      path: '/structures',
      name: 'Data Structures',
      component: () => import('@/views/StructuresView.vue'),
    },
  ],
});

export default router;

