<script setup lang="ts">
import { Sidebar, SidebarItem } from "@stellar/ui";

const route = useRoute();
const navigation = [
  { label: "策略状态", path: "/", icon: "ph:shield-check" },
  { label: "审计日志", path: "/audit", icon: "ph:list-magnifying-glass" },
];
</script>

<template>
  <Sidebar variant="rail" :show-header="false">
    <SidebarItem
      v-for="item in navigation"
      :key="item.path"
      :active="route.path === item.path"
      :icon="item.icon"
      :label="item.label"
      :to="item.path"
    />
    <template #footer>
      <div class="sidebar-footer" title="Anchor Guard">
        <span>AG</span>
      </div>
    </template>
  </Sidebar>
</template>

<style scoped>
.sidebar-footer {
  display: grid;
  width: 100%;
  place-items: center;
  padding: var(--spacing-sm) 0 var(--spacing-md);
  color: var(--st-text-muted);
  font-family: var(--font-family-mono);
  font-size: 11px;
}
</style>
