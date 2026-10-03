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
import type { ApiProxyUsageMetric, ApiProxyUsageRange } from "../../types/app";
import type {
  ApiProxyUsageHoverState,
  ApiProxyUsageContextMenu,
  ApiProxyUsageChartMotion,
  ApiProxyUsageDimension,
  ApiProxyUsageChartProps,
} from "./usageTypes";
import { API_PROXY_USAGE_RANGE_SECONDS } from "./usageConstants";
import { clampContextMenuPosition, resolveUsageHoverState } from "./usageHover";
import { formatUsageTickLabel } from "./usageCurves";
import { buildUsageChartData } from "./buildUsageChartData";
export function useUsageChart({
  copy,
  locale,
  stats,
  range,
  metric,
  loading,
  clearing,
  exporting,
  proxyRunning,
  apiProxyKeys,
  onSelectRange,
  onSelectMetric,
  onExport,
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
  const margins = useMemo(
    () => ({ top: 18, right: 44, bottom: 46, left: 56 }),
    [],
  );
  const plotWidth = chartWidth - margins.left - margins.right;
  const plotHeight = chartHeight - margins.top - margins.bottom;
  const frameRef = useRef<HTMLDivElement | null>(null);
  const cardRef = useRef<HTMLElement | null>(null);
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [dimension, setDimension] = useState<ApiProxyUsageDimension>("model");
  const [exportKeyId, setExportKeyId] = useState("");
  const [hoverState, setHoverState] = useState<ApiProxyUsageHoverState | null>(
    null,
  );
  const [contextMenu, setContextMenu] =
    useState<ApiProxyUsageContextMenu | null>(null);
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
  const exportKeyOptions = useMemo(() => {
    const options = new Map<string, string>();
    // 以当前统计范围为主，确保已删除或停用但仍有历史记录的 Key 也可以导出。
    for (const key of stats?.keySeries ?? []) {
      if (key.keyId) {
        options.set(key.keyId, key.keyLabel || key.keyId);
      }
    }
    // 同时补入当前 Key，便于在尚无历史记录时预先选择过滤条件。
    for (const key of apiProxyKeys) {
      if (!options.has(key.id)) {
        options.set(key.id, key.label || key.id);
      }
    }
    return [...options].map(([id, label]) => ({ id, label }));
  }, [apiProxyKeys, stats?.keySeries]);

  const effectiveExportKeyId = exportKeyOptions.some(
    (key) => key.id === exportKeyId,
  )
    ? exportKeyId
    : "";

  const chartData = useMemo(
    () =>
      buildUsageChartData({
        dimension,
        metric,
        margins,
        plotHeight,
        plotWidth,
        selectedRangeSeconds,
        stats,
      }),
    [
      dimension,
      metric,
      margins,
      plotHeight,
      plotWidth,
      selectedRangeSeconds,
      stats,
    ],
  );

  const series = chartData.series;
  const bucketSeconds =
    chartData.bucketSeconds ?? Math.max(1, selectedRangeSeconds);
  const hasUsageData = chartData.maxValue > 0 && series.length > 0;
  const endTimestamp = chartData.endTimestamp;
  const startTimestamp = endTimestamp - selectedRangeSeconds;
  const metricLabel = metric === "calls" ? copy.chartCalls : copy.chartTokens;
  const xTicks = [0, 0.25, 0.5, 0.75, 1].map((fraction) => {
    const timestamp = Math.round(
      endTimestamp - selectedRangeSeconds + selectedRangeSeconds * fraction,
    );
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
    ? [
        ...new Set(
          [0, 0.25, 0.5, 0.75, 1].map((fraction) =>
            Math.round(yDomain * fraction),
          ),
        ),
      ]
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
  }, [
    endTimestamp,
    hasUsageData,
    metric,
    plotWidth,
    range,
    selectedRangeSeconds,
  ]);

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
        endTimestamp,
        rangeSeconds: selectedRangeSeconds,
        bucketSeconds,
        metric,
        metricLabel,
      });

      setHoverState(next);
    },
    [
      bucketSeconds,
      chartHeight,
      chartWidth,
      endTimestamp,
      hasUsageData,
      locale,
      margins,
      metric,
      metricLabel,
      range,
      selectedRangeSeconds,
      series,
      startTimestamp,
    ],
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
    if (clearing || exporting) {
      return;
    }
    setContextMenu(null);
    void onClear();
  }, [clearing, exporting, onClear]);

  const handleContextMenuClick = useCallback(
    (event: ReactMouseEvent<HTMLButtonElement>) => {
      event.preventDefault();
      event.stopPropagation();
      handleClearUsage();
    },
    [handleClearUsage],
  );

  return {
    copy,
    locale,
    range,
    metric,
    loading,
    clearing,
    exporting,
    proxyRunning,
    onSelectRange,
    onSelectMetric,
    onExport,
    rangeOptions,
    metricOptions,
    chartWidth,
    chartHeight,
    margins,
    plotWidth,
    plotHeight,
    frameRef,
    cardRef,
    svgRef,
    dimension,
    setDimension,
    setExportKeyId,
    hoverState,
    contextMenu,
    chartMotion,
    exportKeyOptions,
    effectiveExportKeyId,
    series,
    hasUsageData,
    xTicks,
    yTicks,
    updatedLabel,
    chartMotionClass,
    chartMotionStyle,
    handlePointerMove,
    handlePointerLeave,
    handleOpenContextMenu,
    handleContextMenuClick,
  };
}
export type UsageChartWorkspace = ReturnType<typeof useUsageChart>;
