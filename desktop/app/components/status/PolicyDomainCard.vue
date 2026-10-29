<script setup lang="ts">
import { Badge, Button, Card, Tag } from "@stellar/ui";
import type { DomainRow } from "~/utils/policy";

defineProps<{ row: DomainRow }>();
const emit = defineEmits<{ showExceptions: [row: DomainRow] }>();
</script>

<template>
  <Card :title="row.title" full-height>
    <div class="domain-card">
      <div class="domain-summary">
        <Badge
          :semantic="row.allowed ? 'success' : undefined"
          :palette="row.allowed ? undefined : 'gray'"
          :text="row.allowed ? '允许' : '禁止'"
          :icon="row.allowed ? 'ph:check' : 'ph:minus'"
        />
        <span>{{ row.summary }}</span>
      </div>
      <div class="domain-details">
        <Tag v-for="detail in row.details" :key="detail">{{ detail }}</Tag>
      </div>
      <Button
        v-if="row.exceptions.length > 0"
        size="small"
        variant="ghost"
        icon="ph:list"
        @click="emit('showExceptions', row)"
      >
        查看 {{ row.exceptions.length }} 个例外
      </Button>
    </div>
  </Card>
</template>

<style scoped>
.domain-card {
  display: grid;
  gap: var(--spacing-md);
}

.domain-summary {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
}

.domain-details {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-xs);
}
</style>
