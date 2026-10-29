<script setup lang="ts">
import { NotificationContainer } from "@stellar/ui";
import AppSidebar from "~/components/shell/AppSidebar.vue";
import AppTitlebar from "~/components/shell/AppTitlebar.vue";
</script>

<template>
  <div class="app-root">
    <AppTitlebar />
    <div class="app-body">
      <AppSidebar />
      <main class="app-main">
        <NuxtPage />
      </main>
    </div>
    <NotificationContainer position="top-right" :max="5" />
  </div>
</template>

<style scoped>
.app-root {
  display: grid;
  height: 100dvh;
  min-height: 0;
  grid-template-rows: var(--pr-titlebar-height) minmax(0, 1fr);
  overflow: hidden;
  border: 1px solid var(--st-border);
  background: var(--st-bg-panel);
  color: var(--st-text-primary);
}

.app-body {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-columns: auto minmax(0, 1fr);
}

.app-main {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  background: var(--st-bg-base);
}
</style>
