import type { AccountsGridProps } from "../../src/components/accounts/types";
import { accounts } from "./fixtures";

const noop = () => {};
export function accountProps(overrides: Partial<AccountsGridProps> = {}): AccountsGridProps {
  return {
    searchVisible: false,
    onCloseSearch: noop,
    onShowAnalytics: noop,
    onSmartSwitch: noop,
    smartSwitching: false,
    accounts,
    loading: false,
    usageRefreshing: false,
    showInitialUsageRefresh: false,
    usageRefreshError: null,
    exportingAccounts: false,
    authBusy: false,
    switchingId: null,
    warmingAccountId: null,
    renamingAccountId: null,
    pendingDeleteId: null,
    onExportAll: noop,
    onExport: noop,
    onReauthorize: noop,
    onEditApiAccount: noop,
    onWarmup: async () => true,
    onRename: async () => true,
    onToggleApiProxy: async () => true,
    onSwitch: async () => true,
    onDelete: noop,
    ...overrides,
  };
}
