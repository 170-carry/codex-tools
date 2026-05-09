import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type CSSProperties,
} from "react";

import { useI18n } from "../i18n/I18nProvider";
import type { MessageCatalog } from "../i18n/catalog";
import { EditorMultiSelect, type MultiSelectOption } from "./EditorMultiSelect";
import type {
  ApiProxyStatus,
  ApiProxyUsageMetric,
  ApiProxyUsageRange,
  ApiProxyUsageStats,
  CloudflaredStatus,
  CloudflaredTunnelMode,
  ApiProxyLoadBalanceMode,
  RemoteAuthMode,
  RemoteProxyStatus,
  RemoteServerConfig,
  StartCloudflaredTunnelInput,
} from "../types/app";

const DEFAULT_PROXY_PORT = "8787";
const DEFAULT_REMOTE_SSH_PORT = "22";
const DEFAULT_REMOTE_LISTEN_PORT = "8787";
const REMOTE_DRAFTS_CACHE_KEY = "codex-tools:proxy-remote-drafts";
const REMOTE_EXPANDED_CACHE_KEY = "codex-tools:proxy-remote-expanded-id";
const REMOTE_SELECTED_CACHE_KEY = "codex-tools:proxy-remote-selected-id";
const REMOTE_HISTORY_CACHE_KEY = "codex-tools:proxy-remote-history";

type RemoteServerDraft = {
  id: string;
  label: string;
  host: string;
  sshPort: string;
  sshUser: string;
  authMode: RemoteAuthMode;
  identityFile: string;
  privateKey: string;
  password: string;
  remoteDir: string;
  listenPort: string;
};

type ApiProxyPanelProps = {
  status: ApiProxyStatus;
  apiProxyUsageStats: ApiProxyUsageStats | null;
  apiProxyUsageRange: ApiProxyUsageRange;
  apiProxyUsageMetric: ApiProxyUsageMetric;
  apiProxyUsageLoading: boolean;
  apiProxyUsageClearing: boolean;
  cloudflaredStatus: CloudflaredStatus;
  accountCount: number;
  autoStartEnabled: boolean;
  savedPort: number;
  loadBalanceMode: ApiProxyLoadBalanceMode;
  sequentialFiveHourLimitPercent: number;
  remoteServers: RemoteServerConfig[];
  remoteStatuses: Record<string, RemoteProxyStatus>;
  remoteLogs: Record<string, string>;
  savingSettings: boolean;
  starting: boolean;
  stopping: boolean;
  refreshingApiKey: boolean;
  refreshingRemoteId: string | null;
  deployingRemoteId: string | null;
  startingRemoteId: string | null;
  stoppingRemoteId: string | null;
  readingRemoteLogsId: string | null;
  installingDependencyName: string | null;
  installingDependencyTargetId: string | null;
  installingCloudflared: boolean;
  startingCloudflared: boolean;
  stoppingCloudflared: boolean;
  onStart: (port: number | null) => Promise<void> | void;
  onStop: () => void;
  onSelectApiProxyUsageRange: (range: ApiProxyUsageRange) => void;
  onSelectApiProxyUsageMetric: (metric: ApiProxyUsageMetric) => void;
  onClearApiProxyUsageStats: () => void;
  onRefreshApiKey: () => void;
  onRefresh: () => void;
  onToggleAutoStart: (enabled: boolean) => void;
  onPersistPort: (port: number) => Promise<void> | void;
  onUpdateLoadBalanceMode: (mode: ApiProxyLoadBalanceMode) => Promise<void> | void;
  onUpdateSequentialFiveHourLimitPercent: (percent: number) => Promise<void> | void;
  onUpdateRemoteServers: (servers: RemoteServerConfig[]) => void;
  onRefreshRemoteStatus: (server: RemoteServerConfig) => void;
  onDeployRemote: (server: RemoteServerConfig) => void;
  onStartRemote: (server: RemoteServerConfig) => void;
  onStopRemote: (server: RemoteServerConfig) => void;
  onReadRemoteLogs: (server: RemoteServerConfig) => void;
  onPickLocalIdentityFile: () => Promise<string | null>;
  onRefreshCloudflared: () => void;
  onInstallCloudflared: () => void;
  onStartCloudflared: (input: StartCloudflaredTunnelInput) => void;
  onStopCloudflared: () => void;
};

function copyText(value: string | null) {
  if (!value) {
    return;
  }
  void navigator.clipboard?.writeText(value).catch(() => {});
}

function createRemoteServerId() {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `remote-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function createRemoteDraft(): RemoteServerDraft {
  return {
    id: createRemoteServerId(),
    label: "",
    host: "",
    sshPort: DEFAULT_REMOTE_SSH_PORT,
    sshUser: "root",
    authMode: "keyPath",
    identityFile: "",
    privateKey: "",
    password: "",
    remoteDir: "/opt/codex-tools",
    listenPort: DEFAULT_REMOTE_LISTEN_PORT,
  };
}

function configToDraft(server: RemoteServerConfig): RemoteServerDraft {
  return {
    id: server.id,
    label: server.label,
    host: server.host,
    sshPort: String(server.sshPort),
    sshUser: server.sshUser,
    authMode: server.authMode,
    identityFile: server.identityFile ?? "",
    privateKey: server.privateKey ?? "",
    password: server.password ?? "",
    remoteDir: server.remoteDir,
    listenPort: String(server.listenPort),
  };
}

function draftToConfig(draft: RemoteServerDraft): RemoteServerConfig {
  return {
    id: draft.id,
    label: draft.label.trim(),
    host: draft.host.trim(),
    sshPort: Number.parseInt(draft.sshPort, 10) || 0,
    sshUser: draft.sshUser.trim(),
    authMode: draft.authMode,
    identityFile: draft.identityFile.trim() || null,
    privateKey: draft.privateKey.trim() || null,
    password: draft.password.trim() || null,
    remoteDir: draft.remoteDir.trim(),
    listenPort: Number.parseInt(draft.listenPort, 10) || 0,
  };
}

function buildRemoteBaseUrl(draft: RemoteServerDraft) {
  const host = draft.host.trim();
  const port = draft.listenPort.trim();
  if (!host || !port) {
    return "--";
  }
  return `http://${host}:${port}/v1`;
}

function readStorageValue(key: string, scope: "session" | "local" = "session") {
  if (typeof window === "undefined") {
    return null;
  }
  try {
    return (scope === "local" ? window.localStorage : window.sessionStorage).getItem(key);
  } catch {
    return null;
  }
}

function writeStorageValue(
  key: string,
  value: string | null,
  scope: "session" | "local" = "session",
) {
  if (typeof window === "undefined") {
    return;
  }
  try {
    const storage = scope === "local" ? window.localStorage : window.sessionStorage;
    if (value === null) {
      storage.removeItem(key);
    } else {
      storage.setItem(key, value);
    }
  } catch {
    // Ignore storage failures in constrained environments.
  }
}

function readCachedRemoteDrafts(remoteServers: RemoteServerConfig[]) {
  const cached = readStorageValue(REMOTE_DRAFTS_CACHE_KEY);
  if (!cached) {
    return remoteServers.map(configToDraft);
  }

  try {
    const parsed = JSON.parse(cached);
    if (!Array.isArray(parsed)) {
      return remoteServers.map(configToDraft);
    }

    return parsed
      .map((item) => {
        if (!item || typeof item !== "object") {
          return null;
        }

        const raw = item as Partial<Record<keyof RemoteServerDraft, unknown>>;
        const authMode =
          raw.authMode === "keyContent" ||
          raw.authMode === "keyFile" ||
          raw.authMode === "keyPath" ||
          raw.authMode === "password"
            ? raw.authMode
            : "keyPath";

        return {
          id: typeof raw.id === "string" && raw.id ? raw.id : createRemoteServerId(),
          label: typeof raw.label === "string" ? raw.label : "",
          host: typeof raw.host === "string" ? raw.host : "",
          sshPort: typeof raw.sshPort === "string" ? raw.sshPort : DEFAULT_REMOTE_SSH_PORT,
          sshUser: typeof raw.sshUser === "string" ? raw.sshUser : "root",
          authMode,
          identityFile: typeof raw.identityFile === "string" ? raw.identityFile : "",
          privateKey: typeof raw.privateKey === "string" ? raw.privateKey : "",
          password: typeof raw.password === "string" ? raw.password : "",
          remoteDir: typeof raw.remoteDir === "string" ? raw.remoteDir : "/opt/codex-tools",
          listenPort:
            typeof raw.listenPort === "string" ? raw.listenPort : DEFAULT_REMOTE_LISTEN_PORT,
        } satisfies RemoteServerDraft;
      })
      .filter((item): item is RemoteServerDraft => item !== null);
  } catch {
    return remoteServers.map(configToDraft);
  }
}

function readCachedEditingRemoteId(remoteServers: RemoteServerConfig[]) {
  const drafts = readCachedRemoteDrafts(remoteServers);
  const cached = readStorageValue(REMOTE_EXPANDED_CACHE_KEY);
  if (cached && drafts.some((draft) => draft.id === cached)) {
    return cached;
  }
  return null;
}

function readCachedSelectedRemoteId(remoteServers: RemoteServerConfig[]) {
  const drafts = readCachedRemoteDrafts(remoteServers);
  const cached = readStorageValue(REMOTE_SELECTED_CACHE_KEY, "local");
  if (cached && drafts.some((draft) => draft.id === cached)) {
    return cached;
  }
  return drafts[0]?.id ?? null;
}

function readCachedRemoteHistory(remoteServers: RemoteServerConfig[]) {
  const activeIds = new Set(remoteServers.map((server) => server.id));
  const cached = readStorageValue(REMOTE_HISTORY_CACHE_KEY, "local");
  if (!cached) {
    return {} as Record<string, number>;
  }

  try {
    const parsed = JSON.parse(cached);
    if (!parsed || typeof parsed !== "object") {
      return {} as Record<string, number>;
    }

    const next: Record<string, number> = {};
    for (const [id, value] of Object.entries(parsed)) {
      if (activeIds.has(id) && typeof value === "number" && Number.isFinite(value) && value > 0) {
        next[id] = value;
      }
    }
    return next;
  } catch {
    return {} as Record<string, number>;
  }
}

function isRemoteDraftConfigured(draft: RemoteServerDraft) {
  const sshPort = Number.parseInt(draft.sshPort, 10);
  const listenPort = Number.parseInt(draft.listenPort, 10);

  if (
    !draft.label.trim() ||
    !draft.host.trim() ||
    !draft.sshUser.trim() ||
    !draft.remoteDir.trim() ||
    !Number.isInteger(sshPort) ||
    sshPort <= 0 ||
    !Number.isInteger(listenPort) ||
    listenPort <= 0
  ) {
    return false;
  }

  if (draft.authMode === "keyContent") {
    return draft.privateKey.trim() !== "";
  }
  if (draft.authMode === "password") {
    return draft.password.trim() !== "";
  }
  return draft.identityFile.trim() !== "";
}

