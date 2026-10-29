<script setup lang="ts">
import { Alert, Button, EmptyState, Input, Select, Table } from "@stellar/ui";
import { shortHash } from "~/utils/audit";
import { formatDateTime } from "~/utils/policy";

const audit = useAudit();
const eventTypeOptions = [
  { value: "", label: "全部事件" },
  { value: "policy_imported", label: "策略导入" },
  { value: "policy_import_rejected", label: "策略导入拒绝" },
  { value: "authorization_denied", label: "授权拒绝" },
  { value: "scan_hit", label: "扫描命中" },
  { value: "scan_completed", label: "扫描完成" },
  { value: "service_start", label: "服务启动" },
  { value: "service_stop", label: "服务停止" },
  { value: "audit_verify", label: "审计校验" },
];
const rows = computed(() =>
  audit.events.value.map(({ sequence, event }) => ({
    id: String(sequence),
    time: formatDateTime(event.timestamp),
    event_type: event.event_type,
    user: event.user,
    action: event.action,
    result: event.result,
    resource: event.resource ?? "—",
    policy_version: event.policy_version ?? "—",
    hash: shortHash(event.event_hash),
  })),
);
const columns = [
  { key: "time", title: "时间", width: 168 },
  { key: "event_type", title: "事件", width: 160 },
  { key: "user", title: "操作者", width: 110 },
  { key: "action", title: "动作", width: 120 },
  { key: "result", title: "结果", width: 120 },
  { key: "resource", title: "资源" },
  { key: "policy_version", title: "策略", width: 70 },
  { key: "hash", title: "事件哈希", width: 140 },
];
</script>

<template>
  <main class="page">
    <header class="page-header">
      <div>
        <h1>审计日志</h1>
        <p>只读查看本机审计事件和 hash chain 校验结论。</p>
      </div>
      <Button
        icon="ph:arrows-clockwise"
        :disabled="audit.loading.value"
        @click="audit.load()"
      >
        {{ audit.loading.value ? "刷新中…" : "刷新" }}
      </Button>
    </header>

    <Alert
      v-if="!audit.chainHealthy.value"
      semantic="error"
      title="审计链异常"
      :model-value="true"
    >
      审计库的 hash chain 校验失败，当前结果只能作为异常证据查看。
    </Alert>
    <Alert
      v-if="audit.error.value"
      semantic="error"
      title="查询失败"
      :model-value="true"
    >
      {{ audit.error.value }}
    </Alert>

    <section class="filters">
      <Select
        v-model="audit.filters.eventType"
        :options="eventTypeOptions"
        label="事件类型"
        placeholder="全部事件"
      />
      <Input
        v-model="audit.filters.user"
        label="操作者"
        placeholder="如 alice"
      />
      <Input
        v-model="audit.filters.policyVersion"
        label="策略版本"
        placeholder="如 3"
      />
      <Input
        v-model="audit.filters.resourcePrefix"
        label="资源前缀"
        placeholder="如 D:\data"
      />
      <Button semantic="primary" variant="solid" @click="audit.load()">
        应用筛选
      </Button>
    </section>

    <Table
      v-if="rows.length > 0"
      :columns="columns"
      :data="rows"
      row-key="id"
      :loading="audit.loading.value"
      empty-text="没有匹配的审计事件"
    />
    <EmptyState
      v-else-if="!audit.loading.value"
      icon="ph:list-magnifying-glass"
      title="没有匹配的审计事件"
      description="调整筛选条件后重新查询。"
    />

    <div v-if="audit.cursor.value !== null" class="load-more">
      <Button :disabled="audit.loading.value" @click="audit.loadMore()">
        加载更多
      </Button>
    </div>
  </main>
</template>

<style scoped>
.page {
  display: grid;
  height: 100%;
  min-height: 0;
  align-content: start;
  gap: var(--spacing-lg);
  padding: var(--spacing-lg);
  overflow: auto;
}

.page-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--spacing-lg);
}

h1 {
  margin: 0;
  font-size: 20px;
  font-weight: 650;
}

.page-header p {
  margin: 4px 0 0;
  color: var(--st-text-muted);
  font-size: 13px;
}

.filters {
  display: grid;
  grid-template-columns: 180px minmax(140px, 1fr) 120px minmax(160px, 1fr) auto;
  align-items: end;
  gap: var(--spacing-sm);
}

.load-more {
  display: flex;
  justify-content: center;
  padding-bottom: var(--spacing-lg);
}

@media (max-width: 900px) {
  .filters {
    grid-template-columns: 1fr 1fr;
  }
}
</style>
