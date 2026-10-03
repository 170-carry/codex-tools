import type { ApiProxyUsageMetric, ApiProxyUsageRange } from "../../types/app";
import type {
  ApiProxyUsageSeriesView,
  ApiProxyUsageHoverState,
} from "./usageTypes";
import {
  API_PROXY_USAGE_TOOLTIP_SIZE,
  API_PROXY_USAGE_TOOLTIP_GAP,
  API_PROXY_USAGE_CONTEXT_MENU_SIZE,
} from "./usageConstants";
import { formatUsageMetricValue, formatUsageTooltipTime } from "./usageFormat";
export function interpolateSeriesAtX(
  pointerX: number,
  series: ApiProxyUsageSeriesView,
) {
  if (series.points.length === 0) {
    return null;
  }

  if (series.points.length === 1) {
    const point = series.points[0];
    return {
      x: point.x,
      y: point.y,
      timestamp: point.timestamp,
      value: point.value,
    };
  }

  const first = series.points[0];
  const last = series.points[series.points.length - 1];

  if (pointerX <= first.x) {
    return {
      x: first.x,
      y: first.y,
      timestamp: first.timestamp,
      value: first.value,
    };
  }

  if (pointerX >= last.x) {
    return {
      x: last.x,
      y: last.y,
      timestamp: last.timestamp,
      value: last.value,
    };
  }

  for (const segment of series.curveSegments) {
    const { start, end } = segment;
    const minX = Math.min(start.x, end.x);
    const maxX = Math.max(start.x, end.x);

    if (pointerX < minX || pointerX > maxX) {
      continue;
    }

    const deltaX = end.x - start.x;
    const t = deltaX === 0 ? 0 : (pointerX - start.x) / deltaX;
    const oneMinusT = 1 - t;
    const y =
      oneMinusT ** 3 * start.y +
      3 * oneMinusT ** 2 * t * segment.cp1y +
      3 * oneMinusT * t ** 2 * segment.cp2y +
      t ** 3 * end.y;

    return {
      x: pointerX,
      y,
      timestamp: start.timestamp + (end.timestamp - start.timestamp) * t,
      value: start.value + (end.value - start.value) * t,
    };
  }

  return null;
}

export function resolveUsageBucketWindow(
  pointerTimestamp: number,
  chartStartTimestamp: number,
  chartEndTimestamp: number,
  bucketSeconds: number,
  series: ApiProxyUsageSeriesView,
) {
  if (series.bucketPoints.length === 0) {
    return null;
  }

  if (series.bucketPoints.length === 1) {
    const point = series.bucketPoints[0];
    return {
      point,
      bucketStartTimestamp: Math.max(chartStartTimestamp, point.timestamp),
      bucketEndTimestamp: chartEndTimestamp,
    };
  }

  for (let index = 0; index < series.bucketPoints.length; index += 1) {
    const point = series.bucketPoints[index];
    const nextPoint = series.bucketPoints[index + 1];
    const rawBucketStart = point.timestamp;
    const rawBucketEnd =
      nextPoint?.timestamp ?? point.timestamp + bucketSeconds;
    const bucketStartTimestamp = Math.max(chartStartTimestamp, rawBucketStart);
    const bucketEndTimestamp = Math.min(chartEndTimestamp, rawBucketEnd);

    if (
      pointerTimestamp >= bucketStartTimestamp &&
      (pointerTimestamp < bucketEndTimestamp ||
        index === series.bucketPoints.length - 1)
    ) {
      return {
        point,
        bucketStartTimestamp,
        bucketEndTimestamp,
      };
    }
  }

  const lastPoint = series.bucketPoints[series.bucketPoints.length - 1];
  return {
    point: lastPoint,
    bucketStartTimestamp: Math.max(chartStartTimestamp, lastPoint.timestamp),
    bucketEndTimestamp: chartEndTimestamp,
  };
}

export function clampTooltipPosition(
  anchorX: number,
  anchorY: number,
  frameWidth: number,
  frameHeight: number,
  tooltipSize: { width: number; height: number },
) {
  const prefersRight =
    anchorX + API_PROXY_USAGE_TOOLTIP_GAP + tooltipSize.width <=
    frameWidth - 12;
  const prefersBottom =
    anchorY + API_PROXY_USAGE_TOOLTIP_GAP + tooltipSize.height <=
    frameHeight - 12;

  const left = prefersRight
    ? anchorX + API_PROXY_USAGE_TOOLTIP_GAP
    : anchorX - API_PROXY_USAGE_TOOLTIP_GAP - tooltipSize.width;
  const top = prefersBottom
    ? anchorY + API_PROXY_USAGE_TOOLTIP_GAP
    : anchorY - API_PROXY_USAGE_TOOLTIP_GAP - tooltipSize.height;

  return {
    x: Math.max(12, Math.min(left, frameWidth - tooltipSize.width - 12)),
    y: Math.max(12, Math.min(top, frameHeight - tooltipSize.height - 12)),
  };
}

