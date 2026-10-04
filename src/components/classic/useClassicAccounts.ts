import { useState, type ReactNode } from "react";
import { useI18n } from "../../i18n/I18nProvider";
import { useAccountsWorkspace } from "../../hooks/useAccountsWorkspace";
import { getUiCopy } from "../accounts/accountCopy";
import type { AccountsGridProps } from "../accounts/types";
import type { AccountSummary, CodexTokenUsageSnapshot } from "../../types/app";

export type ClassicAccountsProps = AccountsGridProps & {
  tokenUsage: CodexTokenUsageSnapshot | null;
  tokenUsageError: string | null;
  leadingContent?: ReactNode;
  toolbarActions?: ReactNode;
};
export function useClassicAccounts(props: ClassicAccountsProps) {
  const { copy, locale } = useI18n();
  const workspace = useAccountsWorkspace(props);
  const text = {
    ...getUiCopy(locale),
    fiveHourUsage: locale === "zh-CN" ? "5小时使用率" : "5h usage",
    weekUsage: locale === "zh-CN" ? "周使用率" : "Weekly usage",
  };
  const [editingAliasId, setEditingAliasId] = useState<string | null>(null);
  const [aliasDraft, setAliasDraft] = useState("");
  const [expandedResetCreditsByAccount, setExpandedResetCreditsByAccount] =
    useState<Record<string, boolean>>({});
  const selectedRow =
    workspace.rows.find(
      (row) => row.account.id === workspace.inspectedAccount?.id,
    ) ??
    workspace.filteredRows.find((row) => row.account.isCurrent) ??
    workspace.filteredRows[0] ??
    workspace.rows[0] ??
    null;
  const cancelAliasEdit = () => {
    setEditingAliasId(null);
    setAliasDraft("");
  };
  const selectAccount = (id: string) => {
    cancelAliasEdit();
    workspace.inspect(id);
  };
  const selectVariant = (groupId: string, account: AccountSummary) => {
    workspace.selectVariant(groupId, account.id);
    selectAccount(account.id);
  };
  const startAliasEdit = (account: AccountSummary) => {
    setEditingAliasId(account.id);
    setAliasDraft(account.label);
  };
  const commitAliasEdit = async (account: AccountSummary) => {
    const label = aliasDraft.trim();
    if (!label || label === account.label.trim()) {
      cancelAliasEdit();
      return;
    }
    if (await props.onRename(account, label)) cancelAliasEdit();
  };
  const handleSwitch = workspace.switchAccount;
  const toggleResetCredits = (id: string) =>
    setExpandedResetCreditsByAccount((value) => ({
      ...value,
      [id]: !value[id],
    }));
  return {
    ...props,
    ...workspace,
    actions: props,
    copy,
    locale,
    text,
    selectedRow,
    editingAliasId,
    aliasDraft,
    setAliasDraft,
    selectAccount,
    selectVariant,
    startAliasEdit,
    cancelAliasEdit,
    commitAliasEdit,
    handleSwitch,
    expandedResetCreditsByAccount,
    toggleResetCredits,
  };
}
export type ClassicAccountsWorkspace = ReturnType<typeof useClassicAccounts>;
