import { useId } from "react";
import { createPortal } from "react-dom";
import { useAnchoredMenu } from "../../hooks/useAnchoredMenu";
import { useI18n } from "../../i18n/I18nProvider";
import { getCompactTableCopy } from "../../i18n/compactTableCopy";
import type { AccountsGridProps } from "./types";
import {
  accountHasBlockingIssue,
  accountHasExhaustedWindow,
} from "./accountModel";
import { WorkspaceIcon } from "../workspace/WorkspaceIcon";

export function AccountsMoreMenu({ actions }: { actions: AccountsGridProps }) {
  const { copy, locale } = useI18n();
  const text = getCompactTableCopy(locale);
  const {
    open,
    toggle,
    close,
    triggerRef,
    menuRef,
    position,
    portalContainer,
  } = useAnchoredMenu();
  const id = useId();
  const issues = actions.accounts.filter(accountHasBlockingIssue).length;
  const exhausted = actions.accounts.filter(
    (account) =>
      !accountHasBlockingIssue(account) && accountHasExhaustedWindow(account),
  ).length;
  const available = actions.accounts.filter(
    (account) =>
      !accountHasBlockingIssue(account) &&
      !accountHasExhaustedWindow(account) &&
      account.profileAuthReady,
  ).length;
  const chinese = locale === "zh-CN";
  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        className="accountsMoreButton"
        onClick={toggle}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
      >
        {text.more}
        <WorkspaceIcon name="more" />
      </button>
      {open && portalContainer
        ? createPortal(
            <div
              ref={menuRef}
              id={id}
              className="accountActionMenu accountsMoreMenu"
              role="menu"
              aria-label={text.more}
              style={position}
            >
              <div className="accountsMenuOverview" role="presentation">
                <strong>{text.overview}</strong>
                <span>
                  {available} {chinese ? "可用" : "available"} · {exhausted}{" "}
                  {chinese ? "已耗尽" : "exhausted"} · {issues}{" "}
                  {chinese ? "异常" : "issues"}
                </span>
              </div>
              <div className="menuSeparator" />
              <button
                type="button"
                role="menuitem"
                disabled={
                  actions.smartSwitching || actions.accounts.length === 0
                }
                onClick={() => {
                  close(true);
                  actions.onSmartSwitch();
                }}
              >
                <WorkspaceIcon name="switch" />
                {copy.addAccount.smartSwitch}
              </button>
              <button
                type="button"
                role="menuitem"
                disabled={
                  actions.exportingAccounts || actions.accounts.length === 0
                }
                onClick={() => {
                  close(true);
                  actions.onExportAll();
                }}
              >
                <WorkspaceIcon name="export" />
                {copy.metaStrip.exportAll}
              </button>
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  close(true);
                  actions.onShowAnalytics();
                }}
              >
                <WorkspaceIcon name="analytics" />
                {chinese ? "Token 用量与分析" : "Token usage & analytics"}
              </button>
            </div>,
            portalContainer,
          )
        : null}
    </>
  );
}
