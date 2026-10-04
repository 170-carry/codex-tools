import type { CodexTokenUsageSnapshot } from "../../types/app";
import type { UiCopy } from "../accounts/types";
import { formatTokenCount } from "../../utils/usage";
import { ActionIcon } from "./ClassicIcons";
export function TokenUsageStrip({
  tokenUsage,
  tokenUsageError,
  locale,
  text,
  accountCount,
  exportingAccounts,
  onExportAll,
}: {
  tokenUsage: CodexTokenUsageSnapshot | null;
  tokenUsageError: string | null;
  locale: string;
  text: UiCopy;
  accountCount: number;
  exportingAccounts: boolean;
  onExportAll: () => void;
}) {
  const items = [
    { label: "24H", value: tokenUsage?.last24h.totalTokens },
    { label: "3D", value: tokenUsage?.last3d.totalTokens },
    { label: "7D", value: tokenUsage?.last7d.totalTokens },
    { label: "30D", value: tokenUsage?.last30d.totalTokens },
  ];

  return (
    <section
      className="accountTokenUsageStrip"
      aria-label={text.tokenUsageTitle}
    >
      <strong>{text.tokenUsageTitle}</strong>
      <div className="accountTokenUsageItems">
        {items.map((item) => (
          <span key={item.label} className="accountTokenUsageItem">
            <em>{item.label}</em>
            <b>{formatTokenCount(item.value, locale)}</b>
          </span>
        ))}
      </div>
      <div className="accountTokenUsageActions">
        {tokenUsageError ? (
          <span className="accountTokenUsageError">{text.tokenUsageError}</span>
        ) : null}
        <button
          className="ghost accountTokenExportButton"
          type="button"
          onClick={onExportAll}
          disabled={exportingAccounts || accountCount === 0}
          aria-label={text.exportAll}
        >
          <ActionIcon type="export" />
          <span>{text.exportAll}</span>
        </button>
      </div>
    </section>
  );
}
