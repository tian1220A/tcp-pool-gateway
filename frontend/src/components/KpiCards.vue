<script setup>
import { computed } from 'vue';

const props = defineProps({
  stats: { type: Object, default: null },
});

const cards = computed(() => {
  const s = props.stats;
  if (!s) return [];
  return [
    { label: '总连接数', value: s.total, color: '#409EFF', suffix: '' },
    { label: '空闲连接', value: s.idle, color: '#67C23A', suffix: '' },
    { label: '活跃连接', value: s.active, color: '#E6A23C', suffix: '' },
    { label: '连接复用次数', value: s.reuse_count, color: '#909399', suffix: ' 次' },
    { label: '最大连接上限', value: s.max_connections, color: '#F56C6C', suffix: '' },
  ];
});
</script>

<template>
  <el-row :gutter="16">
    <el-col v-for="c in cards" :key="c.label" :xs="12" :sm="8" :md="4.8">
      <el-card shadow="hover" class="kpi-card">
        <div class="kpi">
          <div class="kpi-label">{{ c.label }}</div>
          <div class="kpi-value" :style="{ color: c.color }">
            {{ c.value }}<span class="kpi-suffix">{{ c.suffix }}</span>
          </div>
        </div>
      </el-card>
    </el-col>
  </el-row>
</template>

<style scoped>
.kpi-card {
  margin-bottom: 16px;
  text-align: center;
}
.kpi {
  padding: 8px 0;
}
.kpi-label {
  font-size: 13px;
  color: #909399;
  margin-bottom: 8px;
}
.kpi-value {
  font-size: 28px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.kpi-suffix {
  font-size: 13px;
  font-weight: 400;
  color: #909399;
}
</style>
