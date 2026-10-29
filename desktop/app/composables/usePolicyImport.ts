import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { PolicyInspection } from "~/utils/policy";

interface InspectResult {
  pending_id: string;
  inspection: PolicyInspection;
}

interface ImportResult {
  accepted: boolean;
  reason: string;
}

const reasonText: Record<string, string> = {
  policy_parse_failed: "策略文件内容无法解析",
  invalid_signature: "策略签名无效",
  version_not_newer: "策略版本没有高于当前版本",
  expired: "策略已过期",
  policy_too_large: "策略文件超过 256 KiB",
  store_unavailable: "本地策略库不可用",
  pending_not_found: "待导入策略已失效，请重新选择",
};

export function usePolicyImport(autoRefresh?: () => Promise<void> | void) {
  const visible = ref(false);
  const step = ref(1);
  const busy = ref(false);
  const fileName = ref("");
  const pendingId = ref<string | null>(null);
  const inspection = ref<PolicyInspection | null>(null);
  const error = ref<string | null>(null);
  const result = ref<string | null>(null);

  function open() {
    visible.value = true;
    step.value = 1;
    busy.value = false;
    fileName.value = "";
    pendingId.value = null;
    inspection.value = null;
    error.value = null;
    result.value = null;
  }

  async function chooseFile() {
    error.value = null;
    const selected = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "策略包", extensions: ["policy", "json"] }],
    });
    const path = Array.isArray(selected) ? selected[0] : selected;
    if (!path) return;
    busy.value = true;
    try {
      fileName.value = path.split(/[\\/]/).pop() ?? path;
      const inspected = await invoke<InspectResult>("inspect_policy_file", {
        path,
      });
      pendingId.value = inspected.pending_id;
      inspection.value = inspected.inspection;
      step.value = 2;
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy.value = false;
    }
  }

  async function apply() {
    if (!pendingId.value || !inspection.value?.accepted) return;
    busy.value = true;
    error.value = null;
    try {
      const imported = await invoke<ImportResult>("import_pending_policy", {
        pendingId: pendingId.value,
      });
      if (!imported.accepted) {
        error.value = reasonText[imported.reason] ?? imported.reason;
        step.value = 3;
        return;
      }
      pendingId.value = null;
      result.value = "策略已导入，设备将在下一轮核对后应用";
      step.value = 3;
      await autoRefresh?.();
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
      step.value = 3;
    } finally {
      busy.value = false;
    }
  }

  async function close() {
    if (busy.value) return;
    if (pendingId.value) {
      try {
        await invoke("discard_pending_policy", { pendingId: pendingId.value });
      } catch {
        // The pending cache expires with the process even if discard fails.
      }
    }
    visible.value = false;
  }

  return {
    visible,
    step,
    busy,
    fileName,
    pendingId,
    inspection,
    error,
    result,
    open,
    chooseFile,
    apply,
    close,
  };
}
