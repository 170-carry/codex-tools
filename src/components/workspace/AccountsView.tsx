import { AccountsGrid } from "../AccountsGrid";
import type { CodexController } from "../../types/workspace";

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
  return (
    <div className="accountsPage">
      <AccountsGrid
        searchVisible={searchVisible}
        onCloseSearch={onCloseSearch}
        onShowAnalytics={onShowAnalytics}
        onSmartSwitch={() => void c.onSmartSwitch()}
        smartSwitching={c.smartSwitching}
        accounts={c.accounts}
        loading={c.loading}
        usageRefreshing={c.usageRefreshInFlight}
        showInitialUsageRefresh={c.initialUsageRefreshPending}
        usageRefreshError={c.usageRefreshError}
        exportingAccounts={c.exportingAccounts}
        authBusy={c.authBusy}
        switchingId={c.switchingId}
        warmingAccountId={c.warmingAccountId}
        renamingAccountId={c.renamingAccountId}
        pendingDeleteId={c.pendingDeleteId}
        onExportAll={() => void c.onExportAccounts()}
        onExport={(account) => void c.onExportAccounts(account)}
        onReauthorize={(account) => void c.onReauthorizeAccount(account)}
        onWarmup={c.onWarmupAccount}
        onRename={c.onRenameAccountLabel}
        onToggleApiProxy={c.onToggleAccountApiProxy}
        onSwitch={c.onSwitch}
        onDelete={(account) => void c.onDelete(account)}
      />
    </div>
  );
}
