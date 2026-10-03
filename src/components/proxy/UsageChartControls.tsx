import type { UsageChartWorkspace } from "./useUsageChart";
export function UsageChartControls({ chart }: { chart: UsageChartWorkspace }) {
  const {
    copy,
    range,
    metric,
    clearing,
    exporting,
    proxyRunning,
    onSelectRange,
    onSelectMetric,
    onExport,
    rangeOptions,
    metricOptions,
    dimension,
    setDimension,
    setExportKeyId,
    exportKeyOptions,
    effectiveExportKeyId,
    updatedLabel,
  } = chart;
  return (
    <>
      <div className="proxyUsageHeader">
        <div className="proxyUsageHeading">
          <span className="proxyLabel">{copy.chartKicker}</span>
          <h3>{copy.chartTitle}</h3>
        </div>
        <div className="proxyUsageHeaderMeta">
          <span
            className={`proxyHeaderStat proxyUsageStatus${proxyRunning ? " isRunning" : ""}`}
          >
            <span
              className={`proxyStatusDot${proxyRunning ? " isRunning" : ""}`}
              aria-hidden="true"
            />
            <span>{copy.chartKicker}</span>
            <strong>
              {proxyRunning ? copy.statusRunning : copy.statusStopped}
            </strong>
          </span>
          {updatedLabel ? (
            <span className="proxyUsageUpdated">
              {copy.chartUpdatedAt}: {updatedLabel}
            </span>
          ) : null}
        </div>
      </div>
      <div className="proxyUsageControls">
        <div
          className="proxyUsageGroup"
          role="group"
          aria-label={copy.chartDimensionLabel}
        >
          {(
            [
              { value: "model", label: copy.chartByModel },
              { value: "key", label: copy.chartByKey },
            ] as const
          ).map((option) => (
            <button
              key={option.value}
              type="button"
              className={`proxyUsageChip${dimension === option.value ? " isActive" : ""}`}
              aria-pressed={dimension === option.value}
              onClick={() => setDimension(option.value)}
            >
              {option.label}
            </button>
          ))}
        </div>

        <div
          className="proxyUsageGroup"
          role="group"
          aria-label={copy.chartRangeLabel}
        >
          {rangeOptions.map((option) => (
            <button
              key={option.value}
              type="button"
              className={`proxyUsageChip${range === option.value ? " isActive" : ""}`}
              aria-pressed={range === option.value}
              onClick={() => {
                if (option.value !== range) {
                  setExportKeyId("");
                }
                onSelectRange(option.value);
              }}
            >
              {option.label}
            </button>
          ))}
        </div>

        <div
          className="proxyUsageGroup"
          role="group"
          aria-label={copy.chartMetricLabel}
        >
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

        <div className="proxyUsageExportGroup">
          <label className="proxyUsageExportPicker">
            <span>{copy.chartExportKeyLabel}</span>
            <select
              value={effectiveExportKeyId}
              disabled={exporting || clearing}
              aria-label={copy.chartExportKeyLabel}
              onChange={(event) => setExportKeyId(event.currentTarget.value)}
            >
              <option value="">{copy.chartExportAllKeys}</option>
              {exportKeyOptions.map((key) => (
                <option key={key.id} value={key.id}>
                  {key.label || key.id}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            className="ghost proxyUsageExportButton"
            disabled={exporting || clearing}
            onClick={() => void onExport(effectiveExportKeyId || null)}
          >
            {exporting ? copy.chartExporting : copy.chartExportCsv}
          </button>
        </div>
      </div>
    </>
  );
}
