import { dateFormatter, numberFormatter } from "../../utils/intlFormatters.ts";
import type { ApiProxyUsageMetric, ApiProxyUsageRange } from "../../types/app";
import { API_PROXY_USAGE_PALETTE } from "./usageConstants";
export function hashUsageModel(model: string) {
  let hash = 0;
  for (let index = 0; index < model.length; index += 1) {
    hash = (Math.imul(31, hash) + model.charCodeAt(index)) >>> 0;
  }
  return hash;
}

export function formatUsageKeySeriesLabel(label: string, keyId: string) {
  const normalizedLabel = label.trim() || "Unnamed key";
  const shortId = keyId.trim().slice(0, 8);
  return shortId ? `${normalizedLabel} (${shortId})` : normalizedLabel;
}

export function normalizeDisabledProxyModels(
  disabledModels: string[],
  supportedModels: string[],
) {
  const disabledSet = new Set(disabledModels);
  return supportedModels.filter((model) => disabledSet.has(model));
}

export function pickUsageColor(index: number) {
  return API_PROXY_USAGE_PALETTE[index % API_PROXY_USAGE_PALETTE.length];
}

export function formatUsageMetricValue(
  value: number | undefined | null,
  locale: string,
  metric?: ApiProxyUsageMetric,
) {
  if (value === undefined || value === null || Number.isNaN(value)) {
    return "--";
  }

  const normalized = Math.max(0, Math.round(value));
  void metric;
  return numberFormatter(locale, {
    maximumFractionDigits: 0,
    useGrouping: true,
  }).format(normalized);
}

export function formatUsageAxisValue(value: number | undefined | null) {
  if (value === undefined || value === null || Number.isNaN(value)) {
    return "--";
  }

  const normalized = Math.max(0, value);
  if (normalized >= 1_000_000_000_000) {
    return `${(normalized / 1_000_000_000_000).toFixed(1)}T`;
  }
  if (normalized >= 1_000_000) {
    return `${(normalized / 1_000_000).toFixed(1)}M`;
  }
  if (normalized >= 1_000) {
    return `${(normalized / 1_000).toFixed(1)}k`;
  }

  return String(Math.round(normalized));
}

export function usageTooltipDateTimeOptions(
  range: ApiProxyUsageRange,
): Intl.DateTimeFormatOptions {
  const options: Intl.DateTimeFormatOptions = {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  };

  if (range === "1h") {
    options.second = "2-digit";
  }

  if (range === "14d" || range === "30d") {
    delete options.hour;
    delete options.minute;
    delete options.second;
  }

  return options;
}

export function formatUsageTooltipTime(
  locale: string,
  bucketStartSec: number,
  bucketEndSec: number,
  range: ApiProxyUsageRange,
) {
  const startDate = new Date(bucketStartSec * 1000);
  const endDate = new Date(bucketEndSec * 1000);

  try {
    const formatter = dateFormatter(
      locale,
      usageTooltipDateTimeOptions(range),
    );
    const startLabel = formatter.format(startDate);
    const endLabel = formatter.format(endDate);
    return startLabel === endLabel ? startLabel : `${startLabel} - ${endLabel}`;
  } catch {
    return `${startDate.toLocaleString(locale)} - ${endDate.toLocaleString(locale)}`;
  }
}
