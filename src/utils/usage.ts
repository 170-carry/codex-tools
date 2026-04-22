import type { UsageWindow } from "../types/app";

export function percent(value: number | undefined | null): string {
  if (value === undefined || value === null || Number.isNaN(value)) {
    return "--";
  }
  return `${Math.max(0, Math.min(100, value)).toFixed(0)}%`;
}

export function remainingPercent(window: UsageWindow | null): number | null {
  if (!window) {
    return null;
  }
  return Math.max(0, Math.min(100, 100 - window.usedPercent));
}

export function toProgressWidth(value: number | undefined | null): string {
  if (value === undefined || value === null || Number.isNaN(value)) {
    return "0%";
  }
  const clamped = Math.max(0, Math.min(100, value));
  return `${clamped}%`;
}

export function formatPlan(
  plan: string | null | undefined,
  labels?: Record<string, string>,
): string {
  if (!plan) {
    return labels?.unknown ?? "Unknown";
  }
  const normalized = plan.trim().toLowerCase();
  if (!normalized) {
    return labels?.unknown ?? "Unknown";
  }
  if (labels?.[normalized]) {
    return labels[normalized];
  }
  if (normalized === "free") return "Free";
  if (normalized === "plus") return "Plus";
  if (normalized === "pro") return "Pro";
  if (normalized === "team") return "Team";
  if (normalized === "enterprise") return "Enterprise";
  if (normalized === "business") return "Business";
  if (normalized === "api") return "API";
  return normalized[0].toUpperCase() + normalized.slice(1);
}

export function planTone(plan: string | null | undefined): string {
  const normalized = plan?.trim().toLowerCase() ?? "";
  if (normalized === "team") return "team";
  if (normalized === "pro") return "pro";
  if (normalized === "plus") return "plus";
  if (normalized === "enterprise") return "enterprise";
  if (normalized === "business") return "business";
  if (normalized === "api") return "api";
  if (normalized === "free") return "free";
  return "unknown";
}

export function formatResetAt(epochSec: number | null | undefined, locale?: string): string {
  if (!epochSec) {
    return "--";
  }
  return new Date(epochSec * 1000).toLocaleString(locale);
}

type WindowLabelOptions = {
  fallback: string;
  oneWeek: string;
  hourSuffix: string;
  minuteSuffix: string;
};

export function formatWindowLabel(
  window: UsageWindow | null,
  { fallback, oneWeek, hourSuffix, minuteSuffix }: WindowLabelOptions,
): string {
  if (!window?.windowSeconds) {
    return fallback;
  }
  const hours = Math.round(window.windowSeconds / 3600);
  if (hours >= 24 * 7) {
    return oneWeek;
  }
  if (hours > 0) {
    return `${hours}${hourSuffix}`;
  }
  const mins = Math.round(window.windowSeconds / 60);
  return `${mins}${minuteSuffix}`;
}
