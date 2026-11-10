import type { ClaudeAccounts } from "../../src/components/claude/types";

export function createClaudeMockBackend() {
  const data: ClaudeAccounts = {
    accounts: [
      {
        id: "claude-preview",
        label: "Claude 示例账号",
        email: "claude@example.com",
        kind: "oauth",
        isCurrent: false,
        expiresAt: Date.now() + 3600000,
        canRefresh: true,
        plan: "Max",
        baseUrl: null,
        model: null,
        usage: {
          five_hour: { utilization: 24, resets_at: "2026-11-10T12:00:00Z" },
          seven_day: { utilization: 46, resets_at: "2026-11-15T12:00:00Z" },
        },
      },
    ],
  };
  return (command: string, payload: Record<string, unknown> | undefined) => {
    if (command === "list_authorization_apps")
      return [
        { path: "/Applications/Safari.app", name: "Safari" },
        { path: "/Applications/Google Chrome.app", name: "Google Chrome" },
      ];
    if (
      command === "open_authorization_url" ||
      command === "claude_cancel_oauth"
    )
      return null;
    if (command === "claude_prepare_oauth")
      return "https://claude.com/cai/oauth/authorize?code=true&state=preview";
    if (command === "claude_import_desktop_account")
      data.accounts.push({
        id: "desktop-preview",
        label: "desktop@example.com",
        email: "desktop@example.com",
        kind: "desktop",
        isCurrent: false,
        expiresAt: Date.now() + 3600000,
        canRefresh: true,
        plan: "claude_pro",
        baseUrl: null,
        model: null,
        usage: {
          five_hour: { utilization: 31, resets_at: "2026-11-10T12:00:00Z" },
          seven_day: { utilization: 52, resets_at: "2026-11-15T12:00:00Z" },
        },
        sync: {
          lastUsageAt: Math.floor(Date.now() / 1000),
          lastRefreshAt: null,
          needsAuth: false,
          error: null,
          retryAt: 0,
        },
      });
    if (command === "claude_switch_desktop_account")
      data.accounts.forEach((a) => {
        if (a.kind === "desktop") a.isCurrent = a.id === payload?.accountId;
      });
    if (command === "claude_switch_account")
      data.accounts.forEach((a) => {
        a.isCurrent = a.id === payload?.accountId;
      });
    if (command === "claude_delete_account")
      data.accounts = data.accounts.filter((a) => a.id !== payload?.accountId);
    if (command.startsWith("claude_")) return structuredClone(data);
    return undefined;
  };
}
