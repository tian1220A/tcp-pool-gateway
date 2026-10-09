<script setup>
import { ref, onMounted, onBeforeUnmount } from 'vue';
import { ElMessage } from 'element-plus';
import KpiCards from './components/KpiCards.vue';
import TrendChart from './components/TrendChart.vue';
import ConfigPanel from './components/ConfigPanel.vue';
import LogPanel from './components/LogPanel.vue';
import { fetchStats, fetchHistory, fetchLogs } from './api';

const stats = ref(null);
const history = ref([]);
const logs = ref([]);
const counters = ref(null);
const lastError = ref('');

let timers = [];

async function loadStats() {
  try {
    stats.value = await fetchStats();
  } catch (e) {
    lastError.value = String(e);
  }
}

async function loadHistory() {
  try {
    history.value = await fetchHistory();
  } catch (e) {
    lastError.value = String(e);
  }
}

async function loadLogs() {
  try {
    const data = await fetchLogs();
    logs.value = data.logs;
    counters.value = data.counters;
  } catch (e) {
    lastError.value = String(e);
  }
}

function onConfigSaved() {
  loadStats();
  ElMessage.success('配置已更新');
}

onMounted(() => {
  loadStats();
  loadHistory();
  loadLogs();
  timers = [
    setInterval(loadStats, 2000),
    setInterval(loadHistory, 2000),
    setInterval(loadLogs, 3000),
  ];
});

onBeforeUnmount(() => {
  timers.forEach(clearInterval);
});
</script>

<template>
  <div class="app">
    <header class="app-header">
      <div>
        <h1>TCP 连接池网关 · 监控后台</h1>
        <p class="subtitle">基于 Rust 异步 TCP 连接池的高性能网关系统</p>
      </div>
      <div class="status">
        <el-tag v-if="stats" type="success" effect="dark" size="large">连接正常</el-tag>
        <el-tag v-else type="danger" effect="dark" size="large">未连接</el-tag>
      </div>
    </header>

    <el-alert
      v-if="lastError"
      :title="`请求失败：${lastError}`"
      type="warning"
      :closable="false"
      show-icon
      class="error-bar"
    />

    <el-row :gutter="16">
      <el-col :span="24">
        <KpiCards :stats="stats" />
      </el-col>
    </el-row>

    <el-row :gutter="16" class="row-gap">
      <el-col :xs="24" :lg="16">
        <TrendChart :history="history" />
      </el-col>
      <el-col :xs="24" :lg="8">
        <ConfigPanel :stats="stats" @saved="onConfigSaved" />
      </el-col>
    </el-row>

    <el-row :gutter="16" class="row-gap">
      <el-col :span="24">
        <LogPanel :logs="logs" :counters="counters" />
      </el-col>
    </el-row>
  </div>
</template>

<style>
:root {
  color-scheme: light;
}
body {
  margin: 0;
  background: #f5f7fa;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial,
    'PingFang SC', 'Microsoft YaHei', sans-serif;
}
.app {
  max-width: 1400px;
  margin: 0 auto;
  padding: 20px;
}
.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}
.app-header h1 {
  margin: 0;
  font-size: 22px;
  color: #303133;
}
.app-header .subtitle {
  margin: 4px 0 0;
  color: #909399;
  font-size: 13px;
}
.error-bar {
  margin-bottom: 16px;
}
.row-gap {
  margin-top: 16px;
}
</style>
