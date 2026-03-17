import { useI18n } from "../i18n/I18nProvider";

type MetaStripProps = {
  accountCount: number;
  exportingAccounts: boolean;
  onExportAccounts: () => void;
};

export function MetaStrip({
  accountCount,
  exportingAccounts,
  onExportAccounts,
}: MetaStripProps) {
  const { copy } = useI18n();

  return (
    <section className="metaStrip" aria-label={copy.metaStrip.ariaLabel}>
      <article className="metaPill">
        <span>{copy.metaStrip.accountCount}</span>
        <strong>{accountCount}</strong>
      </article>
      <button
        className="ghost metaExportButton"
        onClick={onExportAccounts}
        disabled={exportingAccounts}
        aria-label={copy.addAccount.exportButton}
      >
        {copy.addAccount.exportButton}
      </button>
    </section>
  );
}
