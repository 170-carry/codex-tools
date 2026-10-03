import type {
  ApiProxyUsageSeriesView,
  ApiProxyUsageDimension,
} from "./usageTypes";
import {
  hashUsageModel,
  formatUsageKeySeriesLabel,
  pickUsageColor,
} from "./usageFormat";
import {
  clampUsageValue,
  buildMonotoneCurveSegments,
  buildSmoothPath,
  buildAreaPath,
} from "./usageCurves";
import type { ApiProxyUsageStats, ApiProxyUsageMetric } from "../../types/app";
export function buildUsageChartData({
  dimension,
  metric,
  margins,
  plotHeight,
  plotWidth,
  selectedRangeSeconds,
  stats,
}: {
  dimension: ApiProxyUsageDimension;
  metric: ApiProxyUsageMetric;
  margins: { left: number; top: number };
  plotHeight: number;
  plotWidth: number;
  selectedRangeSeconds: number;
  stats: ApiProxyUsageStats | null;
}) {
  const sourceSeries =
    dimension === "key"
      ? (stats?.keySeries ?? []).map((item) => ({
          model: formatUsageKeySeriesLabel(item.keyLabel, item.keyId),
          totalCalls: item.totalCalls,
          totalTokens: item.totalTokens,
          points: item.points,
        }))
      : (stats?.series ?? []);
  const ordered = [...sourceSeries].sort((left, right) =>
    left.model.localeCompare(right.model),
  );
  const latestPointTimestamp = ordered.reduce((max, item) => {
    const latestPoint = item.points.reduce(
      (pointMax, point) => Math.max(pointMax, point.timestamp),
      0,
    );
    return Math.max(max, latestPoint);
  }, 0);
  const endTimestamp = Math.max(stats?.updatedAt ?? 0, latestPointTimestamp);

  if (ordered.length === 0) {
    return {
      series: [] as ApiProxyUsageSeriesView[],
      maxValue: 0,
      endTimestamp,
    };
  }

  const startTimestamp = endTimestamp - selectedRangeSeconds;
  const baselineY = margins.top + plotHeight;
  const bucketSeconds = Math.max(
    1,
    stats?.bucketSeconds ?? selectedRangeSeconds,
  );

  const prepared = ordered.map((item, index) => {
    const color = pickUsageColor(index);
    const metricPoints = [...item.points]
      .sort((left, right) => left.timestamp - right.timestamp)
      .map((point) => ({
        timestamp: point.timestamp,
        value: metric === "calls" ? point.calls : point.tokens,
      }))
      .filter((point) => Number.isFinite(point.value));

    return {
      model: item.model,
      color,
      gradientId: `proxy-usage-gradient-${index}-${hashUsageModel(item.model)}`,
      totalCalls: item.totalCalls,
      totalTokens: item.totalTokens,
      totalValue: metric === "calls" ? item.totalCalls : item.totalTokens,
      metricPoints,
    };
  });

  let maxValue = 0;
  for (const item of prepared) {
    for (const point of item.metricPoints) {
      if (point.value > maxValue) {
        maxValue = point.value;
      }
    }
  }

  if (maxValue <= 0) {
    return {
      series: [] as ApiProxyUsageSeriesView[],
      maxValue: 0,
      endTimestamp,
    };
  }

  const yDomain = maxValue * 1.12;
  const series = prepared.map((item) => {
    const bucketPoints = item.metricPoints.map((point, index) => {
      const nextPoint = item.metricPoints[index + 1];
      const bucketStartTimestamp = clampUsageValue(
        point.timestamp,
        startTimestamp,
        endTimestamp,
      );
      const bucketEndTimestamp = clampUsageValue(
        nextPoint?.timestamp ?? endTimestamp,
        startTimestamp,
        endTimestamp,
      );
      const bucketMidTimestamp =
        bucketStartTimestamp + (bucketEndTimestamp - bucketStartTimestamp) / 2;
      const x =
        margins.left +
        ((bucketMidTimestamp - startTimestamp) /
          Math.max(selectedRangeSeconds, 1)) *
          plotWidth;
      const y = baselineY - (Math.max(0, point.value) / yDomain) * plotHeight;
      return {
        timestamp: point.timestamp,
        value: point.value,
        x: clampUsageValue(x, margins.left, margins.left + plotWidth),
        y: clampUsageValue(y, margins.top, baselineY),
      };
    });

    const firstBucket = bucketPoints[0];
    const lastBucket = bucketPoints[bucketPoints.length - 1];
    const pointValues =
      firstBucket && lastBucket
        ? [
            {
              timestamp: startTimestamp,
              value: firstBucket.value,
              x: margins.left,
              y: firstBucket.y,
            },
            ...bucketPoints,
            {
              timestamp: endTimestamp,
              value: lastBucket.value,
              x: margins.left + plotWidth,
              y: lastBucket.y,
            },
          ]
        : [];

    const points = pointValues;

    const curveSegments = buildMonotoneCurveSegments(points);
    const linePath = buildSmoothPath(points, curveSegments);
    const areaPath = buildAreaPath(points, baselineY, linePath);

    return {
      model: item.model,
      color: item.color,
      gradientId: item.gradientId,
      totalCalls: item.totalCalls,
      totalTokens: item.totalTokens,
      totalValue: item.totalValue,
      bucketPoints,
      points,
      curveSegments,
      linePath,
      areaPath,
    } satisfies ApiProxyUsageSeriesView;
  });

  return {
    series,
    bucketSeconds,
    maxValue,
    endTimestamp,
  };
}
