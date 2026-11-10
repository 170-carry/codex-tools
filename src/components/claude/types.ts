export type ClaudeAccount = {
  sync?: {
    lastUsageAt: number | null;
    lastRefreshAt: number | null;
    needsAuth: boolean;
    error: string | null;
    retryAt: number;
  };
  id: string;
  label: string;
  email: string | null;
  kind: "oauth" | "api" | "desktop";
  isCurrent: boolean;
  expiresAt: number | null;
  canRefresh?: boolean;
  plan: string | null;
  baseUrl: string | null;
  model: string | null;
  usage: Record<
    string,
    { utilization: number; resets_at: string | null } | null
  > | null;
};
export type ClaudeAccounts = { accounts: ClaudeAccount[] };
