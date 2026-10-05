import { dateFormatter, numberFormatter } from "../../utils/intlFormatters.ts";
import type {
  CodexBudgetAlert,
  CodexCostAnalyticsProgress,
  CodexCostAnalyticsSnapshot,
} from "../../types/app";
import type { AnalyticsCopy } from "./types";

export function formatUsd(value: number, locale: string) {
  const digits = Math.abs(value) < 1 ? 4 : 2;
  return numberFormatter(locale, {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
}

export function formatNumber(value: number, locale: string) {
  return numberFormatter(locale, {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value);
}

export function formatWholeNumber(value: number, locale: string) {
  return numberFormatter(locale, {
    maximumFractionDigits: 0,
  }).format(value);
}

export function formatTokenCount(value: number, locale: string) {
  const absoluteValue = Math.abs(value);
  const scale =
    absoluteValue >= 999_950
      ? { divisor: 1_000_000, suffix: "M" }
      : absoluteValue >= 1_000
        ? { divisor: 1_000, suffix: "K" }
        : null;

  if (!scale) {
    return formatWholeNumber(value, locale);
  }

  const formatted = numberFormatter(locale, {
    maximumFractionDigits: 1,
  }).format(value / scale.divisor);
  return `${formatted}${scale.suffix}`;
}

export function formatDateTime(value: number | null, locale: string) {
  if (!value) {
    return "--";
  }
  return dateFormatter(locale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value * 1000));
}

export function formatDuration(seconds: number | null, locale: string) {
  if (seconds === null) {
    return "--";
  }
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours <= 0) {
    return numberFormatter(locale).format(minutes) + "m";
  }
  return `${numberFormatter(locale).format(hours)}h ${minutes}m`;
}

export function alertLabel(alert: CodexBudgetAlert, copy: AnalyticsCopy) {
  if (alert === "danger") {
    return copy.budgetDanger;
  }
  if (alert === "warning") {
    return copy.budgetWarning;
  }
  if (alert === "ok") {
    return copy.budgetOk;
  }
  return copy.budgetUnset;
}

export function progressStageLabel(
  progress: CodexCostAnalyticsProgress | null,
  copy: AnalyticsCopy,
) {
  if (progress?.stage === "caching") {
    return copy.progressCaching;
  }
  if (progress?.stage === "complete") {
    return copy.progressComplete;
  }
  return copy.progressScanning;
}

export function costSourceDetail(
  analytics: CodexCostAnalyticsSnapshot | null,
  copy: AnalyticsCopy,
  locale: string,
) {
  if (!analytics) {
    return { label: copy.pricingEstimate, title: undefined };
  }

  const updatedAt = formatDateTime(analytics.costSourceUpdatedAt, locale);
  return {
    label: `${copy.costSourceLocal} · ${updatedAt}`,
    title: undefined,
  };
}
