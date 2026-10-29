<script setup lang="ts">
import { Alert, Button, Modal, Steps } from "@stellar/ui";
import type { PolicyInspection } from "~/utils/policy";

defineProps<{
  visible: boolean;
  step: number;
  busy: boolean;
  fileName: string;
  inspection: PolicyInspection | null;
  error: string | null;
  result: string | null;
}>();
const emit = defineEmits<{
  "update:visible": [value: boolean];
  choose: [];
  apply: [];
}>();

const steps = [
  { title: "选择文件", description: "选择管理员签发的策略包" },
  { title: "核对变化", description: "查看版本、有效期和规则变化" },
  { title: "导入结果", description: "服务确认是否替换当前策略" },
];

const reasonText: Record<string, string> = {
  policy_parse_failed: "策略文件内容无法解析",
  invalid_signature: "策略签名无效",
  version_not_newer: "策略版本没有高于当前版本",
  expired: "策略已过期",
  policy_too_large: "策略文件超过 256 KiB",
};
</script>

<template>
  <Modal
    :visible="visible"
    title="导入策略"
    :show-confirm="false"
    :show-cancel="false"
    :blocked="busy"
    @update:visible="emit('update:visible', $event)"
  >
    <div class="import-dialog">
      <Steps :steps="steps" :current="step - 1" direction="horizontal" />

      <Alert
        v-if="error"
        semantic="error"
        title="导入未完成"
        :model-value="true"
      >
        {{ reasonText[error] ?? error }}
      </Alert>
      <Alert
        v-else-if="result"
        semantic="success"
        title="导入成功"
        :model-value="true"
      >
        {{ result }}
      </Alert>

      <template v-if="step === 1">
        <p>
          策略由管理员在管理电脑上签发。请选择本地策略文件，服务会在导入前校验签名、版本和有效期。
        </p>
        <Button
          semantic="primary"
          variant="solid"
          icon="ph:file-arrow-up"
          :disabled="busy"
          @click="emit('choose')"
        >
          选择策略文件
        </Button>
      </template>

      <template v-else-if="step === 2 && inspection">
        <p class="file-name">{{ fileName }}</p>
        <div class="compare">
          <div>
            <span>当前版本</span>
            <strong>v{{ inspection.current?.payload.version ?? "无" }}</strong>
          </div>
          <div>
            <span>新版本</span>
            <strong>v{{ inspection.candidate?.payload.version ?? "—" }}</strong>
          </div>
          <div>
            <span>新到期时间</span>
            <strong>{{
              inspection.candidate?.payload.expires_at ?? "—"
            }}</strong>
          </div>
        </div>
        <p v-if="!inspection.accepted" class="inspection-reason">
          {{ reasonText[inspection.reason] ?? inspection.reason }}
        </p>
        <div class="actions">
          <Button :disabled="busy" @click="emit('update:visible', false)"
            >取消</Button
          >
          <Button
            semantic="primary"
            variant="solid"
            :disabled="busy || !inspection.accepted"
            @click="emit('apply')"
          >
            {{ busy ? "正在导入…" : "导入并应用" }}
          </Button>
        </div>
      </template>

      <template v-else>
        <div class="actions">
          <Button
            semantic="primary"
            variant="solid"
            @click="emit('update:visible', false)"
          >
            关闭
          </Button>
        </div>
      </template>
    </div>
  </Modal>
</template>

<style scoped>
.import-dialog {
  display: grid;
  gap: var(--spacing-lg);
}

.file-name {
  margin: 0;
  overflow-wrap: anywhere;
  font-family: var(--font-family-mono);
  font-size: 12px;
}

.compare {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--spacing-md);
}

.compare div {
  display: grid;
  gap: 4px;
}

.compare span {
  color: var(--st-text-muted);
  font-size: 12px;
}

.inspection-reason {
  margin: 0;
  color: var(--st-danger);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-sm);
}
</style>
