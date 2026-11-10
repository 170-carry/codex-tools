import { ActionMenu } from "../ui/ActionMenu";
import { useI18n } from "../../i18n/I18nProvider";
import { getPageLayoutCopy } from "../../i18n/pageLayoutCopy";
import { PageHeader } from "../ui/PageFrame";
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
  const { locale } = useI18n();
  const layout = getPageLayoutCopy(locale);
  return (
    <PageHeader title={text.title} actions={<>
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
        <ActionMenu label={layout.export} triggerLabel={exporting ? text.exporting : layout.export} disabled={exporting !== null}
          items={[{label: text.exportCsv, action: () => onExport("csv")}, {label: text.exportJson, action: () => onExport("json")}]} />
      </>} />
  );
}
