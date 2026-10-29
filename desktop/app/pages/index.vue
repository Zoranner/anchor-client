<script setup lang="ts">
import { Alert, Button, EmptyState } from "@stellar/ui";
import PolicyDomainCard from "~/components/status/PolicyDomainCard.vue";
import PolicyExceptionsDrawer from "~/components/status/PolicyExceptionsDrawer.vue";
import StatusHero from "~/components/status/StatusHero.vue";
import PolicyImportDialog from "~/components/import/PolicyImportDialog.vue";
import ServerStateCard from "~/components/status/ServerStateCard.vue";
import { policyDomainRows, type DomainRow } from "~/utils/policy";

const policyStatus = usePolicyStatus();
const importer = usePolicyImport(policyStatus.refresh);
const rows = computed(() =>
  policyStatus.status.value?.policy
    ? policyDomainRows(policyStatus.status.value.policy.payload)
    : [],
);
const exceptionsVisible = ref(false);
const exceptionTitle = ref("");
const exceptionItems = ref<string[]>([]);

function showExceptions(row: DomainRow) {
  exceptionTitle.value = `${row.title}的允许例外`;
  exceptionItems.value = row.exceptions;
  exceptionsVisible.value = true;
}
</script>

<template>
  <main class="page">
    <header class="page-header">
      <div>
        <h1>策略状态</h1>
        <p>查看当前策略的授权结论和管控规则。</p>
      </div>
      <div class="page-actions">
        <Button
          variant="ghost"
          icon="ph:arrows-clockwise"
          :disabled="policyStatus.loading.value"
          @click="policyStatus.refresh"
        >
          {{ policyStatus.loading.value ? "刷新中…" : "刷新" }}
        </Button>
        <Button
          semantic="primary"
          variant="solid"
          icon="ph:file-arrow-up"
          @click="importer.open"
        >
          导入策略
        </Button>
      </div>
    </header>

    <Alert
      v-if="policyStatus.error.value"
      semantic="error"
      title="无法读取策略状态"
      :model-value="true"
    >
      {{ policyStatus.error.value }}
    </Alert>

    <template v-if="policyStatus.status.value">
      <StatusHero
        :status="policyStatus.status.value"
        :stale="policyStatus.stale.value"
      />
      <ServerStateCard :server="policyStatus.status.value.server" />

      <template v-if="policyStatus.status.value.policy">
        <section class="domain-grid">
          <PolicyDomainCard
            v-for="row in rows"
            :key="row.key"
            :row="row"
            @show-exceptions="showExceptions"
          />
        </section>
      </template>
      <EmptyState
        v-else
        icon="ph:shield-warning"
        title="尚未导入策略"
        description="导入管理员签发的策略后，才能查看管控规则并恢复正常登录。"
      >
        <Button semantic="primary" variant="solid" @click="importer.open">
          导入策略
        </Button>
      </EmptyState>
    </template>

    <div v-else-if="!policyStatus.loading.value" class="initial-state">
      <EmptyState
        icon="ph:plug"
        title="服务不可用"
        description="本地 Anchor Guard 服务没有响应，请确认服务正在运行。"
      >
        <Button @click="policyStatus.refresh">重新读取</Button>
      </EmptyState>
    </div>

    <PolicyExceptionsDrawer
      v-model:visible="exceptionsVisible"
      :title="exceptionTitle"
      :items="exceptionItems"
    />
    <PolicyImportDialog
      :visible="importer.visible.value"
      :step="importer.step.value"
      :busy="importer.busy.value"
      :file-name="importer.fileName.value"
      :inspection="importer.inspection.value"
      :error="importer.error.value"
      :result="importer.result.value"
      @update:visible="importer.close"
      @choose="importer.chooseFile"
      @apply="importer.apply"
    />
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

.page-actions {
  display: flex;
  flex: 0 0 auto;
  gap: var(--spacing-sm);
}

.domain-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--spacing-lg);
}

.initial-state {
  display: grid;
  min-height: 260px;
  place-items: center;
}

@media (max-width: 900px) {
  .domain-grid {
    grid-template-columns: 1fr;
  }

  .page-header {
    flex-direction: column;
  }
}
</style>
