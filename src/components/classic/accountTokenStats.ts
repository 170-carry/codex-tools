import type {
  AccountSummary,
  CodexAccountTokenUsage,
  CodexTokenUsageSnapshot,
} from "../../types/app";
import { dateFormatter, numberFormatter } from "../../utils/intlFormatters";

const STATS_COPY = {
  "zh-CN": {
    tokens24h: "24h 用量",
    tps: "TPS",
    credits: "余额",
    tokenCardTitle: "Token 与速度",
    tokenCardHint: "按 Codex 会话的创建账号统计",
    last24h: "近 24 小时",
    last7d: "近 7 天",
    last30d: "近 30 天",
    avgTps: "平均输出",
    latestTps: "最近一次",
    tps7d: "7 天平均",
    responses: "响应次数",
    lastActive: "最近使用",
    noUsage: "暂无本地 Codex 用量记录",
    creditsUnlimited: "无限",
    creditsUnavailable: "未提供",
    creditsTitle: "Credits 余额",
    tpsTitle: (avg: string, latest: string, week: string) =>
      `近24h平均输出 ${avg} tok/s · 最近一次 ${latest} · 7天 ${week}`,
    tokenTitle: (week: string, month: string) => `近7天 ${week} · 近30天 ${month}`,
    resetAt: (value: string) => `${value} 重置`,
    minutesAgo: (value: number) => `${value} 分钟前`,
    hoursAgo: (value: number) => `${value} 小时前`,
    justNow: "刚刚",
  },
  en: {
    tokens24h: "24h",
    tps: "TPS",
    credits: "Credits",
    tokenCardTitle: "Tokens & speed",
    tokenCardHint: "Attributed by the account that created each Codex session",
    last24h: "Last 24h",
    last7d: "Last 7d",
    last30d: "Last 30d",
    avgTps: "Avg output",
    latestTps: "Latest",
    tps7d: "7d avg",
    responses: "Responses",
    lastActive: "Last used",
    noUsage: "No local Codex usage recorded yet",
    creditsUnlimited: "Unlimited",
    creditsUnavailable: "Not provided",
    creditsTitle: "Credits balance",
    tpsTitle: (avg: string, latest: string, week: string) =>
      `24h avg output ${avg} tok/s · latest ${latest} · 7d ${week}`,
    tokenTitle: (week: string, month: string) => `7d ${week} · 30d ${month}`,
    resetAt: (value: string) => `resets ${value}`,
    minutesAgo: (value: number) => `${value}m ago`,
    hoursAgo: (value: number) => `${value}h ago`,
    justNow: "just now",
  },
};

export type AccountStatsCopy = (typeof STATS_COPY)["en"];

export function getAccountStatsCopy(locale: string): AccountStatsCopy {
  return locale === "zh-CN" ? STATS_COPY["zh-CN"] : STATS_COPY.en;
}

export function indexAccountTokenUsage(
  snapshot: CodexTokenUsageSnapshot | null,
): Map<string, CodexAccountTokenUsage> {
  return new Map(
    (snapshot?.accounts ?? []).map((usage) => [usage.accountId, usage]),
  );
}

export function formatTps(value: number | null | undefined, locale: string) {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return "—";
  }
  return numberFormatter(locale, {
    maximumFractionDigits: value < 100 ? 1 : 0,
  }).format(value);
}

/** Credits come from the usage API for ChatGPT accounts and the provider balance for relays. */
export function formatCreditsBalance(
  account: AccountSummary,
  locale: string,
  copy: AccountStatsCopy,
): { value: string; empty: boolean } {
  if (account.sourceKind === "relay") {
    return account.balanceText
      ? { value: account.balanceText, empty: false }
      : { value: "—", empty: true };
  }
  const credits = account.usage?.credits;
  if (!credits) return { value: "—", empty: true };
  if (credits.unlimited) return { value: copy.creditsUnlimited, empty: false };
  const balance = Number.parseFloat(credits.balance ?? "");
  if (!Number.isFinite(balance)) {
    return { value: credits.balance?.trim() || "—", empty: true };
  }
  return {
    value: numberFormatter(locale, {
      maximumFractionDigits: balance < 100 ? 2 : 0,
    }).format(balance),
    empty: balance <= 0,
  };
}

export function formatLastActive(
  epochSec: number | null | undefined,
  locale: string,
  copy: AccountStatsCopy,
): string {
  if (!epochSec) return "—";
  const elapsed = Math.max(0, Date.now() / 1000 - epochSec);
  if (elapsed < 60) return copy.justNow;
  if (elapsed < 3600) return copy.minutesAgo(Math.floor(elapsed / 60));
  if (elapsed < 86400) return copy.hoursAgo(Math.floor(elapsed / 3600));
  return dateFormatter(locale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(epochSec * 1000));
}

/** Short reset label: a clock time within a day, otherwise a date; the title carries the full time. */
export function formatResetShort(
  epochSec: number | null | undefined,
  locale: string,
): string | null {
  if (!epochSec) return null;
  const date = new Date(epochSec * 1000);
  if (Number.isNaN(date.getTime())) return null;
  const withinDay = epochSec * 1000 - Date.now() < 24 * 60 * 60 * 1000;
  return dateFormatter(
    locale,
    withinDay
      ? { hour: "2-digit", minute: "2-digit" }
      : { month: "2-digit", day: "2-digit" },
  ).format(date);
}
