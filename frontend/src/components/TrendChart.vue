<script setup>
import { ref, watch, onMounted, onBeforeUnmount, nextTick } from 'vue';
import * as echarts from 'echarts';

const props = defineProps({
  history: { type: Array, default: () => [] },
});

const chartEl = ref(null);
let chart = null;

function fmtTime(ms) {
  const d = new Date(ms);
  const p = (n) => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

function render() {
  if (!chartEl.value) return;
  if (!chart) {
    chart = echarts.init(chartEl.value);
  }
  const h = props.history;
  const times = h.map((p) => fmtTime(p.ts_ms));
  const series = [
    { name: '总连接', key: 'total', color: '#409EFF' },
    { name: '空闲', key: 'idle', color: '#67C23A' },
    { name: '活跃', key: 'active', color: '#E6A23C' },
  ];
  chart.setOption({
    tooltip: { trigger: 'axis' },
    legend: { data: series.map((s) => s.name), top: 0 },
    grid: { left: 40, right: 20, top: 40, bottom: 30 },
    xAxis: {
      type: 'category',
      boundaryGap: false,
      data: times,
    },
    yAxis: { type: 'value', minInterval: 1 },
    series: series.map((s) => ({
      name: s.name,
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: h.map((p) => p[s.key]),
      itemStyle: { color: s.color },
      lineStyle: { width: 2 },
      areaStyle: { opacity: 0.08 },
    })),
  });
}

function onResize() {
  chart?.resize();
}

onMounted(() => {
  nextTick(render);
  window.addEventListener('resize', onResize);
});

onBeforeUnmount(() => {
  window.removeEventListener('resize', onResize);
  chart?.dispose();
  chart = null;
});

watch(() => props.history, render);
</script>

<template>
  <el-card shadow="hover" class="panel">
    <template #header>
      <div class="panel-header">
        <span>连接池趋势</span>
        <span class="hint">每 1s 采样，最近 5 分钟</span>
      </div>
    </template>
    <div ref="chartEl" class="chart"></div>
  </el-card>
</template>

<style scoped>
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.panel-header .hint {
  font-size: 12px;
  color: #909399;
  font-weight: 400;
}
.chart {
  height: 340px;
}
</style>
