<script setup lang="ts">
import { Badge, Card, ProgressBar } from "@stellar/ui";
import {
  denialReason,
  describeDenial,
  formatDateTime,
  formatRemaining,
  isAllowed,
  type DesktopStatus,
} from "~/utils/policy";

const props = defineProps<{
  status: DesktopStatus | null;
  stale: boolean;
}>();

const allowed = computed(() => isAllowed(props.status?.authorization));
const reason = computed(() => denialReason(props.status?.authorization));
const policy = computed(() => props.status?.policy ?? null);
const importedAt = computed(() => policy.value?.imported_at ?? null);
const expiresAt = computed(() => policy.value?.payload.expires_at ?? null);
const remaining = computed(() => formatRemaining(expiresAt.value));
const progress = computed(() => {
  if (!importedAt.value || !expiresAt.value) return null;
  const start = new Date(importedAt.value).getTime();
  const end = new Date(expiresAt.value).getTime();
  if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) {
    return null;
  }
  const elapsed = Date.now() - start;
  const value = Math.min(Math.max(elapsed, 0), end - start);
  return { value, max: end - start };
});
</script>

<template>
  <Card>
    <div class="status-hero">
      <div class="status-heading">
        <div>
          <div class="status-label">当前授权</div>
          <h1>{{ allowed ? "策略有效" : describeDenial(reason) }}</h1>
        </div>
        <Badge
          :semantic="allowed ? 'success' : 'warning'"
          :text="allowed ? '正常' : '需要处理'"
        />
      </div>

      <p v-if="stale" class="status-stale">
        显示的是上次成功读取的状态，服务当前不可用。
      </p>

      <div v-if="policy" class="status-metadata">
        <div>
          <span>策略版本</span>
          <strong>v{{ policy.payload.version }}</strong>
        </div>
        <div>
          <span>到期时间</span>
          <strong>{{ formatDateTime(expiresAt) }}</strong>
        </div>
        <div>
          <span>剩余有效期</span>
          <strong>{{ remaining }}</strong>
        </div>
        <div>
          <span>最后导入</span>
          <strong>{{ formatDateTime(importedAt) }}</strong>
        </div>
      </div>

      <ProgressBar
        v-if="progress"
        label="策略有效期"
        :value="progress.value"
        :max="progress.max"
        :value-text="remaining"
      />
    </div>
  </Card>
</template>

<style scoped>
.status-hero {
  display: grid;
  gap: var(--spacing-lg);
}

.status-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--spacing-lg);
}

.status-label {
  color: var(--st-text-muted);
  font-size: 12px;
}

h1 {
  margin: 4px 0 0;
  font-size: 22px;
  font-weight: 650;
}

.status-stale {
  margin: 0;
  color: var(--st-warning, #b7791f);
}

.status-metadata {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--spacing-md);
}

.status-metadata div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.status-metadata span {
  color: var(--st-text-muted);
  font-size: 12px;
}

.status-metadata strong {
  overflow-wrap: anywhere;
  font-size: 14px;
  font-weight: 550;
}

@media (max-width: 900px) {
  .status-metadata {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
