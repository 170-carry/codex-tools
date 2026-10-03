import type { MessageCatalog } from "../../i18n/catalog";
import type {
  ApiProxyKey,
  ApiProxyUsageMetric,
  ApiProxyUsageRange,
  ApiProxyUsageStats,
} from "../../types/app";
export type ApiProxyUsagePlotPoint = {
  timestamp: number;
  value: number;
  x: number;
  y: number;
};

export type ApiProxyUsageCurveSegment = {
  start: ApiProxyUsagePlotPoint;
  end: ApiProxyUsagePlotPoint;
  cp1x: number;
  cp1y: number;
  cp2x: number;
  cp2y: number;
};

export type ApiProxyUsageSeriesView = {
  model: string;
  color: string;
  gradientId: string;
  totalCalls: number;
  totalTokens: number;
  totalValue: number;
  bucketPoints: ApiProxyUsagePlotPoint[];
  points: ApiProxyUsagePlotPoint[];
  curveSegments: ApiProxyUsageCurveSegment[];
  linePath: string;
  areaPath: string;
};

export type ApiProxyUsageHoverState = {
  cursorX: number;
  cursorY: number;
  tooltipX: number;
  tooltipY: number;
  bucketStartTimestamp: number;
  bucketEndTimestamp: number;
  bucketStartX: number;
  bucketEndX: number;
  timeLabel: string;
  metricLabel: string;
  entries: Array<{
    model: string;
    color: string;
    value: number;
    valueLabel: string;
    pointX: number;
    pointY: number;
  }>;
};

export type ApiProxyUsageContextMenu = {
  x: number;
  y: number;
};

export type ApiProxyUsageChartMotion = {
  id: number;
  mode: "none" | "rise" | "slide";
  offset: number;
};

export type ApiProxyUsageDimension = "model" | "key";

export type ApiProxyUsageChartProps = {
  copy: MessageCatalog["apiProxy"];
  locale: string;
  stats: ApiProxyUsageStats | null;
  range: ApiProxyUsageRange;
  metric: ApiProxyUsageMetric;
  loading: boolean;
  clearing: boolean;
  exporting: boolean;
  proxyRunning: boolean;
  apiProxyKeys: ApiProxyKey[];
  onSelectRange: (range: ApiProxyUsageRange) => void;
  onSelectMetric: (metric: ApiProxyUsageMetric) => void;
  onExport: (keyId: string | null) => Promise<void> | void;
  onClear: () => void;
};
