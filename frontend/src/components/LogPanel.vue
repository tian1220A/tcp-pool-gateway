<script setup>
import { computed } from 'vue';

const props = defineProps({
  logs: { type: Array, default: () => [] },
  counters: { type: Object, default: null },
});

const levelType = (level) => (level === 'error' ? 'danger' : level === 'warn' ? 'warning' : 'info');

const counterItems = computed(() => {
  const c = props.counters;
  if (!c) return [];
  return [
    { label: '总请求', value: c.total_requests, type: 'primary' },
    { label: '后端错误', value: c.backend_errors, type: 'danger' },
    { label: '获取连接错误', value: c.acquire_errors, type: 'warning' },
    { label: '连接池超时', value: c.pool_timeouts, type: 'warning' },
  ];
});

const rows = computed(() =>
  [...props.logs].reverse().map((l) => ({
    ...l,
    time: new Date(l.ts_ms).toLocaleTimeString('zh-CN', { hour12: false }),
  }))
);
</script>

<template>
  <el-card shadow="hover">
    <template #header>
      <div class="panel-header">
        <span>运行日志</span>
        <div class="counters">
          <el-tag v-for="c in counterItems" :key="c.label" :type="c.type" effect="plain" class="counter-tag">
            {{ c.label }}: {{ c.value }}
          </el-tag>
        </div>
      </div>
    </template>

    <el-table :data="rows" height="320" size="small" stripe>
      <el-table-column prop="time" label="时间" width="110" />
      <el-table-column label="级别" width="90">
        <template #default="{ row }">
          <el-tag :type="levelType(row.level)" size="small">{{ row.level }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="message" label="内容" show-overflow-tooltip />
    </el-table>
  </el-card>
</template>

<style scoped>
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.counters {
  display: flex;
  gap: 8px;
}
</style>
