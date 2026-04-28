import { useI18n } from "../i18n/I18nProvider";
import type { CodexTokenUsageSnapshot } from "../types/app";
import { formatTokenCount } from "../utils/usage";

type MetaStripProps = {
  accountCount: number;
  tokenUsage: CodexTokenUsageSnapshot | null;
  tokenUsageError: string | null;
  exportingAccounts: boolean;
  onExportAccounts: () => void;
};

export function MetaStrip({
  accountCount,
  tokenUsage,
  tokenUsageError,
  exportingAccounts,
  onExportAccounts,
}: MetaStripProps) {
  const { copy, locale } = useI18n();
  const tokenMetrics = [
    {
      label: copy.metaStrip.tokensSession,
      value: tokenUsage?.latestSession?.total.totalTokens ?? null,
    },
    {
      label: copy.metaStrip.tokens24h,
      value: tokenUsage?.last24h.totalTokens ?? null,
    },
    {
      label: copy.metaStrip.tokens7d,
      value: tokenUsage?.last7d.totalTokens ?? null,
    },
    {
      label: copy.metaStrip.tokens30d,
      value: tokenUsage?.last30d.totalTokens ?? null,
    },
  ];

  const tokenTitle = tokenUsageError
    ? tokenUsageError
    : tokenUsage
      ? [
          `${copy.metaStrip.tokensUpdatedAt}: ${new Date(
            tokenUsage.updatedAt * 1000,
          ).toLocaleString(locale)}`,
          `${copy.metaStrip.tokensSources}: ${tokenUsage.sourcePathCount}`,
          `${copy.metaStrip.tokensEvents}: ${tokenUsage.eventCount}`,
          tokenUsage.failedPathCount > 0
            ? `${copy.metaStrip.tokensFailedSources}: ${tokenUsage.failedPathCount}`
            : null,
        ]
          .filter(Boolean)
          .join("\n")
      : copy.metaStrip.tokensPending;

  return (
    <section className="metaStrip" aria-label={copy.metaStrip.ariaLabel}>
      <article className="metaPill">
        <span>{copy.metaStrip.accountCount}</span>
        <strong>{accountCount}</strong>
      </article>
      {tokenMetrics.map((metric) => (
        <article
          key={metric.label}
          className={`metaPill tokenMetaPill${tokenUsageError ? " hasError" : ""}`}
          title={tokenTitle}
        >
          <span>{metric.label}</span>
          <strong className="metaPillMono">
            {tokenUsageError ? "--" : formatTokenCount(metric.value, locale)}
          </strong>
        </article>
      ))}
      <button
        className="ghost metaExportButton"
        onClick={onExportAccounts}
        disabled={exportingAccounts || accountCount === 0}
        aria-label={copy.metaStrip.exportAll}
      >
        {copy.metaStrip.exportAll}
      </button>
    </section>
  );
}
