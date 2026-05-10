import type { DashboardTimelineBucket, DashboardWindowStats } from "../types/app";

type TimelineAxisTick = {
  value: number;
  label: string;
};

type TimelineTimeLabel = {
  bucketIndex: number;
  startAt: number;
  label: string;
};

type TimelineChartBar = {
  x: number;
  height: number;
  failureHeight: number;
};

type TimelineChartGridLine = {
  y: number;
  label: string;
};

function formatAxisMs(value: number): string {
  return `${value}ms`;
}

function roundPercent(value: number): number {
  return Math.round(value * 100) / 100;
}

export function buildTimelineAxis(stats: DashboardWindowStats): {
  maxRequests: number;
  maxLatency: number;
  requestTicks: TimelineAxisTick[];
  latencyTicks: TimelineAxisTick[];
  timeLabels: TimelineTimeLabel[];
} {
  const maxRequests = Math.max(1, ...stats.timeline.map((bucket) => bucket.requestCount));
  const maxLatency = Math.max(1, ...stats.timeline.map((bucket) => bucket.totalP95Ms ?? 0));
  const requestMid = Math.round(maxRequests / 2);
  const latencyMid = Math.round(maxLatency / 2);

  return {
    maxRequests,
    maxLatency,
    requestTicks: [
      { value: maxRequests, label: String(maxRequests) },
      ...(requestMid > 0 && requestMid !== maxRequests ? [{ value: requestMid, label: String(requestMid) }] : []),
      { value: 0, label: "0" },
    ],
    latencyTicks: [
      { value: maxLatency, label: formatAxisMs(maxLatency) },
      ...(latencyMid > 0 && latencyMid !== maxLatency ? [{ value: latencyMid, label: formatAxisMs(latencyMid) }] : []),
      { value: 0, label: formatAxisMs(0) },
    ],
    timeLabels: buildTimelineTimeLabels(stats.timeline),
  };
}

export function buildTimelineChart(stats: DashboardWindowStats): {
  maxRequests: number;
  maxLatency: number;
  requestAxisLabel: string;
  latencyAxisLabel: string;
  requestTicks: TimelineAxisTick[];
  latencyTicks: TimelineAxisTick[];
  timeLabels: TimelineTimeLabel[];
  requestGridLines: TimelineChartGridLine[];
  bars: TimelineChartBar[];
  linePoints: string;
} {
  const axis = buildTimelineAxis(stats);
  const bucketCount = stats.timeline.length;

  return {
    ...axis,
    requestAxisLabel: "请求量",
    latencyAxisLabel: "P95 延迟",
    requestGridLines: axis.requestTicks.map((tick) => ({
      label: tick.label,
      y: roundPercent(100 - (tick.value / axis.maxRequests) * 100),
    })),
    bars: stats.timeline.map((bucket, index) => ({
      x: roundPercent(bucketCount <= 1 ? 50 : (index / (bucketCount - 1)) * 100),
      height: roundPercent((bucket.requestCount / axis.maxRequests) * 100),
      failureHeight:
        bucket.requestCount === 0
          ? 0
          : roundPercent((bucket.failureCount / bucket.requestCount) * 100),
    })),
    linePoints: stats.timeline
      .map((bucket, index) => {
        const x = bucketCount <= 1 ? 50 : (index / (bucketCount - 1)) * 100;
        const latency = bucket.totalP95Ms ?? 0;
        const y = 100 - (latency / axis.maxLatency) * 100;
        return `${x.toFixed(2)},${Math.max(0, Math.min(100, y)).toFixed(2)}`;
      })
      .join(" "),
  };
}

function buildTimelineTimeLabels(timeline: DashboardTimelineBucket[]): TimelineTimeLabel[] {
  if (timeline.length === 0) {
    return [];
  }

  const labelCount = Math.min(6, timeline.length);
  return Array.from({ length: labelCount }, (_, index) => {
    const bucketIndex = labelCount === 1 ? 0 : Math.round((index / (labelCount - 1)) * (timeline.length - 1));
    const bucket = timeline[bucketIndex];
    return {
      bucketIndex,
      startAt: bucket.startAt,
      label: new Date(bucket.startAt * 1000).toLocaleTimeString([], {
        hour: "2-digit",
        minute: "2-digit",
        hour12: false,
      }),
    };
  });
}
