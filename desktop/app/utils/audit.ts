export interface AuditFilter {
  since: string | null;
  until: string | null;
  event_type: string | null;
  user: string | null;
  policy_version: number | null;
  resource_prefix: string | null;
}

export interface AuditFilterInput {
  eventType?: string;
  user?: string;
  policyVersion?: string;
  resourcePrefix?: string;
}

export interface AuditQuery {
  filter: AuditFilter;
  after_sequence: number | null;
  limit: number;
}

export interface AuditEvent {
  event_id: string;
  device_id: string;
  user: string;
  timestamp: string;
  event_type: string;
  process: string | null;
  resource: string | null;
  policy_version: number | null;
  action: string;
  result: string;
  rule_id: string | null;
  prev_hash: string;
  event_hash: string;
}

export interface AuditViewEvent {
  sequence: number;
  event: AuditEvent;
}

export interface AuditPage {
  events: AuditViewEvent[];
  next_after_sequence: number | null;
  chain_healthy: boolean;
}

export function auditQueryFromFilters(
  input: AuditFilterInput,
  afterSequence: number | null,
  limit: number,
): AuditQuery {
  const policyVersion =
    input.policyVersion && input.policyVersion.trim() !== ""
      ? Number(input.policyVersion)
      : null;
  return {
    filter: {
      since: null,
      until: null,
      event_type: input.eventType?.trim() || null,
      user: input.user?.trim() || null,
      policy_version:
        policyVersion !== null && Number.isFinite(policyVersion)
          ? policyVersion
          : null,
      resource_prefix: input.resourcePrefix?.trim() || null,
    },
    after_sequence: afterSequence,
    limit,
  };
}

export function nextCursorAfterPage(page: AuditPage): number | null {
  return page.next_after_sequence;
}

export function shortHash(value: string): string {
  return value.length > 12 ? `${value.slice(0, 12)}…` : value;
}
