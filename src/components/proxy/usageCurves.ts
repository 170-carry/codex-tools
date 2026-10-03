import type { ApiProxyUsageRange } from "../../types/app";
import type {
  ApiProxyUsagePlotPoint,
  ApiProxyUsageCurveSegment,
} from "./usageTypes";
export function formatUsageTickLabel(
  locale: string,
  timestampSec: number,
  range: ApiProxyUsageRange,
) {
  const date = new Date(timestampSec * 1000);
  if (range === "1h" || range === "24h") {
    return new Intl.DateTimeFormat(locale, {
      hour: "2-digit",
      minute: "2-digit",
    }).format(date);
  }

  if (range === "7d") {
    return new Intl.DateTimeFormat(locale, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
    }).format(date);
  }

  return new Intl.DateTimeFormat(locale, {
    month: "short",
    day: "numeric",
  }).format(date);
}

export function clampUsageValue(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value));
}

export function normalizeCurvePoints(points: ApiProxyUsagePlotPoint[]) {
  const normalized: ApiProxyUsagePlotPoint[] = [];
  for (const point of points) {
    const previous = normalized[normalized.length - 1];
    if (previous && Math.abs(previous.x - point.x) < 0.001) {
      normalized[normalized.length - 1] = point;
    } else {
      normalized.push(point);
    }
  }

  return normalized;
}

export function buildMonotoneCurveSegments(points: ApiProxyUsagePlotPoint[]) {
  const normalized = normalizeCurvePoints(points);
  if (normalized.length < 2) {
    return [] as ApiProxyUsageCurveSegment[];
  }

  const segmentSlopes = normalized.slice(0, -1).map((point, index) => {
    const next = normalized[index + 1];
    const deltaX = next.x - point.x;
    return deltaX === 0 ? 0 : (next.y - point.y) / deltaX;
  });

  const tangents = normalized.map((_, index) => {
    if (index === 0) {
      return segmentSlopes[0] ?? 0;
    }
    if (index === normalized.length - 1) {
      return segmentSlopes[segmentSlopes.length - 1] ?? 0;
    }

    const previousSlope = segmentSlopes[index - 1];
    const nextSlope = segmentSlopes[index];
    if (
      previousSlope === 0 ||
      nextSlope === 0 ||
      Math.sign(previousSlope) !== Math.sign(nextSlope)
    ) {
      return 0;
    }

    return (previousSlope + nextSlope) / 2;
  });

  for (let index = 0; index < segmentSlopes.length; index += 1) {
    const slope = segmentSlopes[index];
    if (slope === 0) {
      tangents[index] = 0;
      tangents[index + 1] = 0;
      continue;
    }

    const alpha = tangents[index] / slope;
    const beta = tangents[index + 1] / slope;
    const distance = alpha ** 2 + beta ** 2;
    if (distance > 9) {
      const scale = 3 / Math.sqrt(distance);
      tangents[index] = scale * alpha * slope;
      tangents[index + 1] = scale * beta * slope;
    }
  }

  return normalized.slice(0, -1).map((start, index) => {
    const end = normalized[index + 1];
    const deltaX = end.x - start.x;
    const minSegmentY = Math.min(start.y, end.y);
    const maxSegmentY = Math.max(start.y, end.y);
    return {
      start,
      end,
      cp1x: start.x + deltaX / 3,
      cp1y: clampUsageValue(
        start.y + (tangents[index] * deltaX) / 3,
        minSegmentY,
        maxSegmentY,
      ),
      cp2x: end.x - deltaX / 3,
      cp2y: clampUsageValue(
        end.y - (tangents[index + 1] * deltaX) / 3,
        minSegmentY,
        maxSegmentY,
      ),
    } satisfies ApiProxyUsageCurveSegment;
  });
}

export function buildSmoothPath(
  points: ApiProxyUsagePlotPoint[],
  segments: ApiProxyUsageCurveSegment[],
) {
  if (points.length === 0) {
    return "";
  }

  if (points.length === 1) {
    return `M ${points[0].x} ${points[0].y}`;
  }

  const first = segments[0]?.start ?? points[0];
  let path = `M ${first.x} ${first.y}`;
  for (const segment of segments) {
    path += ` C ${segment.cp1x} ${segment.cp1y}, ${segment.cp2x} ${segment.cp2y}, ${segment.end.x} ${segment.end.y}`;
  }

  return path;
}

export function buildAreaPath(
  points: ApiProxyUsagePlotPoint[],
  baselineY: number,
  linePath: string,
) {
  if (points.length === 0) {
    return "";
  }

  if (points.length === 1) {
    const point = points[0];
    return `M ${point.x} ${baselineY} L ${point.x} ${point.y} L ${point.x} ${baselineY} Z`;
  }

  const first = points[0];
  const last = points[points.length - 1];
  return `${linePath} L ${last.x} ${baselineY} L ${first.x} ${baselineY} Z`;
}
