import { createPortal } from "react-dom";
import { useId } from "react";
import type { AccountSummary } from "../../types/app";
import { useI18n } from "../../i18n/I18nProvider";
import type { AccountsGridProps, UiCopy } from "./types";
import { useAnchoredMenu } from "../../hooks/useAnchoredMenu";
import {
  WorkspaceIcon,
  type WorkspaceIconName,
} from "../workspace/WorkspaceIcon";

export function AccountActionMenu({
  account,
  actions,
  text,
  onInspect,
}: {
  account: AccountSummary;
  actions: AccountsGridProps;
  text: UiCopy;
  onInspect: () => void;
}) {
  const { copy } = useI18n();
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
  const items: {
    label: string;
    icon: WorkspaceIconName;
    disabled?: boolean;
    action: () => void;
    danger?: boolean;
  }[] = [
    { label: text.detailsTitle, icon: "info", action: onInspect },
    ...(account.sourceKind === "relay"
      ? [{
          label: copy.addAccount.apiEditTitle,
          icon: "edit" as const,
          disabled: actions.authBusy,
          action: () => actions.onEditApiAccount(account),
        }]
      : []),
    {
      label: text.reauthorize,
      icon: "login",
      disabled: actions.authBusy,
      action: () => actions.onReauthorize(account),
    },
    ...(account.sourceKind !== "relay"
      ? [
          {
            label:
              actions.warmingAccountId === account.id
                ? text.warming
                : text.warmup,
            icon: "warmup" as const,
            disabled: actions.warmingAccountId !== null || actions.authBusy,
            action: () => void actions.onWarmup(account),
          },
        ]
      : []),
    {
      label: text.exportAccount,
      icon: "export",
      disabled: actions.exportingAccounts,
      action: () => actions.onExport(account),
    },
    {
      label: text.deleteAccount,
      icon: "delete",
      danger: true,
      disabled: actions.authBusy,
      action: () => actions.onDelete(account),
    },
  ];
  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        className="rowMoreButton"
        aria-label={`${account.label} · ${text.quickActions}`}
        title={text.quickActions}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={toggle}
      >
        <WorkspaceIcon name="more" />
      </button>
      {open && portalContainer
        ? createPortal(
            <div
              ref={menuRef}
              className="accountActionMenu"
              id={id}
              style={position}
              role="menu"
              aria-label={text.quickActions}
            >
              {items.slice(0, -1).map((item) => (
                <button
                  key={item.icon}
                  type="button"
                  role="menuitem"
                  disabled={item.disabled}
                  onClick={() => {
                    close();
                    item.action();
                  }}
                >
                  <WorkspaceIcon name={item.icon} />
                  <span>{item.label}</span>
                </button>
              ))}
              <div className="menuSeparator" />
              <button
                type="button"
                role="menuitem"
                className="dangerMenuItem"
                disabled={actions.authBusy}
                onClick={() => {
                  close();
                  actions.onDelete(account);
                }}
              >
                <WorkspaceIcon name="delete" />
                <span>{text.deleteAccount}</span>
              </button>
            </div>,
            portalContainer,
          )
        : null}
    </>
  );
}
