import type { MessageCatalog } from "../i18n/catalog.ts";
import type { AccountSummary, Notice } from "../types/app.ts";
import {
  classifyUsageRefreshError,
  extractUsageRefreshStatusCode,
} from "./usageRefreshError.ts";

const reasonKeys = {
  timeout: "usageFailureTimeout",
  network: "usageFailureNetwork",
  authorization: "usageFailureAuthorization",
  rateLimited: "usageFailureRateLimited",
  server: "usageFailureServer",
  invalidResponse: "usageFailureInvalidResponse",
  unknown: "usageFailureUnknown",
} as const;

type UsageRefreshNoticeCopy = {
  accountsGrid: Pick<
    MessageCatalog["accountsGrid"],
    (typeof reasonKeys)[keyof typeof reasonKeys]
  >;
  notices: Pick<
    MessageCatalog["notices"],
    "usageRefreshed" | "usageRefreshPartial" | "usageRefreshAllFailed"
  >;
};

export function buildUsageRefreshNotice(
  accounts: readonly AccountSummary[],
  copy: UsageRefreshNoticeCopy,
): Notice {
  // Plan variants share one backend quota request. Relay accounts do not use it.
  const quotaAccounts = new Map<string, string | null>();
  for (const account of accounts) {
    if (account.sourceKind === "relay") continue;
    const key = account.accountKey || account.id;
    const error = account.usageError?.trim() || null;
    quotaAccounts.set(key, quotaAccounts.get(key) || error);
  }

  const errors = [...quotaAccounts.values()].filter(
    (error): error is string => error !== null,
  );
  if (errors.length === 0) {
    return { type: "ok", message: copy.notices.usageRefreshed };
  }

  const reasons = new Set(errors.map((error) => {
    const kind = classifyUsageRefreshError(error);
    const reason = copy.accountsGrid[reasonKeys[kind]];
    const code = extractUsageRefreshStatusCode(error, kind);
    return code === null ? reason : `${reason} (${code})`;
  }));
  const reason = [...reasons].join("; ");
  return {
    type: "error",
    message: errors.length === quotaAccounts.size
      ? copy.notices.usageRefreshAllFailed(errors.length, reason)
      : copy.notices.usageRefreshPartial(errors.length, quotaAccounts.size, reason),
  };
}
