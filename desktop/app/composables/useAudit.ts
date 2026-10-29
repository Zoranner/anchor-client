import { invoke } from "@tauri-apps/api/core";
import {
  auditQueryFromFilters,
  nextCursorAfterPage,
  type AuditPage,
  type AuditViewEvent,
  type AuditFilterInput,
} from "~/utils/audit";

export function useAudit() {
  const events = ref<AuditViewEvent[]>([]);
  const chainHealthy = ref(true);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const cursor = ref<number | null>(null);
  const filters = reactive<AuditFilterInput>({
    eventType: "",
    user: "",
    policyVersion: "",
    resourcePrefix: "",
  });

  async function load(reset = true) {
    loading.value = true;
    if (reset) {
      cursor.value = null;
      error.value = null;
    }
    try {
      const page = await invoke<AuditPage>("get_audit_page", {
        query: auditQueryFromFilters(filters, reset ? null : cursor.value, 100),
      });
      events.value = reset ? page.events : [...events.value, ...page.events];
      cursor.value = nextCursorAfterPage(page);
      chainHealthy.value = page.chain_healthy;
      error.value = null;
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
    } finally {
      loading.value = false;
    }
  }

  function loadMore() {
    return load(false);
  }

  onMounted(() => void load());

  return {
    events,
    chainHealthy,
    loading,
    error,
    filters,
    cursor,
    load,
    loadMore,
  };
}
