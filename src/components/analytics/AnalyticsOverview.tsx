import { uiCopy } from "../../i18n/uiCopy";
import { useMemo, useState } from "react";
import type { CodexCostAnalyticsSnapshot } from "../../types/app";
import { formatNumber, formatUsd } from "./formatting";
import { formatTokenCount } from "../../utils/usage";
import type { AnalyticsCopy } from "./types";

import { useChartWidth } from "./useChartWidth";

type Metric = "costUsd" | "tokens";
export function AnalyticsOverview({
  analytics,
  text,
  locale,
}: {
  analytics: CodexCostAnalyticsSnapshot | null;
  text: AnalyticsCopy;
  locale: string;
}) {
  const { ref: chartRef, width: chartWidth } = useChartWidth();
  const step = (chartWidth - 80) / 24;
  const ui = uiCopy(locale);
  const [metric, setMetric] = useState<Metric>("tokens");
  const [weekday, setWeekday] = useState("all");
  const hours = useMemo(() => {
    const values = Array.from({ length: 24 }, () => 0);
    for (const bucket of analytics?.heatmap ?? []) {
      if (weekday === "all" || bucket.weekday === Number(weekday))
        values[bucket.hour] += bucket[metric];
    }
    return values;
  }, [analytics?.heatmap, metric, weekday]);
  const peak = Math.max(...hours, 1);
  const valueText = (value: number) =>
    metric === "costUsd"
      ? formatUsd(value, locale)
      : formatTokenCount(value, locale);
  const metrics: { id: Metric; label: string; value: string }[] = [
    {
      id: "costUsd",
      label: ui.estimatedCost,
      value: analytics ? formatUsd(analytics.totalCostUsd, locale) : "—",
    },
    {
      id: "tokens",
      label: ui.totalTokens,
      value: analytics
        ? formatTokenCount(analytics.total.totalTokens, locale)
        : "—",
    },
  ];
  const selectedLabel = metrics.find((item) => item.id === metric)?.label;
  const weekdayFormatter = new Intl.DateTimeFormat(locale, {
    weekday: "long",
    timeZone: "UTC",
  });
  return (
    <section className="analyticsOverview">
      <div className="uiDataScope">{ui.source}: {ui.allTime}</div>
      <div className="analyticsMetrics">
        {metrics.map((item) => (
          <div key={item.id} className="analyticsMetric">
            <span title={item.id === "costUsd" ? text.totalCost : undefined}>{item.label}</span>
            <strong>{item.value}</strong>
          </div>
        ))}
        <div className="analyticsMetric analyticsMetricStatic">
          <span>{ui.sessions}</span>
          <strong>
            {analytics ? formatNumber(analytics.sessions.length, locale) : "—"}
          </strong>
        </div>
      </div>
      <div className="analyticsChartToolbar">
        <div>
          <h3 title={ui.chartScope}>
            {ui.hourly}
          </h3>
          <p className="uiCaption" title={ui.chartScope}>{ui.hourlyScope}</p>
        </div>
        <div className="analyticsChartControls">
          <div className="modeGroup" aria-label={ui.statistics}>
            <button type="button" className={metric === "tokens" ? "primary" : "ghost"} aria-pressed={metric === "tokens"} onClick={() => setMetric("tokens")}>{ui.tokenMetric}</button>
            <button type="button" className={metric === "costUsd" ? "primary" : "ghost"} aria-pressed={metric === "costUsd"} onClick={() => setMetric("costUsd")}>{ui.costMetric}</button>
          </div>
        <select
          aria-label={ui.weekday}
          value={weekday}
          onChange={(event) => setWeekday(event.target.value)}
        >
          <option value="all">{ui.allWeekdays}</option>
          {Array.from({ length: 7 }, (_, day) => (
            <option key={day} value={day}>
              {weekdayFormatter.format(new Date(Date.UTC(2024, 0, 7 + day)))}
            </option>
          ))}
        </select>
        </div>
      </div>
      <div className="analyticsHourlyChart" ref={chartRef}>
        <svg
          viewBox={`0 0 ${chartWidth} 236`}
          role="img"
          aria-label={`${selectedLabel} · ${ui.hourly}`}
        >
          {[0, 0.5, 1].map((ratio) => (
            <g key={ratio}>
              <line
                x1="68"
                x2={chartWidth - 12}
                y1={190 - ratio * 166}
                y2={190 - ratio * 166}
                className="chartGrid"
              />
              <text x="56" y={194 - ratio * 166} textAnchor="end">
                {valueText(peak * ratio)}
              </text>
            </g>
          ))}
          {hours.map((value, hour) => (
            <g key={hour}>
              <rect
                x={72 + hour * step}
                y={190 - (value / peak) * 166}
                width={step * 0.72}
                height={(value / peak) * 166}
                rx="3"
                className="chartBar"
              >
                <title>{`${String(hour).padStart(2, "0")}:00 · ${valueText(value)}`}</title>
              </rect>
              {hour % (chartWidth < 650 ? 6 : 3) === 0 ? (
                <text x={72 + (hour + 0.36) * step} y="216" textAnchor="middle">
                  {String(hour).padStart(2, "0")}:00
                </text>
              ) : null}
            </g>
          ))}
        </svg>
        {!hours.some(Boolean) ? (
          <p className="chartEmpty">
            {ui.noUsage}
          </p>
        ) : null}
      </div>
    </section>
  );
}
