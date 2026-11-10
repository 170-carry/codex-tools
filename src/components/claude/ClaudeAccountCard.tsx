import { claudeText } from "../../i18n/claudeCopy";
import type { ClaudeAccount } from "./types";
import { useI18n } from "../../i18n/I18nProvider";
import { getUiCopy } from "../accounts/accountCopy";
import { AccountListRow } from "../ui/AccountListRow";
import { ActionMenu } from "../ui/ActionMenu";
import { UsageMeter } from "../classic/ClassicUsageMeter";
import { uiCopy } from "../../i18n/uiCopy";
import type { UsageWindow } from "../../types/app";

function claudeWindow(a: ClaudeAccount, key: string): UsageWindow | null {
  const usage = a.usage?.[key];
  return usage ? { usedPercent: usage.utilization, windowSeconds: key === "five_hour" ? 18000 : 604800, resetAt: usage.resets_at ? new Date(usage.resets_at).getTime() / 1000 : null } : null;
}
export function ClaudeAccountCard({ a, busy, run, onReauthorize, onDelete, onInspect = () => {}, selected = false }: {
  a: ClaudeAccount; busy: boolean; run: (command: string, id: string) => Promise<void>; onReauthorize: (a: ClaudeAccount) => void; onDelete: (a: ClaudeAccount) => void; onInspect?: () => void; selected?: boolean;
}) {
  const { locale } = useI18n();
  const ct = claudeText(locale);
  const text = getUiCopy(locale);
  const ui = uiCopy(locale);
  const status = a.sync?.needsAuth || a.sync?.error ? "issue" : a.isCurrent ? "using" : "available";
  const stateLabel = status === "issue" ? text.statusIssue : a.isCurrent ? text.statusUsing : (ct("Saved"));
  const items = [
    { label: ui.details, action: onInspect },
    ...(a.kind !== "api" ? [
      { label: ct("Refresh usage"), disabled: busy, action: () => void run("claude_refresh_usage", a.id) },
      { label: ct("Reauthorize"), disabled: busy, action: () => onReauthorize(a) },
    ] : []),
    { label: ct("Delete"), danger: true, disabled: busy, action: () => onDelete(a) },
  ];
  return <AccountListRow current={a.isCurrent} selected={selected} status={status} onInspect={onInspect}>
    <div className="accountIdentityCell"><div className="accountTitleLine"><span className="planChip">{a.kind === "api" ? "API" : (a.plan?.replace(/^claude_/, "").toUpperCase() ?? "CLAUDE")}</span><button type="button" className="accountNameButton" title={a.email ?? a.label} onClick={event => {event.stopPropagation(); onInspect();}}>{a.label}</button></div><div className="accountStateLine"><span className={`statusText status-${status}`}><span className="statusDot" />{stateLabel}</span><span className="accountSource">{a.kind === "desktop" ? "Claude Desktop" : a.kind === "api" ? "API" : "Claude Code"}</span></div></div>
    <UsageMeter className="accountUsageFive" label={text.fiveHourUsage} windowLabel="5h" window={claudeWindow(a, "five_hour")} text={text} locale={locale} />
    <UsageMeter className="accountUsageWeek" label={text.weekUsage} windowLabel="1w" window={claudeWindow(a, "seven_day")} text={text} locale={locale} />
    <div className="rowActions" onClick={event => event.stopPropagation()}><button type="button" className="rowSwitchButton" disabled={busy || a.isCurrent} onClick={() => void run(a.kind === "desktop" ? "claude_switch_desktop_account" : "claude_switch_account", a.id)}>{a.isCurrent ? text.statusUsing : text.switchAccount}</button><ActionMenu label={`${a.label} · ${ui.more}`} items={items} /></div>
  </AccountListRow>;
}
