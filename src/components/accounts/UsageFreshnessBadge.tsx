import type { AccountSummary } from "../../types/app";
import {
  classifyUsageRefreshError,
  extractUsageRefreshStatusCode,
} from "../../utils/usageRefreshError";

type UsageFreshnessCopy = {
  usageRefreshing: string;
  usageRefreshingCached: (updatedAt: string) => string;
  usageRefreshFailed: (reason: string) => string;
  usageRefreshFailedCached: (reason: string, updatedAt: string) => string;
  usageFailureTimeout: string;
  usageFailureNetwork: string;
  usageFailureAuthorization: string;
  usageFailureRateLimited: string;
  usageFailureServer: string;
  usageFailureInvalidResponse: string;
  usageFailureUnknown: string;
  usageUnavailable: string;
};

type UsageFreshnessTone = "refreshing" | "error" | "unknown";

function summarizeUsageRefreshError(
  error: string,
  copy: UsageFreshnessCopy,
): string {
  const kind = classifyUsageRefreshError(error);
  let summary: string;
  switch (kind) {
    case "timeout":
      summary = copy.usageFailureTimeout;
      break;
    case "network":
      summary = copy.usageFailureNetwork;
      break;
    case "authorization":
      summary = copy.usageFailureAuthorization;
      break;
    case "rateLimited":
      summary = copy.usageFailureRateLimited;
      break;
    case "server":
      summary = copy.usageFailureServer;
      break;
    case "invalidResponse":
      summary = copy.usageFailureInvalidResponse;
      break;
    case "unknown":
      summary = copy.usageFailureUnknown;
      break;
  }

  const statusCode = extractUsageRefreshStatusCode(error, kind);
  return statusCode === null ? summary : `${summary} (${statusCode})`;
}

function formatUsageFetchedAt(epochSec: number | null | undefined, locale: string): string | null {
  if (!epochSec) {
    return null;
  }

  return new Date(epochSec * 1000).toLocaleString(locale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function UsageFreshnessBadge({
  account,
  refreshing,
  showInitialRefresh,
  refreshError,
  locale,
  copy,
  compact = false,
}: {
  account: AccountSummary;
  refreshing: boolean;
  showInitialRefresh: boolean;
  refreshError: string | null;
  locale: string;
  copy: UsageFreshnessCopy;
  compact?: boolean;
}) {
  const fetchedAt = formatUsageFetchedAt(account.usage?.fetchedAt, locale);
  const error = account.usageError || refreshError;
  let tone: UsageFreshnessTone = "unknown";
  let label = copy.usageUnavailable;

  if (showInitialRefresh && refreshing) {
    tone = "refreshing";
    label = fetchedAt
      ? copy.usageRefreshingCached(fetchedAt)
      : copy.usageRefreshing;
  } else if (error) {
    tone = "error";
    if (account.authRefreshBlocked) {
      label = error;
    } else {
      const reason = summarizeUsageRefreshError(error, copy);
      label = fetchedAt
        ? copy.usageRefreshFailedCached(reason, fetchedAt)
        : copy.usageRefreshFailed(reason);
    }
  } else if (fetchedAt) {
    return null;
  }

  return (
    <span
      className={`usageFreshnessBadge tone-${tone}`}
      title={error ? `${label}\n${error}` : label}
      aria-label={label}
    >
      <span className="usageFreshnessDot" aria-hidden="true" />
      <span>{compact && error ? `${summarizeUsageRefreshError(error, copy)}${fetchedAt ? (locale === "zh-CN" ? " · 缓存" : " · cached") : ""}` : compact && refreshing ? copy.usageRefreshing : label}</span>
    </span>
  );
}
