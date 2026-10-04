import type { AnalyticsCopy } from "../analytics/types";
export function ClassicAnalyticsHeader({
  text,
  loading,
  exporting,
  onRefresh,
  onExport,
}: {
  text: AnalyticsCopy;
  loading: boolean;
  exporting: "csv" | "json" | null;
  onRefresh?: () => void;
  onExport: (format: "csv" | "json") => void;
}) {
  return (
    <header className="analyticsHeader">
      <div>
        <span className="analyticsKicker">{text.kicker}</span>
        <h2>{text.title}</h2>
        <p>{text.description}</p>
      </div>
      <div className="analyticsActions">
        {onRefresh ? (
          <button
            type="button"
            className="ghost"
            disabled={loading}
            onClick={onRefresh}
          >
            {text.refresh}
          </button>
        ) : null}
        <button
          type="button"
          className="ghost"
          disabled={exporting !== null}
          onClick={() => onExport("csv")}
        >
          {exporting === "csv" ? text.exporting : text.exportCsv}
        </button>
        <button
          type="button"
          className="primary"
          disabled={exporting !== null}
          onClick={() => onExport("json")}
        >
          {exporting === "json" ? text.exporting : text.exportJson}
        </button>
      </div>
    </header>
  );
}