function formatRemoteHistoryTime(locale: string, timestamp: number) {
  try {
    return new Intl.DateTimeFormat(locale, {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    }).format(timestamp);
  } catch {
    return new Date(timestamp).toLocaleString();
  }
}

const REMOTE_AUTH_OPTIONS: MultiSelectOption<RemoteAuthMode>[] = [
  { id: "keyContent", label: "keyContent" },
  { id: "keyFile", label: "keyFile" },
  { id: "keyPath", label: "keyPath" },
  { id: "password", label: "password" },
];

type ApiProxyUsagePlotPoint = {
  timestamp: number;
  value: number;
  x: number;
  y: number;
};

type ApiProxyUsageCurveSegment = {
  start: ApiProxyUsagePlotPoint;
  end: ApiProxyUsagePlotPoint;
  cp1x: number;
  cp1y: number;
  cp2x: number;
  cp2y: number;
};

type ApiProxyUsageSeriesView = {
  model: string;
  color: string;
  gradientId: string;
  totalCalls: number;
  totalTokens: number;
  totalValue: number;
  points: ApiProxyUsagePlotPoint[];
  curveSegments: ApiProxyUsageCurveSegment[];
  linePath: string;
  areaPath: string;
};

type ApiProxyUsageHoverState = {
  cursorX: number;
  cursorY: number;
  tooltipX: number;
  tooltipY: number;
  timestamp: number;
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

type ApiProxyUsageContextMenu = {
  x: number;
  y: number;
};

type ApiProxyUsageChartMotion = {
  id: number;
  mode: "none" | "rise" | "slide";
  offset: number;
};

type ApiProxyUsageChartProps = {
  copy: MessageCatalog["apiProxy"];
  locale: string;
  stats: ApiProxyUsageStats | null;
  range: ApiProxyUsageRange;
  metric: ApiProxyUsageMetric;
  loading: boolean;
  clearing: boolean;
  proxyRunning: boolean;
  onSelectRange: (range: ApiProxyUsageRange) => void;
  onSelectMetric: (metric: ApiProxyUsageMetric) => void;
  onClear: () => void;
};

const API_PROXY_USAGE_PALETTE = [
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

const API_PROXY_USAGE_TOOLTIP_SIZE = { width: 256, height: 104 };
const API_PROXY_USAGE_TOOLTIP_GAP = 12;
const API_PROXY_USAGE_CONTEXT_MENU_SIZE = { width: 176, height: 44 };
const API_PROXY_USAGE_RANGE_SECONDS: Record<ApiProxyUsageRange, number> = {
  "1h": 3_600,
  "24h": 86_400,
  "7d": 604_800,
  "14d": 1_209_600,
  "30d": 2_592_000,
};

function hashUsageModel(model: string) {
  let hash = 0;
  for (let index = 0; index < model.length; index += 1) {
    hash = (Math.imul(31, hash) + model.charCodeAt(index)) >>> 0;
  }
  return hash;
}

function pickUsageColor(index: number) {
  return API_PROXY_USAGE_PALETTE[index % API_PROXY_USAGE_PALETTE.length];
}

function formatUsageMetricValue(
  value: number | undefined | null,
  locale: string,
  metric?: ApiProxyUsageMetric,
) {
  if (value === undefined || value === null || Number.isNaN(value)) {
    return "--";
  }

  const normalized = Math.max(0, Math.round(value));
  void metric;
  return new Intl.NumberFormat(locale, {
    maximumFractionDigits: 0,
    useGrouping: true,
  }).format(normalized);
}

function formatUsageAxisValue(value: number | undefined | null) {
  if (value === undefined || value === null || Number.isNaN(value)) {
    return "--";
  }

  const normalized = Math.max(0, value);
  if (normalized >= 1_000_000_000_000) {
    return `${(normalized / 1_000_000_000_000).toFixed(1)}T`;
  }
  if (normalized >= 1_000_000) {
    return `${(normalized / 1_000_000).toFixed(1)}M`;
  }
  if (normalized >= 1_000) {
    return `${(normalized / 1_000).toFixed(1)}k`;
  }

  return String(Math.round(normalized));
}

function formatUsageTooltipTime(locale: string, timestampSec: number, range: ApiProxyUsageRange) {
  const date = new Date(timestampSec * 1000);

  try {
    const options: Intl.DateTimeFormatOptions = {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    };

    if (range === "1h") {
      options.second = "2-digit";
    }

    return new Intl.DateTimeFormat(locale, options).format(date);
  } catch {
    return date.toLocaleString(locale);
  }
}

function interpolateSeriesAtX(
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

function clampTooltipPosition(
  anchorX: number,
  anchorY: number,
  frameWidth: number,
  frameHeight: number,
  tooltipSize: { width: number; height: number },
) {
  const prefersRight = anchorX + API_PROXY_USAGE_TOOLTIP_GAP + tooltipSize.width <= frameWidth - 12;
  const prefersBottom =
    anchorY + API_PROXY_USAGE_TOOLTIP_GAP + tooltipSize.height <= frameHeight - 12;

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

function getUsageTooltipSize(entryCount: number) {
  return {
    width: API_PROXY_USAGE_TOOLTIP_SIZE.width,
    height: Math.min(250, 48 + Math.max(entryCount, 1) * 25),
  };
}

function clampContextMenuPosition(
  pointerX: number,
  pointerY: number,
  containerWidth: number,
  containerHeight: number,
) {
  return {
    x: Math.max(8, Math.min(pointerX, containerWidth - API_PROXY_USAGE_CONTEXT_MENU_SIZE.width - 8)),
    y: Math.max(8, Math.min(pointerY, containerHeight - API_PROXY_USAGE_CONTEXT_MENU_SIZE.height - 8)),
  };
}

function resolveUsageHoverState({
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
  rangeSeconds,
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
  rangeSeconds: number;
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

  const entries = series
    .map((item) => {
      const interpolated = interpolateSeriesAtX(relativeX, item);
      if (!interpolated) {
        return null;
      }

      return {
        model: item.model,
        color: item.color,
        value: interpolated.value,
        valueLabel: formatUsageMetricValue(interpolated.value, locale, metric),
        pointX: interpolated.x,
        pointY: interpolated.y,
        timestamp: interpolated.timestamp,
      };
    })
    .filter((entry): entry is NonNullable<typeof entry> => entry !== null)
    .sort((left, right) => right.value - left.value || left.model.localeCompare(right.model));

  if (entries.length === 0) {
    return null;
  }

  const timestamp = startTimestamp + ((relativeX - plotLeft) / Math.max(plotRight - plotLeft, 1)) * rangeSeconds;
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
    cursorX: relativeX,
    cursorY: relativeY,
    tooltipX: tooltipPosition.x,
    tooltipY: tooltipPosition.y,
    timestamp,
    timeLabel: formatUsageTooltipTime(locale, Math.round(timestamp), range),
    metricLabel,
    entries,
  };
}

function formatUsageTickLabel(locale: string, timestampSec: number, range: ApiProxyUsageRange) {
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

function clampUsageValue(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value));
}

function normalizeCurvePoints(points: ApiProxyUsagePlotPoint[]) {
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

function buildMonotoneCurveSegments(points: ApiProxyUsagePlotPoint[]) {
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
    if (previousSlope === 0 || nextSlope === 0 || Math.sign(previousSlope) !== Math.sign(nextSlope)) {
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
      cp1y: clampUsageValue(start.y + (tangents[index] * deltaX) / 3, minSegmentY, maxSegmentY),
      cp2x: end.x - deltaX / 3,
      cp2y: clampUsageValue(end.y - (tangents[index + 1] * deltaX) / 3, minSegmentY, maxSegmentY),
    } satisfies ApiProxyUsageCurveSegment;
  });
}

function buildSmoothPath(points: ApiProxyUsagePlotPoint[], segments: ApiProxyUsageCurveSegment[]) {
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

function buildAreaPath(points: ApiProxyUsagePlotPoint[], baselineY: number, linePath: string) {
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

function ApiProxyUsageChart({
  copy,
  locale,
  stats,
  range,
  metric,
  loading,
  clearing,
  proxyRunning,
  onSelectRange,
  onSelectMetric,
  onClear,
}: ApiProxyUsageChartProps) {
  const rangeOptions: Array<{ value: ApiProxyUsageRange; label: string }> = [
    { value: "1h", label: "1h" },
    { value: "24h", label: "24h" },
    { value: "7d", label: "7d" },
    { value: "14d", label: "14d" },
    { value: "30d", label: "30d" },
  ];
  const metricOptions: Array<{ value: ApiProxyUsageMetric; label: string }> = [
    { value: "calls", label: copy.chartCalls },
    { value: "tokens", label: copy.chartTokens },
  ];
  const chartWidth = 960;
  const chartHeight = 320;
  const margins = useMemo(() => ({ top: 18, right: 44, bottom: 46, left: 56 }), []);
  const plotWidth = chartWidth - margins.left - margins.right;
  const plotHeight = chartHeight - margins.top - margins.bottom;
  const frameRef = useRef<HTMLDivElement | null>(null);
  const cardRef = useRef<HTMLElement | null>(null);
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [hoverState, setHoverState] = useState<ApiProxyUsageHoverState | null>(null);
  const [contextMenu, setContextMenu] = useState<ApiProxyUsageContextMenu | null>(null);
  const [chartMotion, setChartMotion] = useState<ApiProxyUsageChartMotion>({
    id: 0,
    mode: "rise",
    offset: 0,
  });
  const chartMotionIdRef = useRef(0);
  const previousChartWindowRef = useRef<{
    range: ApiProxyUsageRange;
    metric: ApiProxyUsageMetric;
    endTimestamp: number;
  } | null>(null);
  const selectedRangeSeconds = API_PROXY_USAGE_RANGE_SECONDS[range];

  const chartData = useMemo(() => {
    const ordered = [...(stats?.series ?? [])].sort((left, right) => left.model.localeCompare(right.model));
    const latestPointTimestamp = ordered.reduce((max, item) => {
      const latestPoint = item.points.reduce((pointMax, point) => Math.max(pointMax, point.timestamp), 0);
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
      const pointValues = [...item.metricPoints];
      const latestPoint = pointValues[pointValues.length - 1];
      if (latestPoint && latestPoint.timestamp < endTimestamp) {
        pointValues.push({
          timestamp: endTimestamp,
          value: latestPoint.value,
        });
      }

      const points = pointValues.map((point) => {
        const clampedTimestamp = clampUsageValue(point.timestamp, startTimestamp, endTimestamp);
        const x =
          margins.left +
          ((clampedTimestamp - startTimestamp) / Math.max(selectedRangeSeconds, 1)) * plotWidth;
        const y = baselineY - (Math.max(0, point.value) / yDomain) * plotHeight;
        return {
          timestamp: point.timestamp,
          value: point.value,
          x: clampUsageValue(x, margins.left, margins.left + plotWidth),
          y: clampUsageValue(y, margins.top, baselineY),
        };
      });

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
        points,
        curveSegments,
        linePath,
        areaPath,
      } satisfies ApiProxyUsageSeriesView;
    });

    return {
      series,
      maxValue,
      endTimestamp,
    };
  }, [metric, margins.left, margins.top, plotHeight, plotWidth, selectedRangeSeconds, stats]);

  const series = chartData.series;
  const hasUsageData = chartData.maxValue > 0 && series.length > 0;
  const endTimestamp = chartData.endTimestamp;
  const startTimestamp = endTimestamp - selectedRangeSeconds;
  const metricLabel = metric === "calls" ? copy.chartCalls : copy.chartTokens;
  const xTicks = [0, 0.25, 0.5, 0.75, 1].map((fraction) => {
    const timestamp = Math.round(endTimestamp - selectedRangeSeconds + selectedRangeSeconds * fraction);
    const anchor: "start" | "middle" | "end" =
      fraction === 1 ? "end" : fraction === 0 ? "start" : "middle";
    return {
      timestamp,
      x: margins.left + plotWidth * fraction,
      anchor,
      label: formatUsageTickLabel(locale, timestamp, range),
    };
  });

  const yDomain = chartData.maxValue > 0 ? chartData.maxValue * 1.12 : 1;
  const yTicks = hasUsageData
    ? [...new Set([0, 0.25, 0.5, 0.75, 1].map((fraction) => Math.round(yDomain * fraction)))]
        .filter((value) => Number.isFinite(value))
        .sort((left, right) => left - right)
        .map((value) => ({
          value,
          y: margins.top + plotHeight - (value / yDomain) * plotHeight,
        }))
    : [];
  const updatedLabel = stats
    ? new Date(stats.updatedAt * 1000).toLocaleString(locale)
    : null;

  const chartMotionClass =
    chartMotion.mode === "rise"
      ? " isRising"
      : chartMotion.mode === "slide"
        ? " isSliding"
        : "";
  const chartMotionStyle = {
    transformOrigin: `${margins.left + plotWidth / 2}px ${margins.top + plotHeight}px`,
    "--proxyUsageSlideOffset": `${chartMotion.offset}px`,
  } as CSSProperties & Record<"--proxyUsageSlideOffset", string>;

  useEffect(() => {
    if (!hasUsageData || endTimestamp <= 0) {
      previousChartWindowRef.current = null;
      return;
    }

    const previous = previousChartWindowRef.current;
    let nextMotion: ApiProxyUsageChartMotion | null = null;
    if (!previous || previous.range !== range || previous.metric !== metric) {
      nextMotion = {
        id: chartMotionIdRef.current + 1,
        mode: "rise",
        offset: 0,
      };
    } else if (endTimestamp > previous.endTimestamp) {
      const advancedSeconds = endTimestamp - previous.endTimestamp;
      const offset = Math.min(
        plotWidth,
        (advancedSeconds / Math.max(selectedRangeSeconds, 1)) * plotWidth,
      );
      if (offset > 0) {
        nextMotion = {
          id: chartMotionIdRef.current + 1,
          mode: "slide",
          offset,
        };
      }
    }

    previousChartWindowRef.current = { range, metric, endTimestamp };
    if (!nextMotion) {
      return;
    }

    chartMotionIdRef.current = nextMotion.id;
    const animationFrame = window.requestAnimationFrame(() => {
      setChartMotion(nextMotion);
    });
    return () => window.cancelAnimationFrame(animationFrame);
  }, [endTimestamp, hasUsageData, metric, plotWidth, range, selectedRangeSeconds]);

  const refreshHoverState = useCallback(
    (clientX: number, clientY: number) => {
      const frameRect = frameRef.current?.getBoundingClientRect();
      const svgRect = svgRef.current?.getBoundingClientRect();
      if (!frameRect || !svgRect || !hasUsageData) {
        setHoverState(null);
        return;
      }

      const next = resolveUsageHoverState({
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
        rangeSeconds: selectedRangeSeconds,
        metric,
        metricLabel,
      });

      setHoverState(next);
    },
    [chartHeight, chartWidth, hasUsageData, locale, margins, metric, metricLabel, range, selectedRangeSeconds, series, startTimestamp],
  );

  const handlePointerMove = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      refreshHoverState(event.clientX, event.clientY);
    },
    [refreshHoverState],
  );

  const handlePointerLeave = useCallback(() => {
    setHoverState(null);
  }, []);

  useEffect(() => {
    if (!contextMenu) {
      return;
    }

    const closeMenu = () => {
      setContextMenu(null);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeMenu();
      }
    };

    window.addEventListener("click", closeMenu);
    window.addEventListener("resize", closeMenu);
    window.addEventListener("scroll", closeMenu, true);
    window.addEventListener("keydown", closeOnEscape);
    return () => {
      window.removeEventListener("click", closeMenu);
      window.removeEventListener("resize", closeMenu);
      window.removeEventListener("scroll", closeMenu, true);
      window.removeEventListener("keydown", closeOnEscape);
    };
  }, [contextMenu]);

  const handleOpenContextMenu = useCallback(
    (event: ReactMouseEvent<HTMLElement>) => {
      event.preventDefault();
      event.stopPropagation();
      const cardRect = cardRef.current?.getBoundingClientRect();
      if (!cardRect) {
        return;
      }
      const position = clampContextMenuPosition(
        event.clientX - cardRect.left,
        event.clientY - cardRect.top,
        cardRect.width,
        cardRect.height,
      );
      setContextMenu(position);
    },
    [],
  );

  const handleClearUsage = useCallback(() => {
    if (clearing) {
      return;
    }
    setContextMenu(null);
    void onClear();
  }, [clearing, onClear]);

  const handleContextMenuClick = useCallback(
    (event: ReactMouseEvent<HTMLButtonElement>) => {
      event.preventDefault();
      event.stopPropagation();
      handleClearUsage();
    },
    [handleClearUsage],
  );

  return (
    <section
      ref={cardRef}
      className={`proxySectionCard proxyUsageCard${proxyRunning ? " isRunning" : ""}`}
      onContextMenu={handleOpenContextMenu}
    >
      <div className="proxyUsageHeader">
        <div className="proxyUsageHeading">
          <span className="proxyLabel">{copy.chartKicker}</span>
          <h3>{copy.chartTitle}</h3>
          <p>{copy.chartDescription}</p>
        </div>
        <div className="proxyUsageHeaderMeta">
          <span className={`proxyHeaderStat proxyUsageStatus${proxyRunning ? " isRunning" : ""}`}>
            <span className={`proxyStatusDot${proxyRunning ? " isRunning" : ""}`} aria-hidden="true" />
            <span>{copy.chartKicker}</span>
            <strong>{proxyRunning ? copy.statusRunning : copy.statusStopped}</strong>
          </span>
          {updatedLabel ? <span className="proxyUsageUpdated">{copy.chartUpdatedAt}: {updatedLabel}</span> : null}
        </div>
      </div>

      <div className="proxyUsageControls">
        <div className="proxyUsageGroup" role="group" aria-label={copy.chartRangeLabel}>
          {rangeOptions.map((option) => (
            <button
              key={option.value}
              type="button"
              className={`proxyUsageChip${range === option.value ? " isActive" : ""}`}
              aria-pressed={range === option.value}
              onClick={() => onSelectRange(option.value)}
            >
              {option.label}
            </button>
          ))}
        </div>

        <div className="proxyUsageGroup" role="group" aria-label={copy.chartMetricLabel}>
          {metricOptions.map((option) => (
            <button
              key={option.value}
              type="button"
              className={`proxyUsageChip${metric === option.value ? " isActive" : ""}`}
              aria-pressed={metric === option.value}
              onClick={() => onSelectMetric(option.value)}
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>

      <div
        ref={frameRef}
        className={`proxyUsageFrame${loading ? " isLoading" : ""}${hasUsageData ? " hasData" : ""}`}
        onContextMenu={handleOpenContextMenu}
        onPointerMove={handlePointerMove}
        onPointerLeave={handlePointerLeave}
      >
        {hasUsageData ? (
          <>
            <svg
              ref={svgRef}
              className="proxyUsageSvg"
              viewBox={`0 0 ${chartWidth} ${chartHeight}`}
              role="img"
              aria-label={`${copy.chartTitle} ${metric === "calls" ? copy.chartCalls : copy.chartTokens}`}
            >
              <defs>
                <clipPath id="proxy-usage-clip">
                  <rect x={margins.left} y={margins.top} width={plotWidth} height={plotHeight} rx="14" />
                </clipPath>
                {series.map((item) => (
                  <linearGradient
                    key={item.gradientId}
                    id={item.gradientId}
                    x1="0%"
                    x2="0%"
                    y1="0%"
                    y2="100%"
                  >
                    <stop offset="0%" stopColor={item.color} stopOpacity="0.34" />
                    <stop offset="100%" stopColor={item.color} stopOpacity="0.02" />
                  </linearGradient>
                ))}
              </defs>

              <rect className="proxyUsagePlotBg" x={margins.left} y={margins.top} width={plotWidth} height={plotHeight} rx="14" />

              <g className="proxyUsageGrid">
                {yTicks.map((tick) => (
                  <g key={`y-${tick.value}`}>
                    <line x1={margins.left} x2={margins.left + plotWidth} y1={tick.y} y2={tick.y} />
                    <text x={margins.left - 10} y={tick.y + 4} textAnchor="end">
                      {formatUsageAxisValue(tick.value)}
                    </text>
                  </g>
                ))}
                {xTicks.map((tick, index) => (
                  <g key={`x-${tick.timestamp}-${index}`}>
                    <line x1={tick.x} x2={tick.x} y1={margins.top} y2={margins.top + plotHeight} className="proxyUsageGridLineVertical" />
                    <text x={tick.x} y={margins.top + plotHeight + 24} textAnchor={tick.anchor}>
                      {tick.label}
                    </text>
                  </g>
                ))}
              </g>

              <g clipPath="url(#proxy-usage-clip)">
                <g
                  key={chartMotion.id}
                  className={`proxyUsageAnimatedPlot${chartMotionClass}`}
                  style={chartMotionStyle}
                >
                  {series.map((item) => (
                    <g
                      key={item.gradientId}
                      className="proxyUsageSeries"
                    >
                      <path className="proxyUsageArea" d={item.areaPath} fill={`url(#${item.gradientId})`} />
                      <path className="proxyUsageLine" d={item.linePath} stroke={item.color} />
                    </g>
                  ))}
                </g>
                {hoverState ? (
                  <g className="proxyUsageHoverLayer" pointerEvents="none">
                    <line
                      className="proxyUsageHoverCrosshair"
                      x1={hoverState.cursorX}
                      x2={hoverState.cursorX}
                      y1={margins.top}
                      y2={margins.top + plotHeight}
                    />
                    {hoverState.entries.map((entry) => (
                      <g key={`${entry.model}-${entry.pointX}-${entry.pointY}`}>
                        <circle
                          className="proxyUsageHoverMarkerHalo"
                          cx={entry.pointX}
                          cy={entry.pointY}
                          r="7"
                          fill={entry.color}
                        />
                        <circle
                          className="proxyUsageHoverMarker"
                          cx={entry.pointX}
                          cy={entry.pointY}
                          r="3.5"
                          fill={entry.color}
                        />
                      </g>
                    ))}
                  </g>
                ) : null}
              </g>
            </svg>
            {hoverState ? (
              <div
                className="proxyUsageTooltip"
                style={{ left: hoverState.tooltipX, top: hoverState.tooltipY }}
                aria-hidden="true"
              >
                <div className="proxyUsageTooltipHeader">
                  <span className="proxyUsageTooltipTime">{hoverState.timeLabel}</span>
                  <span className="proxyUsageTooltipMetricLabel">{hoverState.metricLabel}</span>
                </div>
                <div className="proxyUsageTooltipEntries">
                  {hoverState.entries.map((entry) => (
                    <div className="proxyUsageTooltipEntry" key={`${entry.model}-${entry.value}`}>
                      <span className="proxyUsageTooltipSwatch" style={{ background: entry.color }} />
                      <span className="proxyUsageTooltipEntryModel" title={entry.model}>
                        {entry.model}
                      </span>
                      <strong className="proxyUsageTooltipEntryValue">{entry.valueLabel}</strong>
                    </div>
                  ))}
                </div>
              </div>
            ) : null}
            {loading ? <div className="proxyUsageFrameBadge">{copy.chartLoadingTitle}</div> : null}
          </>
        ) : (
          <div className="proxyUsageState" aria-live="polite" aria-busy={loading || clearing}>
            <span className={`proxyUsageStateOrb${loading ? " isLoading" : ""}`} />
            <strong>{loading ? copy.chartLoadingTitle : copy.chartEmptyTitle}</strong>
            <p>{loading ? copy.chartLoadingDescription : copy.chartEmptyDescription}</p>
          </div>
        )}
      </div>

      {contextMenu ? (
        <div
          className="proxyUsageContextMenu"
          role="menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(event) => event.stopPropagation()}
          onContextMenu={(event) => {
            event.preventDefault();
            event.stopPropagation();
          }}
        >
          <button
            type="button"
            className="proxyUsageContextMenuItem"
            role="menuitem"
            disabled={clearing}
            onClick={handleContextMenuClick}
          >
            {copy.chartClearHistory}
          </button>
        </div>
      ) : null}

      {hasUsageData ? (
        <div className="proxyUsageLegend" aria-label={copy.chartTitle}>
          {series.map((item) => {
            const primary = metric === "calls" ? item.totalCalls : item.totalTokens;
            const secondary = metric === "calls" ? item.totalTokens : item.totalCalls;
            return (
              <article key={item.gradientId} className="proxyUsageLegendItem">
                <span className="proxyUsageLegendSwatch" style={{ background: item.color }} aria-hidden="true" />
                <div className="proxyUsageLegendBody">
                  <strong title={item.model}>{item.model}</strong>
                  <span>
                    {formatUsageMetricValue(primary, locale)} {metric === "calls" ? copy.chartCalls : copy.chartTokens}
                  </span>
                  <small>
                    {formatUsageMetricValue(secondary, locale)} {metric === "calls" ? copy.chartTokens : copy.chartCalls}
                  </small>
                </div>
              </article>
            );
          })}
        </div>
      ) : null}
    </section>
  );
}

export function ApiProxyPanel({
  status,
  apiProxyUsageStats,
  apiProxyUsageRange,
  apiProxyUsageMetric,
  apiProxyUsageLoading,
  apiProxyUsageClearing,
  cloudflaredStatus,
  accountCount,
  autoStartEnabled,
  savedPort,
  loadBalanceMode,
  sequentialFiveHourLimitPercent,
  remoteServers,
  remoteStatuses,
  remoteLogs,
  savingSettings,
  starting,
  stopping,
  refreshingApiKey,
  refreshingRemoteId,
  deployingRemoteId,
  startingRemoteId,
  stoppingRemoteId,
  readingRemoteLogsId,
  installingDependencyName,
  installingDependencyTargetId,
  installingCloudflared,
  startingCloudflared,
  stoppingCloudflared,
  onStart,
  onStop,
  onSelectApiProxyUsageRange,
  onSelectApiProxyUsageMetric,
  onClearApiProxyUsageStats,
  onRefreshApiKey,
  onRefresh,
  onToggleAutoStart,
  onPersistPort,
  onUpdateLoadBalanceMode,
  onUpdateSequentialFiveHourLimitPercent,
  onUpdateRemoteServers,
  onRefreshRemoteStatus,
  onDeployRemote,
  onStartRemote,
  onStopRemote,
  onReadRemoteLogs,
  onPickLocalIdentityFile,
  onRefreshCloudflared,
  onInstallCloudflared,
  onStartCloudflared,
  onStopCloudflared,
}: ApiProxyPanelProps) {
  const { copy, locale } = useI18n();
  const proxyCopy = copy.apiProxy;
  const remoteAuthOptions = REMOTE_AUTH_OPTIONS.map((option) => ({
    ...option,
    label:
      option.id === "keyContent"
        ? proxyCopy.remoteAuthKeyContent
        : option.id === "keyFile"
          ? proxyCopy.remoteAuthKeyFile
          : option.id === "keyPath"
            ? proxyCopy.remoteAuthKeyPath
            : proxyCopy.remoteAuthPassword,
  }));
  const busy = starting || stopping;
  const cloudflaredBusy = installingCloudflared || startingCloudflared || stoppingCloudflared;
  const [portDraft, setPortDraft] = useState<string | null>(null);
  const [sequentialLimitDraft, setSequentialLimitDraft] = useState<number | null>(null);
  const [publicAccessEnabled, setPublicAccessEnabled] = useState(cloudflaredStatus.running);
  const [tunnelMode, setTunnelMode] = useState<CloudflaredTunnelMode>(
    cloudflaredStatus.tunnelMode ?? "quick",
  );
  const [useHttp2, setUseHttp2] = useState(cloudflaredStatus.useHttp2);
  const [remoteDrafts, setRemoteDrafts] = useState<RemoteServerDraft[]>(() =>
    readCachedRemoteDrafts(remoteServers),
  );
  const [selectedRemoteId, setSelectedRemoteId] = useState<string | null>(() =>
    readCachedSelectedRemoteId(remoteServers),
  );
  const [editingRemoteId, setEditingRemoteId] = useState<string | null>(() =>
    readCachedEditingRemoteId(remoteServers),
  );
  const [diagnosticsRemoteId, setDiagnosticsRemoteId] = useState<string | null>(null);
  const [remoteHistory, setRemoteHistory] = useState<Record<string, number>>(() =>
    readCachedRemoteHistory(remoteServers),
  );
  const [namedInput, setNamedInput] = useState({
    apiToken: "",
    accountId: "",
    zoneId: "",
    hostname: cloudflaredStatus.customHostname ?? "",
  });
  const commitSequentialLimitRef = useRef<number | null>(null);
  const cloudflaredEnabled = publicAccessEnabled || cloudflaredStatus.running;

  const loadBalanceOptions = useMemo(
    () => [
      { id: "average" as const, label: proxyCopy.loadBalanceAverage },
      { id: "sequential" as const, label: proxyCopy.loadBalanceSequential },
    ],
    [proxyCopy.loadBalanceAverage, proxyCopy.loadBalanceSequential],
  );

  const effectiveRemoteDrafts =
    remoteDrafts.length === 0 && remoteServers.length > 0
      ? remoteServers.map(configToDraft)
      : remoteDrafts;

  useEffect(() => {
    writeStorageValue(REMOTE_DRAFTS_CACHE_KEY, JSON.stringify(effectiveRemoteDrafts));
  }, [effectiveRemoteDrafts]);

  useEffect(() => {
    const resolvedEditingRemoteId =
      editingRemoteId && effectiveRemoteDrafts.some((draft) => draft.id === editingRemoteId)
        ? editingRemoteId
        : null;
    writeStorageValue(REMOTE_EXPANDED_CACHE_KEY, resolvedEditingRemoteId);
  }, [effectiveRemoteDrafts, editingRemoteId]);

  useEffect(() => {
    const resolvedSelectedRemoteId =
      selectedRemoteId && effectiveRemoteDrafts.some((draft) => draft.id === selectedRemoteId)
        ? selectedRemoteId
        : effectiveRemoteDrafts[0]?.id ?? null;
    writeStorageValue(REMOTE_SELECTED_CACHE_KEY, resolvedSelectedRemoteId, "local");
  }, [effectiveRemoteDrafts, selectedRemoteId]);

  useEffect(() => {
    writeStorageValue(REMOTE_HISTORY_CACHE_KEY, JSON.stringify(remoteHistory), "local");
  }, [remoteHistory]);

  const portInput = portDraft ?? String(status.port ?? savedPort ?? DEFAULT_PROXY_PORT);
  const effectiveSequentialLimit = sequentialLimitDraft ?? sequentialFiveHourLimitPercent;
  const rawPort = portInput.trim();
  const effectivePort = !rawPort
    ? 8787
    : Number.isInteger(Number(rawPort)) && Number(rawPort) >= 1 && Number(rawPort) <= 65535
      ? Number(rawPort)
      : null;
  const hasRemoteServers = effectiveRemoteDrafts.length > 0;
  const resolvedSelectedRemoteId =
    selectedRemoteId && effectiveRemoteDrafts.some((draft) => draft.id === selectedRemoteId)
      ? selectedRemoteId
      : effectiveRemoteDrafts[0]?.id ?? null;
  const selectedRemoteDraft =
    resolvedSelectedRemoteId === null
      ? null
      : effectiveRemoteDrafts.find((draft) => draft.id === resolvedSelectedRemoteId) ?? null;
  const selectedRemoteConfig = selectedRemoteDraft ? draftToConfig(selectedRemoteDraft) : null;
  const selectedRemoteStatus = selectedRemoteDraft ? remoteStatuses[selectedRemoteDraft.id] : null;
  const selectedRemoteLog = selectedRemoteDraft ? remoteLogs[selectedRemoteDraft.id] : undefined;
  const selectedRemoteConfigured = selectedRemoteDraft
    ? isRemoteDraftConfigured(selectedRemoteDraft)
    : false;
  const selectedRemoteIdentity = selectedRemoteDraft
    ? selectedRemoteDraft.label.trim() || selectedRemoteDraft.host.trim() || proxyCopy.remoteTitle
    : proxyCopy.remoteTitle;
  const selectedRefreshing =
    selectedRemoteDraft !== null && refreshingRemoteId === selectedRemoteDraft.id;
  const selectedDeploying =
    selectedRemoteDraft !== null && deployingRemoteId === selectedRemoteDraft.id;
  const selectedStarting =
    selectedRemoteDraft !== null && startingRemoteId === selectedRemoteDraft.id;
  const selectedStopping =
    selectedRemoteDraft !== null && stoppingRemoteId === selectedRemoteDraft.id;
  const selectedReadingLogs =
    selectedRemoteDraft !== null && readingRemoteLogsId === selectedRemoteDraft.id;
  const selectedInstallingDependency =
    selectedRemoteDraft !== null &&
    installingDependencyName === "sshpass" &&
    installingDependencyTargetId === selectedRemoteDraft.id;
  const selectedRemoteBusy =
    selectedRefreshing ||
    selectedDeploying ||
    selectedStarting ||
    selectedStopping ||
    selectedInstallingDependency;
  const selectedRemoteLastChecked =
    selectedRemoteDraft !== null ? remoteHistory[selectedRemoteDraft.id] ?? 0 : 0;
  const selectedRemoteCheckedLabel =
    selectedRemoteLastChecked > 0
      ? formatRemoteHistoryTime(locale, selectedRemoteLastChecked)
      : proxyCopy.remoteNeverChecked;
  const editingSelectedRemote =
    selectedRemoteDraft !== null && editingRemoteId === selectedRemoteDraft.id;
  const diagnosticsOpen =
    selectedRemoteDraft !== null && diagnosticsRemoteId === selectedRemoteDraft.id;
  const selectedRemoteRunningText = selectedRemoteStatus
    ? selectedRemoteStatus.running
      ? proxyCopy.statusRunning
      : proxyCopy.statusStopped
    : proxyCopy.remoteStatusUnknown;
  const selectedRemoteInstalledText = selectedRemoteStatus
    ? selectedRemoteStatus.installed
      ? proxyCopy.remoteInstalledYes
      : proxyCopy.remoteInstalledNo
    : proxyCopy.remoteStatusUnknown;
  const selectedRemoteSystemdText = selectedRemoteStatus
    ? selectedRemoteStatus.serviceInstalled
      ? proxyCopy.remoteInstalledYes
      : proxyCopy.remoteInstalledNo
    : proxyCopy.remoteStatusUnknown;
  const selectedRemoteEnabledText = selectedRemoteStatus
    ? selectedRemoteStatus.enabled
      ? proxyCopy.remoteInstalledYes
      : proxyCopy.remoteInstalledNo
    : proxyCopy.remoteStatusUnknown;
  const remoteOrder = Object.fromEntries(
    effectiveRemoteDrafts.map((draft, index) => [draft.id, index]),
  );
  const orderedRemoteDrafts = [...effectiveRemoteDrafts].sort((left, right) => {
    const historyDelta = (remoteHistory[right.id] ?? 0) - (remoteHistory[left.id] ?? 0);
    if (historyDelta !== 0) {
      return historyDelta;
    }
    return (remoteOrder[left.id] ?? 0) - (remoteOrder[right.id] ?? 0);
  });

  const namedReady =
    namedInput.apiToken.trim() !== "" &&
    namedInput.accountId.trim() !== "" &&
    namedInput.zoneId.trim() !== "" &&
    namedInput.hostname.trim() !== "";

  const canStartCloudflared =
    status.running &&
    status.port !== null &&
    cloudflaredStatus.installed &&
    !cloudflaredBusy &&
    (tunnelMode === "quick" || namedReady);

  const cloudflaredInput: StartCloudflaredTunnelInput | null =
    status.port === null
      ? null
      : {
          apiProxyPort: status.port,
          useHttp2,
          mode: tunnelMode,
          named:
            tunnelMode === "named"
              ? {
                  apiToken: namedInput.apiToken.trim(),
                  accountId: namedInput.accountId.trim(),
                  zoneId: namedInput.zoneId.trim(),
                  hostname: namedInput.hostname.trim(),
                }
              : null,
        };

  const persistRemoteDrafts = (drafts: RemoteServerDraft[]) => {
    onUpdateRemoteServers(drafts.map(draftToConfig));
  };

  const persistPortIfNeeded = async (explicitPort?: number | null) => {
    const nextPort = explicitPort ?? effectivePort;
    if (nextPort === null || nextPort === savedPort) {
      return;
    }
    await onPersistPort(nextPort);
  };

  const commitSequentialLimit = useCallback(
    (value: number) => {
      const nextValue = Math.min(100, Math.max(0, Math.round(value)));
      if (
        nextValue === sequentialFiveHourLimitPercent ||
        commitSequentialLimitRef.current === nextValue
      ) {
        setSequentialLimitDraft(null);
        return;
      }

      commitSequentialLimitRef.current = nextValue;
      void Promise.resolve(onUpdateSequentialFiveHourLimitPercent(nextValue)).finally(() => {
        commitSequentialLimitRef.current = null;
        setSequentialLimitDraft(null);
      });
    },
    [onUpdateSequentialFiveHourLimitPercent, sequentialFiveHourLimitPercent],
  );

  const handleStart = async () => {
    await persistPortIfNeeded(effectivePort);
    await onStart(effectivePort);
    setPortDraft(null);
  };

  const updateRemoteDraft = (
    id: string,
    key: keyof Omit<RemoteServerDraft, "id">,
    value: string | RemoteAuthMode,
  ) => {
    setRemoteDrafts((current) =>
      (current.length === 0 && remoteServers.length > 0 ? remoteServers.map(configToDraft) : current).map((draft) => {
        if (draft.id !== id) {
          return draft;
        }
        const next = { ...draft, [key]: value } as RemoteServerDraft;
        if (key === "authMode") {
          if (value !== "keyContent") {
            next.privateKey = "";
          }
          if (value !== "password") {
            next.password = "";
          }
          if (value === "keyContent" || value === "password") {
            next.identityFile = "";
          }
        }
        return next;
      }),
    );
  };

  const addRemoteDraft = () => {
    const nextDraft = createRemoteDraft();
    setRemoteDrafts((current) => [
      ...(current.length === 0 && remoteServers.length > 0 ? remoteServers.map(configToDraft) : current),
      nextDraft,
    ]);
    setSelectedRemoteId(nextDraft.id);
    setEditingRemoteId(nextDraft.id);
    setDiagnosticsRemoteId(null);
  };

  const removeRemoteDraft = (id: string) => {
    const next = effectiveRemoteDrafts.filter((draft) => draft.id !== id);
    setRemoteDrafts(next);
    persistRemoteDrafts(next);
    setSelectedRemoteId((current) => (current === id ? next[0]?.id ?? null : current));
    setEditingRemoteId((current) => (current === id ? null : current));
    setDiagnosticsRemoteId((current) => (current === id ? null : current));
    setRemoteHistory((current) => {
      if (!(id in current)) {
        return current;
      }
      const nextHistory = { ...current };
      delete nextHistory[id];
      return nextHistory;
    });
  };

  const selectRemoteDraft = (id: string) => {
    setSelectedRemoteId(id);
    setDiagnosticsRemoteId(null);
    setRemoteHistory((current) => ({ ...current, [id]: Date.now() }));

    const targetDraft = effectiveRemoteDrafts.find((draft) => draft.id === id);
    if (!targetDraft) {
      return;
    }

    if (!isRemoteDraftConfigured(targetDraft)) {
      setEditingRemoteId(id);
      return;
    }

    onRefreshRemoteStatus(draftToConfig(targetDraft));
  };

  const toggleSelectedDiagnostics = () => {
    if (!selectedRemoteDraft) {
      return;
    }

    const nextOpenId = diagnosticsOpen ? null : selectedRemoteDraft.id;
    setDiagnosticsRemoteId(nextOpenId);

    if (
      nextOpenId &&
      selectedRemoteConfigured &&
      selectedRemoteConfig &&
      !remoteLogs[selectedRemoteDraft.id] &&
      !selectedReadingLogs
    ) {
      onReadRemoteLogs(selectedRemoteConfig);
    }
  };

  let remoteGuideTitle = proxyCopy.remoteStatusUnknown;
  let remoteGuideDescription = proxyCopy.remoteDescription;

  if (selectedRemoteDraft && !selectedRemoteConfigured) {
    remoteGuideTitle = proxyCopy.remoteGuideSetupTitle;
    remoteGuideDescription = proxyCopy.remoteGuideSetupDescription;
  } else if (selectedRefreshing) {
    remoteGuideTitle = proxyCopy.remoteRefreshing;
    remoteGuideDescription = proxyCopy.remoteDescription;
  } else if (selectedRemoteStatus?.running) {
    remoteGuideTitle = proxyCopy.remoteGuideReadyTitle;
    remoteGuideDescription = proxyCopy.remoteGuideReadyDescription;
  } else if (selectedRemoteStatus?.installed) {
    remoteGuideTitle = proxyCopy.remoteGuideStartTitle;
    remoteGuideDescription = proxyCopy.remoteGuideStartDescription;
  } else if (selectedRemoteDraft && selectedRemoteConfigured) {
    remoteGuideTitle = proxyCopy.remoteGuideDeployTitle;
    remoteGuideDescription = proxyCopy.remoteGuideDeployDescription;
  }

  return (
    <section className="proxyPage">
      <div className="proxyShell">
        <ApiProxyUsageChart
          copy={proxyCopy}
          locale={locale}
          stats={apiProxyUsageStats}
          range={apiProxyUsageRange}
          metric={apiProxyUsageMetric}
          loading={apiProxyUsageLoading}
          clearing={apiProxyUsageClearing}
          proxyRunning={status.running}
          onSelectRange={onSelectApiProxyUsageRange}
          onSelectMetric={onSelectApiProxyUsageMetric}
          onClear={onClearApiProxyUsageStats}
        />

        <section className="proxySectionCard proxySectionCardPrimary">
          <div className="proxyHeaderStats">
            <span className="proxyHeaderStat">
              <span className={`proxyStatusDot${status.running ? " isRunning" : ""}`} aria-hidden="true" />
              <span>{proxyCopy.statusLabel}</span>
              <strong>{status.running ? proxyCopy.statusRunning : proxyCopy.statusStopped}</strong>
            </span>
            <span className="proxyHeaderStat">
              <span>{proxyCopy.portLabel}</span>
              <strong>{status.port ?? "--"}</strong>
            </span>
            <span className="proxyHeaderStat">
              <span>{proxyCopy.accountCountLabel}</span>
              <strong>{accountCount}</strong>
            </span>
          </div>

          <div className="proxyControlRow">
            <label className="proxyCompactField">
              <span>{proxyCopy.portLabel}</span>
              <input
                className="proxyPortInput"
                inputMode="numeric"
                aria-label={proxyCopy.portInputAriaLabel}
                placeholder={DEFAULT_PROXY_PORT}
                value={portInput}
                onChange={(event) => setPortDraft(event.target.value)}
                onBlur={() => {
                  void (async () => {
                    await persistPortIfNeeded();
                    if (effectivePort !== null) {
                      setPortDraft(null);
                    }
                  })();
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.currentTarget.blur();
                  }
                }}
                disabled={busy || status.running}
              />
            </label>

            <div className="proxySwitchRow proxyInlineSetting">
              <div className="settingMeta">
                <strong>{proxyCopy.defaultStartLabel}</strong>
              </div>
              <label className="themeSwitch" aria-label={proxyCopy.defaultStartLabel}>
                <input
                  type="checkbox"
                  checked={autoStartEnabled}
                  disabled={savingSettings}
                  onChange={(event) => onToggleAutoStart(event.target.checked)}
                />
                <span className="themeSwitchTrack" aria-hidden="true">
                  <span className="themeSwitchThumb" />
                </span>
                <span className="themeSwitchText">
                  {autoStartEnabled
                    ? proxyCopy.defaultStartEnabled
                    : proxyCopy.defaultStartDisabled}
                </span>
              </label>
            </div>

            <div className="proxyControlActions">
              <button className="ghost" onClick={onRefresh} disabled={busy}>
                {proxyCopy.refreshStatus}
              </button>
              {status.running ? (
                <button className="danger" onClick={onStop} disabled={busy}>
                  {stopping ? proxyCopy.stopping : proxyCopy.stop}
                </button>
              ) : (
                <button
                  className="primary"
                  onClick={() => {
                    void handleStart();
                  }}
                  disabled={busy || accountCount === 0 || effectivePort === null}
                >
                  {starting ? proxyCopy.starting : proxyCopy.start}
                </button>
              )}
            </div>
          </div>

          <article className="proxyDetailCard proxyBalanceCard">
            <div className="proxyBalanceHeader">
              <span className="proxyLabel">{proxyCopy.loadBalanceLabel}</span>
              <EditorMultiSelect
                className="proxyModePicker"
                options={loadBalanceOptions}
                value={loadBalanceMode}
                ariaLabel={proxyCopy.loadBalanceLabel}
                placeholder={proxyCopy.loadBalanceLabel}
                disabled={savingSettings}
                onChange={(mode) => {
                  void onUpdateLoadBalanceMode(mode);
                }}
              />
            </div>

            {loadBalanceMode === "sequential" ? (
              <div className="proxySequentialLimit">
                <div className="proxySequentialLimitHeader">
                  <span className="proxyInlineLabel">{proxyCopy.sequentialFiveHourLimitLabel}</span>
                  <strong>{effectiveSequentialLimit}%</strong>
                </div>
                <input
                  className="proxyRangeInput"
                  type="range"
                  min={0}
                  max={100}
                  step={1}
                  value={effectiveSequentialLimit}
                  disabled={savingSettings}
                  aria-label={proxyCopy.sequentialFiveHourLimitLabel}
                  aria-valuetext={`${effectiveSequentialLimit}%`}
                  onChange={(event) => {
                    setSequentialLimitDraft(Number(event.currentTarget.value));
                  }}
                  onPointerUp={(event) => {
                    commitSequentialLimit(Number(event.currentTarget.value));
                  }}
                  onBlur={(event) => {
                    commitSequentialLimit(Number(event.currentTarget.value));
                  }}
                />
                <p>{proxyCopy.sequentialFiveHourLimitDescription}</p>
              </div>
            ) : null}
          </article>

          <div className="proxyDetailGrid">
            <article className="proxyDetailCard proxyEndpointCard">
              <span className="proxyLabel">{proxyCopy.baseUrlLabel}</span>
              <div className="proxyEndpointList">
                <div className="proxyEndpointRow">
                  <div className="proxyEndpointMeta">
                    <span>{proxyCopy.localBaseUrlLabel}</span>
                    <code>{status.baseUrl ?? proxyCopy.baseUrlPlaceholder}</code>
                  </div>
                  <button
                    className="ghost proxyCopyButton"
                    onClick={() => copyText(status.baseUrl)}
                    disabled={!status.baseUrl}
                  >
                    {proxyCopy.copy}
                  </button>
                </div>

                {status.lanBaseUrl ? (
                  <div className="proxyEndpointRow">
                    <div className="proxyEndpointMeta">
                      <span>{proxyCopy.lanBaseUrlLabel}</span>
                      <code>{status.lanBaseUrl}</code>
                    </div>
                    <button
                      className="ghost proxyCopyButton"
                      onClick={() => copyText(status.lanBaseUrl)}
                      disabled={!status.lanBaseUrl}
                    >
                      {proxyCopy.copy}
                    </button>
                  </div>
                ) : null}
              </div>
            </article>

            <article className="proxyDetailCard">
              <div className="proxyDetailHeader">
                <span className="proxyLabel">{proxyCopy.apiKeyLabel}</span>
                <div className="proxyDetailActions">
                  <button
                    className="ghost proxyCopyButton"
                    onClick={onRefreshApiKey}
                    disabled={refreshingApiKey}
                  >
                    {refreshingApiKey ? proxyCopy.refreshingKey : proxyCopy.refreshKey}
                  </button>
                  <button
                    className="ghost proxyCopyButton"
                    onClick={() => copyText(status.apiKey)}
                    disabled={!status.apiKey}
                  >
                    {proxyCopy.copy}
                  </button>
                </div>
              </div>
              <code>{status.apiKey ?? proxyCopy.apiKeyPlaceholder}</code>
            </article>

            <article className="proxyDetailCard">
              <span className="proxyLabel">{proxyCopy.activeAccountLabel}</span>
              <strong>{status.activeAccountLabel ?? proxyCopy.activeAccountEmptyTitle}</strong>
              <p>{status.activeAccountId ?? proxyCopy.activeAccountEmptyDescription}</p>
            </article>

            <article className="proxyDetailCard">
              <span className="proxyLabel">{proxyCopy.lastErrorLabel}</span>
              <p className="proxyErrorText">{status.lastError ?? proxyCopy.none}</p>
            </article>
          </div>
        </section>

        <section className="proxySectionCard">
          <div className="proxySectionHeader">
            <div className="remoteSectionHeading">
              <h3>{proxyCopy.remoteTitle}</h3>
              <p>{proxyCopy.remoteDescription}</p>
            </div>
            <button className="primary" onClick={addRemoteDraft}>
              {proxyCopy.remoteAddServer}
            </button>
          </div>

          {hasRemoteServers ? (
            <div className="remoteWorkspace">
              <aside className="remoteHistoryPanel">
                <div className="remoteHistoryHeader">
                  <span className="proxyLabel">{proxyCopy.remoteHistoryTitle}</span>
                  <strong>{orderedRemoteDrafts.length}</strong>
                </div>
                <div className="remoteHistoryList">
                  {orderedRemoteDrafts.map((draft) => {
                    const remoteStatus = remoteStatuses[draft.id];
                    const remoteIdentity =
                      draft.label.trim() || draft.host.trim() || proxyCopy.remoteTitle;
                    const recentCheckedAt = remoteHistory[draft.id] ?? 0;
                    const historyStateText =
                      refreshingRemoteId === draft.id
                        ? proxyCopy.remoteRefreshing
                        : remoteStatus?.running
                          ? proxyCopy.statusRunning
                          : remoteStatus?.installed
                            ? proxyCopy.statusStopped
                            : isRemoteDraftConfigured(draft)
                              ? proxyCopy.remoteInstalledNo
                              : proxyCopy.remoteStatusUnknown;

                    return (
                      <button
                        key={draft.id}
                        type="button"
                        className={`remoteHistoryItem${
                          resolvedSelectedRemoteId === draft.id ? " isSelected" : ""
                        }`}
                        onClick={() => selectRemoteDraft(draft.id)}
                      >
                        <div className="remoteHistoryItemTop">
                          <div className="remoteHistoryIdentity">
                            <strong>{remoteIdentity}</strong>
                            <span>{draft.host.trim() || "--"}</span>
                          </div>
                          <span
                            className={`remoteServerState${
                              remoteStatus?.running ? " isRunning" : ""
                            }`}
                          >
                            <span
                              className={`proxyStatusDot${
                                remoteStatus?.running ? " isRunning" : ""
                              }`}
                              aria-hidden="true"
                            />
                            {historyStateText}
                          </span>
                        </div>

                        <div className="remoteHistoryItemMeta">
                          <span>
                            SSH {(draft.sshUser.trim() || "root")}:{draft.sshPort.trim() || "--"}
                          </span>
                          <span>
                            {proxyCopy.remoteLastCheckedLabel}{" "}
                            {recentCheckedAt > 0
                              ? formatRemoteHistoryTime(locale, recentCheckedAt)
                              : proxyCopy.remoteNeverChecked}
                          </span>
                        </div>
                      </button>
                    );
                  })}
                </div>
              </aside>

              {selectedRemoteDraft ? (
                <div className="remoteWorkbench">
                  <article className="remoteWorkbenchCard">
                    <div className="remoteWorkbenchHeader">
                      <div className="remoteServerSummary">
                        <div className="remoteServerIdentity">
                          <strong>{selectedRemoteIdentity}</strong>
                          <span>
                            {selectedRemoteStatus?.baseUrl ?? buildRemoteBaseUrl(selectedRemoteDraft)}
                          </span>
                        </div>
                        <div className="remoteServerSummaryMeta">
                          <span className="remoteServerSummaryPill">
                            {proxyCopy.remoteHostLabel} {selectedRemoteDraft.host.trim() || "--"}
                          </span>
                          <span className="remoteServerSummaryPill">
                            SSH {(selectedRemoteDraft.sshUser.trim() || "root")}:
                            {selectedRemoteDraft.sshPort.trim() || "--"}
                          </span>
                          <span className="remoteServerSummaryPill">
                            {proxyCopy.remoteListenPortLabel}{" "}
                            {selectedRemoteDraft.listenPort.trim() || "--"}
                          </span>
                          <span className="remoteServerSummaryPill">
                            {proxyCopy.remoteLastCheckedLabel} {selectedRemoteCheckedLabel}
                          </span>
                        </div>
                      </div>

                      <div className="remoteWorkbenchActions">
                        <button
                          className="ghost"
                          onClick={() => {
                            if (!selectedRemoteDraft) {
                              return;
                            }
                            if (!selectedRemoteConfigured) {
                              setEditingRemoteId(selectedRemoteDraft.id);
                              return;
                            }
                            if (selectedRemoteConfig) {
                              setRemoteHistory((current) => ({
                                ...current,
                                [selectedRemoteDraft.id]: Date.now(),
                              }));
                              onRefreshRemoteStatus(selectedRemoteConfig);
                            }
                          }}
                          disabled={selectedRemoteBusy}
                        >
                          {selectedRefreshing ? proxyCopy.remoteRefreshing : proxyCopy.remoteRefresh}
                        </button>
                        <button
                          className="ghost"
                          onClick={() =>
                            setEditingRemoteId((current) =>
                              current === selectedRemoteDraft.id ? null : selectedRemoteDraft.id,
                            )
                          }
                        >
                          {editingSelectedRemote ? proxyCopy.remoteCollapse : proxyCopy.remoteExpand}
                        </button>
                        {selectedRemoteStatus?.installed ? (
                          selectedRemoteStatus.running ? (
                            <button
                              className="danger"
                              onClick={() => {
                                if (selectedRemoteConfig) {
                                  onStopRemote(selectedRemoteConfig);
                                }
                              }}
                              disabled={!selectedRemoteConfigured || selectedRemoteBusy}
                            >
                              {selectedStopping ? proxyCopy.remoteStopping : proxyCopy.remoteStop}
                            </button>
                          ) : (
                            <button
                              className="primary"
                              onClick={() => {
                                if (selectedRemoteConfig) {
                                  onStartRemote(selectedRemoteConfig);
                                }
                              }}
                              disabled={!selectedRemoteConfigured || selectedRemoteBusy}
                            >
                              {selectedStarting ? proxyCopy.remoteStarting : proxyCopy.remoteStart}
                            </button>
                          )
                        ) : (
                          <button
                            className="primary"
                            onClick={() => {
                              if (selectedRemoteConfig) {
                                onDeployRemote(selectedRemoteConfig);
                              }
                            }}
                            disabled={!selectedRemoteConfigured || selectedRemoteBusy}
                          >
                            {selectedDeploying ? proxyCopy.remoteDeploying : proxyCopy.remoteDeploy}
                          </button>
                        )}
                      </div>
                    </div>

                    <div className="remoteServerStatus">
                      <div className="remoteServerMeta">
                        <span>{proxyCopy.remoteInstalledLabel}</span>
                        <strong>{selectedRemoteInstalledText}</strong>
                      </div>
                      <div className="remoteServerMeta">
                        <span>{proxyCopy.remoteSystemdLabel}</span>
                        <strong>{selectedRemoteSystemdText}</strong>
                      </div>
                      <div className="remoteServerMeta">
                        <span>{proxyCopy.remoteEnabledLabel}</span>
                        <strong>{selectedRemoteEnabledText}</strong>
                      </div>
                      <div className="remoteServerMeta">
                        <span>{proxyCopy.remoteRunningLabel}</span>
                        <strong>{selectedRemoteRunningText}</strong>
                      </div>
                      <div className="remoteServerMeta">
                        <span>{proxyCopy.remotePidLabel}</span>
                        <strong>{selectedRemoteStatus?.pid ?? "--"}</strong>
                      </div>
                    </div>
                  </article>

                  <article className="proxyDetailCard remoteGuideCard">
                    <span className="proxyLabel">{proxyCopy.remoteKicker}</span>
                    <strong>{remoteGuideTitle}</strong>
                    <p>{remoteGuideDescription}</p>
                    <div className="remoteGuideActions">
                      {selectedRemoteConfigured ? (
                        selectedRemoteStatus?.running ? (
                          <>
                            <button
                              className="ghost"
                              onClick={() => copyText(selectedRemoteStatus.baseUrl)}
                              disabled={!selectedRemoteStatus.baseUrl}
                            >
                              {proxyCopy.remoteBaseUrlLabel}
                            </button>
                            <button
                              className="ghost"
                              onClick={() => copyText(selectedRemoteStatus.apiKey ?? null)}
                              disabled={!selectedRemoteStatus.apiKey}
                            >
                              {proxyCopy.remoteApiKeyLabel}
                            </button>
                            <button
                              className="ghost"
                              onClick={toggleSelectedDiagnostics}
                              disabled={selectedReadingLogs}
                            >
                              {diagnosticsOpen
                                ? proxyCopy.remoteCollapse
                                : selectedReadingLogs
                                  ? proxyCopy.remoteReadingLogs
                                  : proxyCopy.remoteReadLogs}
                            </button>
                          </>
                        ) : null
                      ) : (
                        <button
                          className="ghost"
                          onClick={() => setEditingRemoteId(selectedRemoteDraft.id)}
                        >
                          {proxyCopy.remoteExpand}
                        </button>
                      )}
                    </div>

                    <div className="proxyDetailGrid remoteProxyDetailGrid">
                      <article className="proxyDetailCard">
                        <div className="proxyDetailHeader">
                          <span className="proxyLabel">{proxyCopy.remoteBaseUrlLabel}</span>
                          <button
                            className="ghost proxyCopyButton"
                            onClick={() =>
                              copyText(
                                selectedRemoteStatus?.baseUrl ??
                                  buildRemoteBaseUrl(selectedRemoteDraft),
                              )
                            }
                          >
                            {proxyCopy.copy}
                          </button>
                        </div>
                        <code>
                          {selectedRemoteStatus?.baseUrl ?? buildRemoteBaseUrl(selectedRemoteDraft)}
                        </code>
                      </article>

                      <article className="proxyDetailCard">
                        <div className="proxyDetailHeader">
                          <span className="proxyLabel">{proxyCopy.remoteApiKeyLabel}</span>
                          <button
                            className="ghost proxyCopyButton"
                            onClick={() => copyText(selectedRemoteStatus?.apiKey ?? null)}
                            disabled={!selectedRemoteStatus?.apiKey}
                          >
                            {proxyCopy.copy}
                          </button>
                        </div>
                        <code>{selectedRemoteStatus?.apiKey ?? proxyCopy.apiKeyPlaceholder}</code>
                      </article>

                      <article className="proxyDetailCard">
                        <span className="proxyLabel">{proxyCopy.remoteServiceLabel}</span>
                        <code>{selectedRemoteStatus?.serviceName ?? proxyCopy.remoteStatusUnknown}</code>
                      </article>
                    </div>
                  </article>

                  {editingSelectedRemote ? (
                    <div className="remoteWorkbenchSection">
                      <div className="remoteWorkbenchSectionHeader">
                        <div>
                          <span className="proxyLabel">{proxyCopy.remoteConfigTitle}</span>
                          <strong>{selectedRemoteIdentity}</strong>
                        </div>
                        <div className="remoteWorkbenchSectionActions">
                          <button
                            className="ghost"
                            onClick={() => {
                              persistRemoteDrafts(effectiveRemoteDrafts);
                              setEditingRemoteId(null);
                            }}
                            disabled={selectedRemoteBusy}
                          >
                            {proxyCopy.remoteSave}
                          </button>
                          <button
                            className="ghost"
                            onClick={() => removeRemoteDraft(selectedRemoteDraft.id)}
                            disabled={selectedRemoteBusy}
                          >
                            {proxyCopy.remoteRemove}
                          </button>
                        </div>
                      </div>

                      <div className="remoteServerPanel">
                        <div className="remoteServerGrid">
                          <label className="remoteServerField">
                            <span>{proxyCopy.remoteNameLabel}</span>
                            <input
                              value={selectedRemoteDraft.label}
                              onChange={(event) =>
                                updateRemoteDraft(
                                  selectedRemoteDraft.id,
                                  "label",
                                  event.target.value,
                                )
                              }
                              placeholder="tokyo-01"
                            />
                          </label>
                          <label className="remoteServerField">
                            <span>{proxyCopy.remoteHostLabel}</span>
                            <input
                              value={selectedRemoteDraft.host}
                              onChange={(event) =>
                                updateRemoteDraft(selectedRemoteDraft.id, "host", event.target.value)
                              }
                              placeholder="1.2.3.4"
                            />
                          </label>
                          <label className="remoteServerField">
                            <span>{proxyCopy.remoteSshPortLabel}</span>
                            <input
                              inputMode="numeric"
                              value={selectedRemoteDraft.sshPort}
                              onChange={(event) =>
                                updateRemoteDraft(
                                  selectedRemoteDraft.id,
                                  "sshPort",
                                  event.target.value,
                                )
                              }
                              placeholder={DEFAULT_REMOTE_SSH_PORT}
                            />
                          </label>
                          <label className="remoteServerField">
                            <span>{proxyCopy.remoteUserLabel}</span>
                            <input
                              value={selectedRemoteDraft.sshUser}
                              onChange={(event) =>
                                updateRemoteDraft(
                                  selectedRemoteDraft.id,
                                  "sshUser",
                                  event.target.value,
                                )
                              }
                              placeholder="root"
                            />
                          </label>
                          <label className="remoteServerField">
                            <span>{proxyCopy.remoteDirLabel}</span>
                            <input
                              value={selectedRemoteDraft.remoteDir}
                              onChange={(event) =>
                                updateRemoteDraft(
                                  selectedRemoteDraft.id,
                                  "remoteDir",
                                  event.target.value,
                                )
                              }
                              placeholder="/opt/codex-tools"
                            />
                          </label>
                          <label className="remoteServerField">
                            <span>{proxyCopy.remoteListenPortLabel}</span>
                            <input
                              inputMode="numeric"
                              value={selectedRemoteDraft.listenPort}
                              onChange={(event) =>
                                updateRemoteDraft(
                                  selectedRemoteDraft.id,
                                  "listenPort",
                                  event.target.value,
                                )
                              }
                              placeholder={DEFAULT_REMOTE_LISTEN_PORT}
                            />
                          </label>
                        </div>
                      </div>

                      <div className="remoteServerPanel">
                        <div className="remoteAuthRow">
                          <label className="remoteServerField remoteAuthSelectField">
                            <span>{proxyCopy.remoteAuthLabel}</span>
                            <EditorMultiSelect
                              className="remoteAuthPicker"
                              ariaLabel={proxyCopy.remoteAuthLabel}
                              options={remoteAuthOptions}
                              value={selectedRemoteDraft.authMode}
                              onChange={(next) =>
                                updateRemoteDraft(selectedRemoteDraft.id, "authMode", next)
                              }
                            />
                          </label>

                          <div className="remoteAuthInputArea">
                            {selectedRemoteDraft.authMode === "keyContent" ? (
                              <label className="remoteServerField">
                                <span>{proxyCopy.remotePrivateKeyLabel}</span>
                                <textarea
                                  className="remoteServerTextarea"
                                  value={selectedRemoteDraft.privateKey}
                                  onChange={(event) =>
                                    updateRemoteDraft(
                                      selectedRemoteDraft.id,
                                      "privateKey",
                                      event.target.value,
                                    )
                                  }
                                  placeholder={proxyCopy.remotePrivateKeyPlaceholder}
                                />
                              </label>
                            ) : null}

                            {selectedRemoteDraft.authMode === "password" ? (
                              <label className="remoteServerField">
                                <span>{proxyCopy.remotePasswordLabel}</span>
                                <input
                                  type="password"
                                  value={selectedRemoteDraft.password}
                                  onChange={(event) =>
                                    updateRemoteDraft(
                                      selectedRemoteDraft.id,
                                      "password",
                                      event.target.value,
                                    )
                                  }
                                  placeholder={proxyCopy.remotePasswordPlaceholder}
                                />
                              </label>
                            ) : null}

                            {selectedRemoteDraft.authMode === "keyFile" ||
                            selectedRemoteDraft.authMode === "keyPath" ? (
                              <div className="remoteIdentityRow">
                                <label className="remoteServerField">
                                  <span>{proxyCopy.remoteIdentityFileLabel}</span>
                                  <input
                                    value={selectedRemoteDraft.identityFile}
                                    onChange={(event) =>
                                      updateRemoteDraft(
                                        selectedRemoteDraft.id,
                                        "identityFile",
                                        event.target.value,
                                      )
                                    }
                                    placeholder={proxyCopy.remoteIdentityFilePlaceholder}
                                  />
                                </label>
                                {selectedRemoteDraft.authMode === "keyFile" ? (
                                  <button
                                    className="ghost"
                                    type="button"
                                    onClick={() => {
                                      void onPickLocalIdentityFile().then((value) => {
                                        if (value) {
                                          updateRemoteDraft(
                                            selectedRemoteDraft.id,
                                            "identityFile",
                                            value,
                                          );
                                        }
                                      });
                                    }}
                                  >
                                    {proxyCopy.remotePickIdentityFile}
                                  </button>
                                ) : null}
                              </div>
                            ) : null}
                          </div>
                        </div>
                      </div>

                      {selectedInstallingDependency ? (
                        <div
                          className="remoteDependencyInstall"
                          role="status"
                          aria-live="polite"
                          aria-busy="true"
                        >
                          <div className="remoteDependencyInstallHeader">
                            <strong>{copy.notices.installingDependency("sshpass")}</strong>
                          </div>
                          <div className="remoteDependencyInstallTrack" aria-hidden="true">
                            <span className="remoteDependencyInstallFill" />
                          </div>
                        </div>
                      ) : null}
                    </div>
                  ) : null}

                  <div className="remoteWorkbenchSection">
                    <div className="remoteWorkbenchSectionHeader">
                      <div>
                        <span className="proxyLabel">{proxyCopy.remoteLogsLabel}</span>
                        <strong>{selectedRemoteIdentity}</strong>
                      </div>
                      <div className="remoteWorkbenchSectionActions">
                        <button
                          className="ghost"
                          onClick={toggleSelectedDiagnostics}
                          disabled={selectedReadingLogs}
                        >
                          {diagnosticsOpen
                            ? proxyCopy.remoteCollapse
                            : selectedReadingLogs
                              ? proxyCopy.remoteReadingLogs
                              : proxyCopy.remoteReadLogs}
                        </button>
                      </div>
                    </div>

                    {diagnosticsOpen ? (
                      <div className="remoteDiagnosticsGrid">
                        <article className="proxyDetailCard remoteLogCard">
                          <div className="proxyDetailHeader">
                            <span className="proxyLabel">{proxyCopy.remoteLogsLabel}</span>
                            <button
                              className="ghost proxyCopyButton"
                              onClick={() => copyText(selectedRemoteLog ?? null)}
                              disabled={!selectedRemoteLog}
                            >
                              {proxyCopy.copy}
                            </button>
                          </div>
                          <code className="remoteLogCode">
                            {selectedRemoteLog ?? proxyCopy.remoteLogsEmpty}
                          </code>
                        </article>

                        <article className="proxyDetailCard remoteErrorCard">
                          <span className="proxyLabel">{proxyCopy.remoteLastErrorLabel}</span>
                          <p className="proxyErrorText">
                            {selectedRemoteStatus?.lastError ?? proxyCopy.none}
                          </p>
                        </article>
                      </div>
                    ) : null}
                  </div>
                </div>
              ) : null}
            </div>
          ) : (
            <article className="cloudflaredCallout">
              <strong>{proxyCopy.remoteEmptyTitle}</strong>
              <p>{proxyCopy.remoteEmptyDescription}</p>
            </article>
          )}
        </section>

        <section className="proxySectionCard">
          <div className="proxySectionHeader">
            <h3>{proxyCopy.cloudflaredTitle}</h3>
            <div className="proxySwitchRow proxySectionToggle">
              <div className="settingMeta">
                <strong>{proxyCopy.cloudflaredToggle}</strong>
              </div>
              <label className="themeSwitch" aria-label={proxyCopy.cloudflaredToggle}>
                <input
                  type="checkbox"
                  checked={publicAccessEnabled}
                  onChange={(event) => setPublicAccessEnabled(event.target.checked)}
                />
                <span className="themeSwitchTrack" aria-hidden="true">
                  <span className="themeSwitchThumb" />
                </span>
                <span className="themeSwitchText">
                  {publicAccessEnabled
                    ? proxyCopy.defaultStartEnabled
                    : proxyCopy.defaultStartDisabled}
                </span>
              </label>
            </div>
          </div>

          {cloudflaredEnabled ? (
            <div className="cloudflaredContent">
              {!status.running ? (
                <article className="cloudflaredCallout">
                  <strong>{proxyCopy.startLocalProxyFirstTitle}</strong>
                  <p>{proxyCopy.startLocalProxyFirstDescription}</p>
                </article>
              ) : null}

              {!cloudflaredStatus.installed ? (
                <article className="cloudflaredInstallCard">
                  <div>
                    <span className="proxyLabel">{proxyCopy.notInstalledLabel}</span>
                    <strong>{proxyCopy.installTitle}</strong>
                    <p>{proxyCopy.installDescription}</p>
                  </div>
                  <button
                    className="primary"
                    onClick={onInstallCloudflared}
                    disabled={installingCloudflared}
                  >
                    {installingCloudflared ? proxyCopy.installing : proxyCopy.installButton}
                  </button>
                </article>
              ) : (
                <>
                  <div className="cloudflaredModeGrid">
                    <button
                      className={`cloudflaredModeCard${tunnelMode === "quick" ? " isActive" : ""}`}
                      onClick={() => setTunnelMode("quick")}
                      disabled={cloudflaredBusy || cloudflaredStatus.running}
                    >
                      <span className="proxyLabel">{proxyCopy.quickModeLabel}</span>
                      <strong>{proxyCopy.quickModeTitle}</strong>
                      <p>{proxyCopy.quickModeDescription}</p>
                    </button>
                    <button
                      className={`cloudflaredModeCard${tunnelMode === "named" ? " isActive" : ""}`}
                      onClick={() => setTunnelMode("named")}
                      disabled={cloudflaredBusy || cloudflaredStatus.running}
                    >
                      <span className="proxyLabel">{proxyCopy.namedModeLabel}</span>
                      <strong>{proxyCopy.namedModeTitle}</strong>
                      <p>{proxyCopy.namedModeDescription}</p>
                    </button>
                  </div>

                  {tunnelMode === "quick" ? (
                    <article className="cloudflaredCallout">
                      <strong>{proxyCopy.quickNoteTitle}</strong>
                      <p>{proxyCopy.quickNoteBody}</p>
                    </article>
                  ) : null}

                  {tunnelMode === "named" ? (
                    <div className="cloudflaredFormGrid">
                      <label className="cloudflaredInputField">
                        <span>{proxyCopy.apiTokenLabel}</span>
                        <input
                          type="password"
                          value={namedInput.apiToken}
                          onChange={(event) =>
                            setNamedInput((current) => ({ ...current, apiToken: event.target.value }))
                          }
                          placeholder={proxyCopy.apiTokenPlaceholder}
                          disabled={cloudflaredBusy || cloudflaredStatus.running}
                        />
                      </label>
                      <label className="cloudflaredInputField">
                        <span>{proxyCopy.accountIdLabel}</span>
                        <input
                          value={namedInput.accountId}
                          onChange={(event) =>
                            setNamedInput((current) => ({ ...current, accountId: event.target.value }))
                          }
                          placeholder={proxyCopy.accountIdPlaceholder}
                          disabled={cloudflaredBusy || cloudflaredStatus.running}
                        />
                      </label>
                      <label className="cloudflaredInputField">
                        <span>{proxyCopy.zoneIdLabel}</span>
                        <input
                          value={namedInput.zoneId}
                          onChange={(event) =>
                            setNamedInput((current) => ({ ...current, zoneId: event.target.value }))
                          }
                          placeholder={proxyCopy.zoneIdPlaceholder}
                          disabled={cloudflaredBusy || cloudflaredStatus.running}
                        />
                      </label>
                      <label className="cloudflaredInputField">
                        <span>{proxyCopy.hostnameLabel}</span>
                        <input
                          value={namedInput.hostname}
                          onChange={(event) =>
                            setNamedInput((current) => ({ ...current, hostname: event.target.value }))
                          }
                          placeholder={proxyCopy.hostnamePlaceholder}
                          disabled={cloudflaredBusy || cloudflaredStatus.running}
                        />
                      </label>
                    </div>
                  ) : null}

                  <div className="cloudflaredToolbar">
                    <div className="proxySwitchRow cloudflaredToolbarMeta">
                      <div className="settingMeta">
                        <strong>{proxyCopy.useHttp2}</strong>
                      </div>
                      <label className="themeSwitch" aria-label={proxyCopy.useHttp2}>
                        <input
                          type="checkbox"
                          checked={useHttp2}
                          onChange={(event) => setUseHttp2(event.target.checked)}
                          disabled={cloudflaredBusy || cloudflaredStatus.running}
                        />
                        <span className="themeSwitchTrack" aria-hidden="true">
                          <span className="themeSwitchThumb" />
                        </span>
                        <span className="themeSwitchText">
                          {useHttp2
                            ? proxyCopy.defaultStartEnabled
                            : proxyCopy.defaultStartDisabled}
                        </span>
                      </label>
                    </div>

                    <div className="cloudflaredToolbarActions">
                      <button
                        className="ghost"
                        onClick={onRefreshCloudflared}
                        disabled={cloudflaredBusy}
                      >
                        {proxyCopy.refreshPublicStatus}
                      </button>
                      {cloudflaredStatus.running ? (
                        <button
                          className="danger"
                          onClick={onStopCloudflared}
                          disabled={cloudflaredBusy}
                        >
                          {stoppingCloudflared ? proxyCopy.stoppingPublic : proxyCopy.stopPublic}
                        </button>
                      ) : (
                        <button
                          className="primary"
                          onClick={() => {
                            if (cloudflaredInput) {
                              onStartCloudflared(cloudflaredInput);
                            }
                          }}
                          disabled={!canStartCloudflared || cloudflaredInput === null}
                        >
                          {startingCloudflared ? proxyCopy.startingPublic : proxyCopy.startPublic}
                        </button>
                      )}
                    </div>
                  </div>

                  <div className="proxyDetailGrid">
                    <article className="proxyDetailCard">
                      <span className="proxyLabel">{proxyCopy.publicStatusLabel}</span>
                      <strong className={`proxyStatus${cloudflaredStatus.running ? " isRunning" : ""}`}>
                        {cloudflaredStatus.running
                          ? proxyCopy.publicStatusRunning
                          : proxyCopy.publicStatusStopped}
                      </strong>
                      <p>
                        {cloudflaredStatus.running
                          ? proxyCopy.publicStatusRunningDescription
                          : proxyCopy.publicStatusStoppedDescription}
                      </p>
                    </article>

                    <article className="proxyDetailCard">
                      <div className="proxyDetailHeader">
                        <span className="proxyLabel">{proxyCopy.publicUrlLabel}</span>
                        <button
                          className="ghost proxyCopyButton"
                          onClick={() => copyText(cloudflaredStatus.publicUrl)}
                          disabled={!cloudflaredStatus.publicUrl}
                        >
                          {proxyCopy.copy}
                        </button>
                      </div>
                      <code>{cloudflaredStatus.publicUrl ?? proxyCopy.baseUrlPlaceholder}</code>
                    </article>

                    <article className="proxyDetailCard">
                      <span className="proxyLabel">{proxyCopy.installPathLabel}</span>
                      <code>{cloudflaredStatus.binaryPath ?? proxyCopy.notDetected}</code>
                    </article>

                    <article className="proxyDetailCard">
                      <span className="proxyLabel">{proxyCopy.lastErrorLabel}</span>
                      <p className="proxyErrorText">{cloudflaredStatus.lastError ?? proxyCopy.none}</p>
                    </article>
                  </div>
                </>
              )}
            </div>
          ) : null}
        </section>
      </div>
    </section>
  );
}
