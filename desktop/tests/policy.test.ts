import { describe, expect, test } from "bun:test";
import {
  describeDenial,
  formatRemaining,
  policyDomainRows,
} from "../app/utils/policy";

describe("policy presentation", () => {
  test("maps denial reasons to user-facing text", () => {
    expect(describeDenial("no_active_policy")).toBe("尚未导入策略");
    expect(describeDenial("expired")).toBe("策略已到期");
    expect(describeDenial("clock_rollback")).toBe("检测到系统时间回拨");
  });

  test("formats remaining days without inventing a progress value", () => {
    expect(
      formatRemaining("2026-10-31T00:00:00Z", new Date("2026-10-24T00:00:00Z")),
    ).toBe("剩余 7 天");
    expect(
      formatRemaining("2026-10-23T00:00:00Z", new Date("2026-10-24T00:00:00Z")),
    ).toBe("已到期");
  });

  test("creates four policy domain rows from a payload", () => {
    const rows = policyDomainRows({
      version: 1,
      expires_at: "2026-12-31T00:00:00Z",
      usb: { read: "deny", write: "deny", allowed: [] },
      network: { policy: "deny", allowed: [] },
      burning: { policy: "deny", allowed: [] },
      scan: { controlled_dirs: [], file_types: [], rules: [] },
    });
    expect(rows.map((row) => row.key)).toEqual([
      "usb",
      "network",
      "burning",
      "scan",
    ]);
  });
});
