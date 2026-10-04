import type { CodexCostAnalyticsSnapshot } from "../../types/app";
import type { AnalyticsCopy } from "./types";
import { formatNumber, formatUsd } from "./formatting";
import { useAppLayout } from "../../hooks/useAppLayout";

export function AnalyticsSummary({
  analytics,
  text,
  locale,
}: {
  analytics: CodexCostAnalyticsSnapshot | null;
  text: AnalyticsCopy;
  locale: string;
}) {
  const { layout } = useAppLayout();
  const items = [
    [
      text.totalCost,
      analytics ? formatUsd(analytics.totalCostUsd, locale) : "—",
    ],
    [
      text.last7dCost,
      analytics ? formatUsd(analytics.last7dCostUsd, locale) : "—",
    ],
    [
      text.totalTokens,
      analytics ? formatNumber(analytics.total.totalTokens, locale) : "—",
    ],
    [
      text.sessions,
      analytics ? formatNumber(analytics.sessions.length, locale) : "—",
    ],
  ];
  return (
    <section
      className={
        layout === "classic" ? "analyticsStats" : "analyticsSummaryRow"
      }
    >
      {items.map(([label, value]) => (
        <div
          key={label}
          className={layout === "classic" ? "analyticsStatCard" : undefined}
        >
          <span>{label}</span>
          <strong>{value}</strong>
        </div>
      ))}
    </section>
  );
}
