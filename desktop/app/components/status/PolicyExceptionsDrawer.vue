<script setup lang="ts">
import { Drawer } from "@stellar/ui";

defineProps<{
  visible: boolean;
  title: string;
  items: string[];
}>();
const emit = defineEmits<{ "update:visible": [value: boolean] }>();
</script>

<template>
  <Drawer
    :visible="visible"
    :title="title"
    position="right"
    size="default"
    :show-confirm="false"
    cancel-text="关闭"
    @update:visible="emit('update:visible', $event)"
  >
    <ul class="exception-list">
      <li v-for="item in items" :key="item">{{ item }}</li>
    </ul>
  </Drawer>
</template>

<style scoped>
.exception-list {
  display: grid;
  gap: var(--spacing-sm);
  margin: 0;
  padding-left: 1.2em;
  overflow-wrap: anywhere;
  font-family: var(--font-family-mono);
  font-size: 12px;
}
</style>
