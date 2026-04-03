import { useI18n } from "../i18n/I18nProvider";

type MetaStripProps = {
  accountCount: number;
};

export function MetaStrip({ accountCount }: MetaStripProps) {
  const { copy } = useI18n();

  return (
    <section className="metaStrip" aria-label={copy.metaStrip.ariaLabel}>
      <article className="metaPill">
        <span>{copy.metaStrip.accountCount}</span>
        <strong>{accountCount}</strong>
      </article>
    </section>
  );
}
