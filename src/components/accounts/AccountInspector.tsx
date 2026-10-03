import { useLayoutEffect, useRef, useState } from "react";
import type { AccountSummary } from "../../types/app";
import type { AccountsGridProps, SwitchRecord, UiCopy } from "./types";
import { useI18n } from "../../i18n/I18nProvider";
import { accountInitial, displayAccountAddress } from "./accountPresentation";
import { accountIssueReason, accountStatus } from "./accountModel";
import { statusLabel } from "./accountCopy";
import { formatFullDate } from "../../utils/dateFormatting";
import { formatPlan } from "../../utils/usage";
import { MembershipExpiry } from "./MembershipExpiry";
import { ResetCreditsSection } from "./ResetCreditsSection";
import { QuotaMeter } from "./QuotaMeter";
import { UsageFreshnessBadge } from "./UsageFreshnessBadge";
import { AccountActionMenu } from "./AccountActionMenu";
import { WorkspaceIcon } from "../workspace/WorkspaceIcon";

export function AccountInspector({
  account,
  actions,
  text,
  records,
  onClose,
  onSwitch,
}: {
  account: AccountSummary;
  actions: AccountsGridProps;
  text: UiCopy;
  records: SwitchRecord[];
  onClose: () => void;
  onSwitch: (account: AccountSummary) => Promise<void>;
}) {
  const { copy, locale } = useI18n();
  const dialog = useRef<HTMLDialogElement>(null);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(account.label);
  const [expandedCredits, setExpandedCredits] = useState(false);
  const status = accountStatus(account);
  const inspectorActions = {
    ...actions,
    onDelete: (target: AccountSummary) => {
      onClose();
      actions.onDelete(target);
    },
    onReauthorize: (target: AccountSummary) => {
      onClose();
      actions.onReauthorize(target);
    },
  };
  const saving = actions.renamingAccountId === account.accountKey;
  useLayoutEffect(() => {
    const node = dialog.current;
    if (!node) return;
    const previousFocus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    node.showModal();
    return () => {
      node.close();
      if (previousFocus?.isConnected) previousFocus.focus();
    };
  }, []);
  const save = async () => {
    const label = draft.trim();
    if (!label) return;
    if (label === account.label || (await actions.onRename(account, label)))
      setEditing(false);
  };
  const address = displayAccountAddress(account, text.emptyValue);
  return (
    <dialog
      ref={dialog}
      className="accountInspector"
      aria-labelledby="inspector-title"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="inspectorContent">
        <header className="inspectorToolbar">
          <span>{text.detailsTitle}</span>
          <button
            type="button"
            className="toolbarIconButton"
            onClick={onClose}
            aria-label={locale === "zh-CN" ? "关闭详情" : "Close details"}
          >
            <WorkspaceIcon name="close" />
          </button>
        </header>
        <div className="inspectorBody">
          <div className="detailIdentity">
            <span className="accountAvatar detailAvatar">
              {accountInitial(account)}
            </span>
            <div>
              <h2 id="inspector-title">{account.label}</h2>
              <span className={`statusText status-${status}`}>
                <span className="statusDot" />
                {statusLabel(status, text)}
                {account.isCurrent && status !== "using"
                  ? ` · ${text.statusUsing}`
                  : ""}
              </span>
            </div>
          </div>
          <button
            type="button"
            className="inspectorAddress"
            title={locale === "zh-CN" ? "复制账号地址" : "Copy account address"}
            onClick={() =>
              void navigator.clipboard.writeText(address).catch(() => {})
            }
          >
            {address}
            <WorkspaceIcon name="export" />
          </button>
          {editing ? (
            <form
              className="detailAliasEditor"
              onSubmit={(event) => {
                event.preventDefault();
                void save();
              }}
            >
              <input
                aria-label={text.edit}
                autoFocus
                value={draft}
                onChange={(event) => setDraft(event.currentTarget.value)}
                disabled={saving}
              />
              <button type="submit" disabled={saving || !draft.trim()}>
                {text.save}
              </button>
              <button type="button" onClick={() => setEditing(false)}>
                {text.cancel}
              </button>
            </form>
          ) : (
            <button
              type="button"
              className="detailEditButton"
              onClick={() => {
                setDraft(account.label);
                setEditing(true);
              }}
            >
              <WorkspaceIcon name="edit" />
              {text.edit}
            </button>
          )}
          {accountIssueReason(account, text.issueFallbackReason) ? (
            <p className="inspectorIssue">
              {accountIssueReason(account, text.issueFallbackReason)}
            </p>
          ) : null}
          <section className="inspectorSection">
            <h3>{text.usageOverview}</h3>
            <UsageFreshnessBadge
              account={account}
              refreshing={actions.usageRefreshing}
              showInitialRefresh={actions.showInitialUsageRefresh}
              refreshError={actions.usageRefreshError}
              locale={locale}
              copy={copy.accountsGrid}
            />
            <QuotaMeter
              label={text.fiveHourUsage}
              window={account.usage?.fiveHour ?? null}
              locale={locale}
            />
            <QuotaMeter
              label={text.weekUsage}
              window={account.usage?.oneWeek ?? null}
              locale={locale}
            />
          </section>
          <section className="inspectorSection detailMetaGrid">
            <div>
              <span>{text.planType}</span>
              <strong>
                {formatPlan(
                  account.planType || account.usage?.planType,
                  copy.accountCard.planLabels,
                )}
              </strong>
            </div>
            <MembershipExpiry
              account={account}
              locale={locale}
              authBusy={actions.authBusy}
              reauthorizeLabel={text.reauthorize}
              onReauthorize={inspectorActions.onReauthorize}
            />
            <label className="inspectorProxyRow">
              <span>{copy.accountCard.apiProxyToggle}</span>
              <input
                type="checkbox"
                checked={account.apiProxyEnabled}
                disabled={actions.authBusy}
                onChange={(event) =>
                  void actions.onToggleApiProxy(
                    account,
                    event.currentTarget.checked,
                  )
                }
              />
            </label>
          </section>
          <ResetCreditsSection
            account={account}
            expanded={expandedCredits}
            locale={locale}
            text={text}
            onToggle={() => setExpandedCredits(!expandedCredits)}
          />
          <details className="inspectorHistory">
            <summary>
              {text.recentSwitches}
              <WorkspaceIcon name="chevron" />
            </summary>
            {records.length ? (
              <ul className="recentSwitchList">
                {records.map((record) => (
                  <li key={record.id}>
                    <strong>{record.target}</strong>
                    <time>
                      {formatFullDate(
                        record.timestamp,
                        locale,
                        text.emptyValue,
                      )}
                    </time>
                    <span>
                      {text.fromPrefix} {record.source}
                    </span>
                  </li>
                ))}
              </ul>
            ) : (
              <p>{text.noSwitchRecords}</p>
            )}
          </details>
        </div>
        <footer className="inspectorFooter">
          <AccountActionMenu
            account={account}
            actions={inspectorActions}
            text={text}
            onInspect={() =>
              dialog.current
                ?.querySelector<HTMLButtonElement>(".detailEditButton")
                ?.focus()
            }
          />
          <button
            type="button"
            className="primary"
            disabled={actions.authBusy || account.isCurrent}
            onClick={() => void onSwitch(account)}
          >
            {account.isCurrent
              ? text.statusUsing
              : actions.switchingId === account.id
                ? copy.accountCard.launching
                : text.switchAccount}
          </button>
        </footer>
      </div>
    </dialog>
  );
}
