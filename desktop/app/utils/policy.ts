export type ActionPolicy = "allow" | "deny";

export interface UsbRules {
  read: ActionPolicy;
  write: ActionPolicy;
  allowed: Array<Record<string, string | null>>;
}

export interface NetworkRules {
  policy: ActionPolicy;
  allowed: Array<{ adapter_id: string }>;
}

export interface BurningRules {
  policy: ActionPolicy;
  allowed: Array<{ drive_id: string }>;
}

export interface ScanRule {
  rule_id: string;
  kind: "keyword" | "regex";
  pattern: string;
}

export interface ScanRules {
  controlled_dirs: string[];
  file_types: string[];
  rules: ScanRule[];
}

export interface PolicyPayload {
  version: number;
  expires_at: string;
  usb: UsbRules;
  network: NetworkRules;
  burning: BurningRules;
  scan: ScanRules;
}

export interface PolicySummary {
  imported_at: string | null;
  payload: PolicyPayload;
}

export type AuthorizationResponse = "allowed" | { denied: { reason: string } };

export interface DesktopStatus {
  authorization: AuthorizationResponse;
  policy: PolicySummary | null;
  last_trusted_at: string | null;
  server?: ServerStatusView | null;
}

export type ServerSyncState =
  | "unknown"
  | "pending"
  | "active"
  | "rejected"
  | "disabled"
  | "unreachable"
  | "error";

export interface ServerStatusView {
  state: ServerSyncState;
  last_sync_at: string | null;
  last_error: string | null;
  policy_version: number | null;
}

export interface PolicyInspection {
  accepted: boolean;
  reason: string;
  current: PolicySummary | null;
  candidate: PolicySummary | null;
}

export interface DomainRow {
  key: string;
  title: string;
  icon: string;
  allowed: boolean;
  summary: string;
  exceptions: string[];
  details: string[];
}

export function isAllowed(
  authorization: AuthorizationResponse | undefined,
): boolean {
  return authorization === "allowed";
}

export function denialReason(
  authorization: AuthorizationResponse | undefined,
): string | null {
  return authorization && authorization !== "allowed"
    ? authorization.denied.reason
    : null;
}

export function describeDenial(reason: string | null | undefined): string {
  switch (reason) {
    case "no_active_policy":
      return "尚未导入策略";
    case "expired":
      return "策略已到期";
    case "trusted_time_unavailable":
      return "运行状态不可用";
    case "clock_rollback":
      return "检测到系统时间回拨";
    case "audit_unhealthy":
      return "审计链校验失败";
    case "enforcement_unhealthy":
      return "策略执行异常";
    default:
      return "授权状态不可用";
  }
}

export function formatRemaining(
  expiresAt: string | null | undefined,
  now = new Date(),
): string {
  if (!expiresAt) return "到期时间未知";
  const expiry = new Date(expiresAt).getTime();
  if (Number.isNaN(expiry)) return "到期时间未知";
  const days = Math.ceil((expiry - now.getTime()) / 86_400_000);
  return days > 0 ? `剩余 ${days} 天` : "已到期";
}

export function formatDateTime(value: string | null | undefined): string {
  if (!value) return "—";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString("zh-CN");
}

function describeAction(value: ActionPolicy): string {
  return value === "allow" ? "允许" : "禁止";
}

function usbExceptionLabel(rule: Record<string, string | null>): string {
  return (
    rule.instance_id ||
    [rule.vendor_id, rule.product_id].filter(Boolean).join(" / ") ||
    rule.serial ||
    "已填写匹配条件"
  );
}

export function policyDomainRows(payload: PolicyPayload): DomainRow[] {
  return [
    {
      key: "usb",
      title: "USB 存储",
      icon: "ph:usb",
      allowed: payload.usb.read === "allow" || payload.usb.write === "allow",
      summary: `读取${describeAction(payload.usb.read)} · 写入${describeAction(payload.usb.write)}`,
      exceptions: payload.usb.allowed.map(usbExceptionLabel),
      details: [`${payload.usb.allowed.length} 个允许例外`],
    },
    {
      key: "network",
      title: "网络",
      icon: "ph:wifi-high",
      allowed: payload.network.policy === "allow",
      summary: describeAction(payload.network.policy),
      exceptions: payload.network.allowed.map((rule) => rule.adapter_id),
      details: [`${payload.network.allowed.length} 个允许网卡`],
    },
    {
      key: "burning",
      title: "刻录",
      icon: "ph:disc",
      allowed: payload.burning.policy === "allow",
      summary: describeAction(payload.burning.policy),
      exceptions: payload.burning.allowed.map((rule) => rule.drive_id),
      details: [`${payload.burning.allowed.length} 个允许光驱`],
    },
    {
      key: "scan",
      title: "文件扫描",
      icon: "ph:magnifying-glass",
      allowed:
        payload.scan.controlled_dirs.length > 0 ||
        payload.scan.rules.length > 0,
      summary:
        payload.scan.controlled_dirs.length > 0
          ? `受控目录 ${payload.scan.controlled_dirs.length} 个`
          : "未配置受控目录",
      exceptions: [
        ...payload.scan.controlled_dirs,
        ...payload.scan.file_types.map((value) => `类型: ${value}`),
      ],
      details: [`${payload.scan.rules.length} 条识别规则`],
    },
  ];
}
