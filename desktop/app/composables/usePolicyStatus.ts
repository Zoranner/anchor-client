import { invoke } from "@tauri-apps/api/core";
import type { DesktopStatus } from "~/utils/policy";

export function usePolicyStatus() {
  const status = ref<DesktopStatus | null>(null);
  const loading = ref(false);
  const stale = ref(false);
  const error = ref<string | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;

  async function refresh() {
    loading.value = true;
    try {
      status.value = await invoke<DesktopStatus>("get_policy_status");
      error.value = null;
      stale.value = false;
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
      stale.value = status.value !== null;
    } finally {
      loading.value = false;
    }
  }

  onMounted(() => {
    void refresh();
    timer = setInterval(() => void refresh(), 10_000);
    window.addEventListener("focus", refresh);
  });

  onUnmounted(() => {
    if (timer) clearInterval(timer);
    window.removeEventListener("focus", refresh);
  });

  return { status, loading, stale, error, refresh };
}
