import { claudeText } from "../../i18n/claudeCopy";
import type { ClaudeAccount } from "./types";
import { useI18n } from "../../i18n/I18nProvider";
export function ClaudePageHeader({ accounts }: { accounts: ClaudeAccount[] }) {
  const { locale } = useI18n(); const ct = claudeText(locale);
  return <div className="accountListLeading"><section className="metaStrip" aria-label={ct("Account overview")}>{[
    [ct("Accounts"), accounts.length],
    [ct("In use"), accounts.filter(a => a.isCurrent).length],
    [ct("Needs attention"), accounts.filter(a => a.sync?.needsAuth).length],
  ].map(([label, value]) => <article className="metaPill" key={label}><span className="metricCopy"><span>{label}</span><strong>{value}</strong></span></article>)}</section></div>;
}
export function ClaudeEmptyState({ filtered, onAdd }: { filtered: boolean; onAdd: () => void }) {
  const { locale } = useI18n(); const ct = claudeText(locale);
  return <div className="emptyState accountEmptyState"><h3>{filtered ? (ct("No matching accounts")) : (ct("Add your first Claude account"))}</h3><p>{filtered ? (ct("Try a different name or email.")) : (ct("Authorize an account or import your desktop login to view usage here."))}</p>{!filtered && <button className="primary" onClick={onAdd}>{ct("Add account")}</button>}</div>;
}
