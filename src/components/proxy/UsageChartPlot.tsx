import { formatUsageAxisValue } from "./usageFormat";
import type { UsageChartWorkspace } from "./useUsageChart";
export function UsageChartPlot({ chart }: { chart: UsageChartWorkspace }) {
  const {
    copy,
    metric,
    loading,
    clearing,
    chartWidth,
    chartHeight,
    margins,
    plotWidth,
    plotHeight,
    frameRef,
    svgRef,
    hoverState,
    chartMotion,
    series,
    hasUsageData,
    xTicks,
    yTicks,
    chartMotionClass,
    chartMotionStyle,
    handlePointerMove,
    handlePointerLeave,
    handleOpenContextMenu,
  } = chart;
  return (
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
                <rect
                  x={margins.left}
                  y={margins.top}
                  width={plotWidth}
                  height={plotHeight}
                  rx="14"
                />
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
                  <stop
                    offset="100%"
                    stopColor={item.color}
                    stopOpacity="0.02"
                  />
                </linearGradient>
              ))}
            </defs>

            <rect
              className="proxyUsagePlotBg"
              x={margins.left}
              y={margins.top}
              width={plotWidth}
              height={plotHeight}
              rx="14"
            />

            <g className="proxyUsageGrid">
              {yTicks.map((tick) => (
                <g key={`y-${tick.value}`}>
                  <line
                    x1={margins.left}
                    x2={margins.left + plotWidth}
                    y1={tick.y}
                    y2={tick.y}
                  />
                  <text x={margins.left - 10} y={tick.y + 4} textAnchor="end">
                    {formatUsageAxisValue(tick.value)}
                  </text>
                </g>
              ))}
              {xTicks.map((tick, index) => (
                <g key={`x-${tick.timestamp}-${index}`}>
                  <line
                    x1={tick.x}
                    x2={tick.x}
                    y1={margins.top}
                    y2={margins.top + plotHeight}
                    className="proxyUsageGridLineVertical"
                  />
                  <text
                    x={tick.x}
                    y={margins.top + plotHeight + 24}
                    textAnchor={tick.anchor}
                  >
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
                  <g key={item.gradientId} className="proxyUsageSeries">
                    <path
                      className="proxyUsageArea"
                      d={item.areaPath}
                      fill={`url(#${item.gradientId})`}
                    />
                    <path
                      className="proxyUsageLine"
                      d={item.linePath}
                      stroke={item.color}
                    />
                  </g>
                ))}
              </g>
              {hoverState ? (
                <g className="proxyUsageHoverLayer" pointerEvents="none">
                  <rect
                    className="proxyUsageHoverBand"
                    x={hoverState.bucketStartX}
                    y={margins.top}
                    width={Math.max(
                      1,
                      hoverState.bucketEndX - hoverState.bucketStartX,
                    )}
                    height={plotHeight}
                  />
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
                <span className="proxyUsageTooltipTime">
                  {hoverState.timeLabel}
                </span>
                <span className="proxyUsageTooltipMetricLabel">
                  {hoverState.metricLabel}
                </span>
              </div>
              <div className="proxyUsageTooltipEntries">
                {hoverState.entries.map((entry) => (
                  <div
                    className="proxyUsageTooltipEntry"
                    key={`${entry.model}-${entry.value}`}
                  >
                    <span
                      className="proxyUsageTooltipSwatch"
                      style={{ background: entry.color }}
                    />
                    <span
                      className="proxyUsageTooltipEntryModel"
                      title={entry.model}
                    >
                      {entry.model}
                    </span>
                    <strong className="proxyUsageTooltipEntryValue">
                      {entry.valueLabel}
                    </strong>
                  </div>
                ))}
              </div>
            </div>
          ) : null}
          {loading ? (
            <div className="proxyUsageFrameBadge">{copy.chartLoadingTitle}</div>
          ) : null}
        </>
      ) : (
        <div
          className="proxyUsageState"
          aria-live="polite"
          aria-busy={loading || clearing}
        >
          <span
            className={`proxyUsageStateOrb${loading ? " isLoading" : ""}`}
          />
          <strong>
            {loading ? copy.chartLoadingTitle : copy.chartEmptyTitle}
          </strong>
          <p>
            {loading
              ? copy.chartLoadingDescription
              : copy.chartEmptyDescription}
          </p>
        </div>
      )}
    </div>
  );
}