export function getUsageTooltipSize(entryCount: number) {
  return {
    width: API_PROXY_USAGE_TOOLTIP_SIZE.width,
    height: Math.min(250, 48 + Math.max(entryCount, 1) * 25),
  };
}

export function clampContextMenuPosition(
  pointerX: number,
  pointerY: number,
  containerWidth: number,
  containerHeight: number,
) {
  return {
    x: Math.max(
      8,
      Math.min(
        pointerX,
        containerWidth - API_PROXY_USAGE_CONTEXT_MENU_SIZE.width - 8,
      ),
    ),
    y: Math.max(
      8,
      Math.min(
        pointerY,
        containerHeight - API_PROXY_USAGE_CONTEXT_MENU_SIZE.height - 8,
      ),
    ),
  };
}

export function resolveUsageHoverState({
  clientX,
  clientY,
  frameRect,
  svgRect,
  chartWidth,
  chartHeight,
  margins,
  series,
  locale,
  range,
  startTimestamp,
  endTimestamp,
  rangeSeconds,
  bucketSeconds,
  metric,
  metricLabel,
}: {
  clientX: number;
  clientY: number;
  frameRect: DOMRect;
  svgRect: DOMRect;
  chartWidth: number;
  chartHeight: number;
  margins: { top: number; right: number; bottom: number; left: number };
  series: ApiProxyUsageSeriesView[];
  locale: string;
  range: ApiProxyUsageRange;
  startTimestamp: number;
  endTimestamp: number;
  rangeSeconds: number;
  bucketSeconds: number;
  metric: ApiProxyUsageMetric;
  metricLabel: string;
}): ApiProxyUsageHoverState | null {
  if (series.length === 0 || svgRect.width <= 0 || svgRect.height <= 0) {
    return null;
  }

  const scaleX = svgRect.width / chartWidth || 1;
  const scaleY = svgRect.height / chartHeight || 1;
  const relativeX = (clientX - svgRect.left) / scaleX;
  const relativeY = (clientY - svgRect.top) / scaleY;
  const plotLeft = margins.left;
  const plotRight = margins.left + (chartWidth - margins.left - margins.right);
  const plotTop = margins.top;
  const plotBottom = margins.top + (chartHeight - margins.top - margins.bottom);

  if (
    relativeX < plotLeft ||
    relativeX > plotRight ||
    relativeY < plotTop ||
    relativeY > plotBottom
  ) {
    return null;
  }

  const pointerTimestamp =
    startTimestamp +
    ((relativeX - plotLeft) / Math.max(plotRight - plotLeft, 1)) * rangeSeconds;
  const primaryBucket = resolveUsageBucketWindow(
    pointerTimestamp,
    startTimestamp,
    endTimestamp,
    bucketSeconds,
    series[0],
  );
  if (!primaryBucket) {
    return null;
  }

  const bucketStartX =
    plotLeft +
    ((primaryBucket.bucketStartTimestamp - startTimestamp) /
      Math.max(rangeSeconds, 1)) *
      (plotRight - plotLeft);
  const bucketEndX =
    plotLeft +
    ((primaryBucket.bucketEndTimestamp - startTimestamp) /
      Math.max(rangeSeconds, 1)) *
      (plotRight - plotLeft);
  const bucketCursorX = primaryBucket.point.x;

  const entries = series
    .map((item) => {
      const bucket = resolveUsageBucketWindow(
        pointerTimestamp,
        startTimestamp,
        endTimestamp,
        bucketSeconds,
        item,
      );
      if (!bucket) {
        return null;
      }
      const interpolated = interpolateSeriesAtX(bucket.point.x, item);
      if (!interpolated) {
        return null;
      }

      return {
        model: item.model,
        color: item.color,
        value: bucket.point.value,
        valueLabel: formatUsageMetricValue(bucket.point.value, locale, metric),
        pointX: interpolated.x,
        pointY: interpolated.y,
      };
    })
    .filter((entry): entry is NonNullable<typeof entry> => entry !== null)
    .sort(
      (left, right) =>
        right.value - left.value || left.model.localeCompare(right.model),
    );

  if (entries.length === 0) {
    return null;
  }

  const anchorX = clientX - frameRect.left;
  const anchorY = clientY - frameRect.top;
  const tooltipSize = getUsageTooltipSize(entries.length);
  const tooltipPosition = clampTooltipPosition(
    anchorX,
    anchorY,
    frameRect.width,
    frameRect.height,
    tooltipSize,
  );

  return {
    cursorX: bucketCursorX,
    cursorY: relativeY,
    tooltipX: tooltipPosition.x,
    tooltipY: tooltipPosition.y,
    bucketStartTimestamp: primaryBucket.bucketStartTimestamp,
    bucketEndTimestamp: primaryBucket.bucketEndTimestamp,
    bucketStartX,
    bucketEndX,
    timeLabel: formatUsageTooltipTime(
      locale,
      Math.round(primaryBucket.bucketStartTimestamp),
      Math.round(primaryBucket.bucketEndTimestamp),
      range,
    ),
    metricLabel,
    entries,
  };
}
