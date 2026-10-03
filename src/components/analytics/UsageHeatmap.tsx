import { tokenHeatmapLevel } from "../../utils/heatmapScale";
import type { CodexHourlyCostBucket } from "../../types/app";
import type { AnalyticsCopy } from "./types";
import { formatTokenCount } from "./formatting";

export function UsageHeatmap({
  buckets,
  locale,
  copy,
}: {
  buckets: CodexHourlyCostBucket[];
  locale: string;
  copy: Pick<AnalyticsCopy, "heatmapAriaLabel" | "heatmapTooltip">;
}) {
  const byKey = new Map(
    buckets.map((bucket) => [`${bucket.weekday}:${bucket.hour}`, bucket]),
  );
  const maxTokens = Math.max(...buckets.map((bucket) => bucket.tokens), 1);
  const weekdayFormatter = new Intl.DateTimeFormat(locale, {
    weekday: "short",
    timeZone: "UTC",
  });
  const weekdayLabels = Array.from({ length: 7 }, (_, weekday) =>
    weekdayFormatter.format(new Date(Date.UTC(2024, 0, 7 + weekday, 12))),
  );
  const hourLabels = Array.from({ length: 24 }, (_, hour) => hour);

  return (
    <div
      className="analyticsHeatmap"
      role="img"
      aria-label={copy.heatmapAriaLabel}
    >
      <div className="analyticsHeatmapHeader" aria-hidden="true">
        <span />
        {hourLabels.map((hour) => (
          <b key={hour}>{hour % 6 === 0 ? `${hour}:00` : ""}</b>
        ))}
      </div>
      {weekdayLabels.map((label, weekday) => (
        <div key={label} className="analyticsHeatmapRow">
          <span>{label}</span>
          {hourLabels.map((hour) => {
            const bucket = byKey.get(`${weekday}:${hour}`);
            const tokens = bucket?.tokens ?? 0;
            const level = tokenHeatmapLevel(tokens, maxTokens);
            const tooltip = copy.heatmapTooltip(
              label,
              `${hour}:00`,
              formatTokenCount(tokens, locale),
            );
            return (
              <i
                key={hour}
                className={`analyticsHeatmapCell level${level}`}
                data-tooltip={tooltip}
              />
            );
          })}
        </div>
      ))}
    </div>
  );
}
