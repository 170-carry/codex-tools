import type { ApiProxyUsageChartProps } from "./usageTypes";
import { formatUsageMetricValue } from "./usageFormat";
import { useUsageChart } from "./useUsageChart";
import { UsageChartControls } from "./UsageChartControls";
import { UsageChartPlot } from "./UsageChartPlot";
export function ApiProxyUsageChart(props: ApiProxyUsageChartProps) {
  const chart = useUsageChart(props);
  const {
    copy,
    locale,
    metric,
    clearing,
    exporting,
    proxyRunning,
    cardRef,
    contextMenu,
    series,
    hasUsageData,
    handleOpenContextMenu,
    handleContextMenuClick,
  } = chart;
  return (
    <section
      ref={cardRef}
      className={`proxySectionCard proxyUsageCard${proxyRunning ? " isRunning" : ""}`}
      onContextMenu={handleOpenContextMenu}
    >
      <UsageChartControls chart={chart} />

      <UsageChartPlot chart={chart} />

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
            disabled={clearing || exporting}
            onClick={handleContextMenuClick}
          >
            {copy.chartClearHistory}
          </button>
        </div>
      ) : null}

      {hasUsageData ? (
        <div className="proxyUsageLegend" aria-label={copy.chartTitle}>
          {series.map((item) => {
            const primary =
              metric === "calls" ? item.totalCalls : item.totalTokens;
            const secondary =
              metric === "calls" ? item.totalTokens : item.totalCalls;
            return (
              <article key={item.gradientId} className="proxyUsageLegendItem">
                <span
                  className="proxyUsageLegendSwatch"
                  style={{ background: item.color }}
                  aria-hidden="true"
                />
                <div className="proxyUsageLegendBody">
                  <strong title={item.model}>{item.model}</strong>
                  <span>
                    {formatUsageMetricValue(primary, locale)}{" "}
                    {metric === "calls" ? copy.chartCalls : copy.chartTokens}
                  </span>
                  <small>
                    {formatUsageMetricValue(secondary, locale)}{" "}
                    {metric === "calls" ? copy.chartTokens : copy.chartCalls}
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
