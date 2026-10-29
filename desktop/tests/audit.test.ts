import { expect, test } from "bun:test";
import { auditQueryFromFilters, nextCursorAfterPage } from "../app/utils/audit";

test("maps filters to a cursor query", () => {
  const query = auditQueryFromFilters(
    {
      eventType: "scan_hit",
      user: "alice",
      policyVersion: "3",
      resourcePrefix: "D:\\data",
    },
    null,
    100,
  );
  expect(query.filter.event_type).toBe("scan_hit");
  expect(query.filter.user).toBe("alice");
  expect(query.filter.policy_version).toBe(3);
  expect(query.after_sequence).toBeNull();
});

test("uses the service cursor for the next page", () => {
  expect(
    nextCursorAfterPage({
      events: [],
      next_after_sequence: 42,
      chain_healthy: true,
    }),
  ).toBe(42);
});
