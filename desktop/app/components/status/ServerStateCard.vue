<script setup lang="ts">
import { Badge, Card, Tag } from "@stellar/ui";
import type { ServerStatusView } from "~/utils/policy";
import { formatDateTime } from "~/utils/policy";

const props = defineProps<{
  server: ServerStatusView | null | undefined;
}>();

const meta = computed(() => {
  switch (props.server?.state) {
    case "active":
      return {
        label: "已批准",
        semantic: "success" as const,
        palette: undefined,
      };
    case "pending":
      return {
        label: "待审批",
        semantic: "warning" as const,
        palette: undefined,
      };
    case "rejected":
      return {
        label: "已拒绝",
        semantic: "error" as const,
        palette: undefined,
      };
    case "disabled":
      return {
        label: "已禁用",
        semantic: "error" as const,
        palette: undefined,
      };
    case "unreachable":
      return {
        label: "服务端不可达",
        semantic: "warning" as const,
        palette: undefined,
      };
    case "error":
      return {
        label: "同步异常",
        semantic: "error" as const,
        palette: undefined,
      };
    case "unknown":
      return {
        label: "等待首次同步",
        semantic: "info" as const,
        palette: undefined,
      };
    default:
      return {
        label: "离线模式",
        semantic: undefined,
        palette: "gray" as const,
      };
  }
});
</script>

<template>
  <Card title="服务端同步" :hoverable="false">
    <div class="server-state">
      <div class="server-heading">
        <Badge :semantic="meta.semantic" :palette="meta.palette">
          {{ meta.label }}
        </Badge>
        <span v-if="!server" class="server-hint">
          当前使用离线模式，策略通过本地卷导入。
        </span>
        <span v-else class="server-hint">
          {{
            server.last_sync_at
              ? `最后同步 ${formatDateTime(server.last_sync_at)}`
              : "尚未完成同步"
          }}
        </span>
      </div>

      <div v-if="server" class="server-metadata">
        <Tag v-if="server.policy_version" size="small" icon="ph:shield-check">
          已同步策略 v{{ server.policy_version }}
        </Tag>
        <Tag v-else size="small" icon="ph:shield-slash">尚未同步策略</Tag>
      </div>

      <p v-if="server?.last_error" class="server-error">
        {{ server.last_error }}
      </p>
    </div>
  </Card>
</template>

<style scoped>
.server-state {
  display: grid;
  gap: var(--spacing-md);
}

.server-heading {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--spacing-sm);
}

.server-hint {
  color: var(--st-text-muted);
  font-size: 13px;
}

.server-metadata {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-xs);
}

.server-error {
  margin: 0;
  color: var(--st-danger);
  font-family: var(--font-family-mono);
  font-size: 12px;
  overflow-wrap: anywhere;
}
</style>
