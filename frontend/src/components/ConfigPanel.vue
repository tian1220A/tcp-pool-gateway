<script setup>
import { ref, watch, reactive } from 'vue';
import { ElMessage } from 'element-plus';
import { updateConfig } from '../api';

const props = defineProps({
  stats: { type: Object, default: null },
});
const emit = defineEmits(['saved']);

const form = reactive({
  max_connections: 32,
  idle_timeout_ms: 30000,
  acquire_timeout_ms: 5000,
});

const saving = ref(false);

// Sync form with live stats whenever they change (and the form is not dirty).
watch(
  () => props.stats,
  (s) => {
    if (!s) return;
    form.max_connections = s.max_connections;
    form.idle_timeout_ms = s.idle_timeout_ms;
    form.acquire_timeout_ms = s.acquire_timeout_ms;
  },
  { immediate: true }
);

async function save() {
  saving.value = true;
  try {
    await updateConfig({
      max_connections: form.max_connections,
      idle_timeout_ms: form.idle_timeout_ms,
      acquire_timeout_ms: form.acquire_timeout_ms,
    });
    emit('saved');
  } catch (e) {
    ElMessage.error(`更新失败：${e}`);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <el-card shadow="hover" class="panel">
    <template #header>
      <span>配置面板</span>
    </template>
    <el-form label-position="top" @submit.prevent>
      <el-form-item label="最大连接数 (max_connections)">
        <el-input-number v-model="form.max_connections" :min="1" :max="1024" :step="1" style="width: 100%" />
      </el-form-item>
      <el-form-item label="空闲超时 (ms)">
        <el-input-number v-model="form.idle_timeout_ms" :min="1000" :max="3600000" :step="1000" style="width: 100%" />
      </el-form-item>
      <el-form-item label="获取连接超时 (ms)">
        <el-input-number v-model="form.acquire_timeout_ms" :min="100" :max="60000" :step="100" style="width: 100%" />
      </el-form-item>
      <el-button type="primary" :loading="saving" style="width: 100%" @click="save">
        应用配置
      </el-button>
    </el-form>
  </el-card>
</template>

<style scoped>
.panel :deep(.el-card__header) {
  font-weight: 600;
}
</style>
