import type { AccountSummary } from "../../types/app";
import { useI18n } from "../../i18n/I18nProvider";
import type { AccountsGridProps, UiCopy } from "./types";
import { ActionMenu } from "../ui/ActionMenu";
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
  return <ActionMenu label={`${account.label} · ${text.quickActions}`} items={items.map(item => ({...item, icon: <WorkspaceIcon name={item.icon} />}))} />;
}
