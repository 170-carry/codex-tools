import { AccountsGrid } from "../AccountsGrid";
import type { CodexController } from "../../types/workspace";
import type { AccountsGridProps } from "../accounts/types";
import { useAppLayout } from "../../hooks/useAppLayout";
import { ClassicAccountsGrid } from "../classic/ClassicAccountsGrid";
import { ClassicMetaStrip } from "../classic/ClassicMetaStrip";
import { ClassicAccountActions } from "../classic/ClassicAccountActions";
import { moveAccountGroup } from "../../utils/accountDisplayOrder";

export function AccountsView({
  c,
  searchVisible,
  onCloseSearch,
  onShowAnalytics,
}: {
  c: CodexController;
  searchVisible: boolean;
  onCloseSearch: () => void;
  onShowAnalytics: () => void;
}) {
  const { layout } = useAppLayout();
  const actions: AccountsGridProps = {
    accountOrder: c.settings.accountOrder,
    orderingAccounts: c.savingSettings,
    onMoveAccount: (account, direction) => void c.updateSettings({
      accountOrder: moveAccountGroup(c.accounts, c.settings.accountOrder, account.accountKey, direction),
    }),
    searchVisible,
    onCloseSearch,
    onShowAnalytics,
    onSmartSwitch: () => void c.onSmartSwitch(),
    smartSwitching: c.smartSwitching,
    accounts: c.accounts,
    loading: c.loading,
    usageRefreshing: c.usageRefreshInFlight,
    showInitialUsageRefresh: c.initialUsageRefreshPending,
    usageRefreshError: c.usageRefreshError,
    exportingAccounts: c.exportingAccounts,
    authBusy: c.authBusy,
    switchingId: c.switchingId,
    warmingAccountId: c.warmingAccountId,
    resettingAccountId: c.resettingAccountId,
    onUseResetCredit: c.onUseResetCredit,
    renamingAccountId: c.renamingAccountId,
    pendingDeleteId: c.pendingDeleteId,
    onExportAll: () => void c.onExportAccounts(),
    onExport: (account) => void c.onExportAccounts(account),
    onReauthorize: (account) => void c.onReauthorizeAccount(account),
    onEditApiAccount: c.apiAccountEditor.open,
    onWarmup: c.onWarmupAccount,
    onRename: c.onRenameAccountLabel,
    onToggleApiProxy: c.onToggleAccountApiProxy,
    onSwitch: c.onSwitch,
    onDelete: (account) => void c.onDelete(account),
  };
  return (
    <div className="accountsPage">
      {layout === "classic" ? (
        <ClassicAccountsGrid
          {...actions}
          tokenUsage={c.tokenUsage}
          tokenUsageError={c.tokenUsageError}
          leadingContent={
            <ClassicMetaStrip
              accounts={c.accounts}
              exportingAccounts={c.exportingAccounts}
              onExportAccounts={actions.onExportAll}
            />
          }
          toolbarActions={
            <ClassicAccountActions
              onOpenAddDialog={c.onOpenAddDialog}
              onSmartSwitch={actions.onSmartSwitch}
              smartSwitching={c.smartSwitching || c.authBusy}
            />
          }
        />
      ) : (
        <AccountsGrid {...actions} />
      )}
    </div>
  );
}
