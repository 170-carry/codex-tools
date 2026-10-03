import type {
  AccountSummary,
  CodexCostAnalyticsSnapshot,
  CodexTokenTotals,
  CodexTokenUsageSnapshot,
} from "../../src/types/app";

export const now = Math.floor(Date.now() / 1000);
const totals = (value: number): CodexTokenTotals => ({
  inputTokens: value * 0.7,
  cachedInputTokens: value * 0.2,
  outputTokens: value * 0.3,
  reasoningOutputTokens: value * 0.1,
  totalTokens: value,
});

export function fixtureAccount(
  id: string,
  label: string,
  fiveHour: number,
  week: number,
  extra: Partial<AccountSummary> = {},
): AccountSummary {
  return {
    id,
    label,
    email: `${id}@example.com`,
    sourceKind: "chatgpt",
    accountKey: id,
    accountId: id,
    planType: "pro",
    subscriptionActiveUntil: now + 86400 * 15,
    apiBaseUrl: null,
    modelName: null,
    balanceText: null,
    relayBalance: null,
    profileAuthReady: true,
    profileConfigReady: true,
    profileIntegrityError: null,
    profileLastValidatedAt: now,
    profileLastValidationError: null,
    addedAt: now - 86400,
    updatedAt: now,
    usageError: null,
    authRefreshBlocked: false,
    authRefreshError: null,
    apiProxyEnabled: true,
    isCurrent: false,
    usage: {
      fetchedAt: now,
      planType: "pro",
      fiveHour: {
        usedPercent: fiveHour,
        windowSeconds: 18000,
        resetAt: now + 10800,
      },
      oneWeek: {
        usedPercent: week,
        windowSeconds: 604800,
        resetAt: now + 432000,
      },
      credits: null,
      resetCredits: {
        availableCount: 3,
        credits: [1, 2, 3].map((n) => ({
          grantedAt: now,
          expiresAt: now + n * 604800,
        })),
      },
    },
    ...extra,
  };
}

const freeExample = fixtureAccount("spent", "spent@example.com", 100, 100, {
  planType: "free",
});
freeExample.usage!.planType = "free";
freeExample.usage!.resetCredits = { availableCount: 0, credits: [] };
const cachedExample = fixtureAccount("cached", "cached@example.com", 54, 77, {
  usageError: "usage -> 401 Unauthorized: token is expired",
});
cachedExample.usage!.resetCredits = { availableCount: null, credits: [] };
const alternatePlanExample = fixtureAccount(
  "team-pro",
  "team@example.com",
  20,
  70,
  { accountKey: "team", email: "team@example.com" },
);
alternatePlanExample.usage!.resetCredits = {
  availableCount: 1,
  credits: alternatePlanExample.usage!.resetCredits!.credits.slice(0, 1),
};

export const accounts = [
  fixtureAccount("daily", "daily@example.com", 37, 63, { isCurrent: true }),
  fixtureAccount("studio", "studio@example.com", 5, 12),
  fixtureAccount("backup", "backup@example.com", 22, 41, { planType: "plus" }),
  fixtureAccount("team", "team@example.com", 14, 55, { planType: "team" }),
  alternatePlanExample,
  fixtureAccount("low", "low@example.com", 82, 93),
  freeExample,
  cachedExample,
  fixtureAccount("relay", "自定义 API", 0, 0, {
    sourceKind: "relay",
    planType: "api",
    email: null,
    apiBaseUrl: "https://api.example.com/v1",
    modelName: "gpt-5.4",
    balanceText: "$128.40",
    usage: null,
  }),
];

export const tokenUsage: CodexTokenUsageSnapshot = {
  updatedAt: now,
  sourcePathCount: 4,
  failedPathCount: 0,
  unresolvedForkCount: 0,
  unresolvedUsageEventCount: 0,
  eventCount: 30,
  last24h: totals(1642800),
  last3d: totals(5386000),
  last7d: totals(12430000),
  last30d: totals(42920000),
  latestSession: null,
};

export const analytics: CodexCostAnalyticsSnapshot = {
  updatedAt: now,
  pricingSource: "fixture",
  sourcePathCount: 4,
  failedPathCount: 0,
  unresolvedForkCount: 0,
  unresolvedUsageEventCount: 0,
  eventCount: 30,
  total: totals(42920000),
  totalCostUsd: 82.65,
  localTotalCostUsd: 82.65,
  last7d: totals(12430000),
  last7dCostUsd: 24.8,
  localLast7dCostUsd: 24.8,
  budgetPeriodCostUsd: 24.8,
  localBudgetPeriodCostUsd: 24.8,
  costSource: "local_estimate",
  costSourceUpdatedAt: now,
  costSourceError: null,
  weeklyBudgetUsd: 50,
  weeklyBudgetPercent: 49.6,
  weeklyBudgetAlert: "ok",
  projects: [
    {
      projectPath: "/preview/codex-tools",
      projectName: "codex-tools",
      sessionCount: 12,
      promptCount: 36,
      eventCount: 30,
      total: totals(24300000),
      costUsd: 46.9,
      lastAt: now,
    },
    {
      projectPath: "/preview/app",
      projectName: "workspace",
      sessionCount: 8,
      promptCount: 24,
      eventCount: 20,
      total: totals(18620000),
      costUsd: 35.75,
      lastAt: now,
    },
  ],
  sessions: [
    {
      sessionId: "preview-session-01",
      parentSessionId: null,
      projectPath: "/preview/codex-tools",
      projectName: "codex-tools",
      startedAt: now - 3600,
      updatedAt: now,
      durationSeconds: 3600,
      promptCount: 12,
      eventCount: 10,
      model: "gpt-5.4",
      total: totals(1800000),
      costUsd: 3.6,
      sourcePath: "/preview/session.jsonl",
    },
  ],
  heatmap: Array.from({ length: 168 }, (_, n) => ({
    weekday: Math.floor(n / 24),
    hour: n % 24,
    calls: n % 11 === 0 ? 12 : n % 5,
    tokens: (n % 11) * 30000,
    costUsd: (n % 11) * 0.1,
  })),
  topPrompts: [],
};
