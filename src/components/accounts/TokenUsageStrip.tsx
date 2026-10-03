import { useI18n } from "../../i18n/I18nProvider";
import type { CodexTokenUsageSnapshot } from "../../types/app";
import { formatTokenCount } from "../../utils/usage";
import type { UiCopy } from "./types";
import { WorkspaceIcon } from "../workspace/WorkspaceIcon";

export function TokenUsageStrip({
  tokenUsage,
  error,
  text,
}: {
  tokenUsage: CodexTokenUsageSnapshot | null;
  error: string | null;
  text: UiCopy;
}) {
  const { locale } = useI18n();
  const periods = [
    { label: "24H", value: tokenUsage?.last24h.totalTokens },
    { label: "3D", value: tokenUsage?.last3d.totalTokens },
    { label: "7D", value: tokenUsage?.last7d.totalTokens },
    { label: "30D", value: tokenUsage?.last30d.totalTokens },
  ];
  return (
    <details className="tokenUsageDisclosure">
      <summary>
        <WorkspaceIcon name="analytics" />
        {text.tokenUsageTitle}
        <span>
          24H · {formatTokenCount(tokenUsage?.last24h.totalTokens, locale)}
        </span>
        <WorkspaceIcon name="chevron" />
      </summary>
      <div className="accountTokenUsageItems">
        {periods.map((period) => (
          <span key={period.label}>
            <small>{period.label}</small>
            <strong>{formatTokenCount(period.value, locale)}</strong>
          </span>
        ))}
      </div>
      {error ? (
        <p className="accountTokenUsageError" title={error}>
          {text.tokenUsageError}
        </p>
      ) : null}
    </details>
  );
}
