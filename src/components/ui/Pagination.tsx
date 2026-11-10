import { uiCopy } from "../../i18n/uiCopy";

export function Pagination({ page, pages, size, total, onChange, locale }: { page: number; pages: number; size: number; total: number; onChange: (page: number) => void; locale: string }) {
  const text = uiCopy(locale);
  if (total === 0) return null;
  return <nav className="uiPagination" aria-label={`${text.records} · ${text.previous} / ${text.next}`}>
    <span>{page * size + 1}–{Math.min((page + 1) * size, total)} / {total} {text.records}</span>
    {pages > 1 && <div><button type="button" disabled={page === 0} onClick={() => onChange(page - 1)}>{text.previous}</button><span aria-live="polite">{page + 1} / {pages}</span><button type="button" disabled={page + 1 === pages} onClick={() => onChange(page + 1)}>{text.next}</button></div>}
  </nav>;
}
