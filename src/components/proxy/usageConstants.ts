import type { ApiProxyUsageRange } from "../../types/app";
export const API_PROXY_USAGE_PALETTE = [
  "var(--proxy-usage-series-1)",
  "var(--proxy-usage-series-2)",
  "var(--proxy-usage-series-3)",
  "var(--proxy-usage-series-4)",
  "var(--proxy-usage-series-5)",
  "var(--proxy-usage-series-6)",
  "var(--proxy-usage-series-7)",
  "var(--proxy-usage-series-8)",
  "var(--proxy-usage-series-9)",
  "var(--proxy-usage-series-10)",
  "var(--proxy-usage-series-11)",
  "var(--proxy-usage-series-12)",
] as const;

export const API_PROXY_USAGE_TOOLTIP_SIZE = { width: 256, height: 104 };

export const API_PROXY_USAGE_TOOLTIP_GAP = 12;

export const API_PROXY_USAGE_CONTEXT_MENU_SIZE = { width: 176, height: 44 };

export const API_PROXY_USAGE_RANGE_SECONDS: Record<ApiProxyUsageRange, number> =
  {
    "1h": 3_600,
    "24h": 86_400,
    "7d": 604_800,
    "14d": 1_209_600,
    "30d": 2_592_000,
  };
