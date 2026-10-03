import type { AccountSummary } from "../../types/app";

export type AccountGroup = {
  id: string;
  variants: AccountSummary[];
};

export type AccountRow = AccountGroup & {
  account: AccountSummary;
};

export type SwitchRecord = {
  id: string;
  target: string;
  source: string;
  timestamp: number;
};

export type AccountStatus =
  | "using"
  | "available"
  | "low"
  | "exhausted"
  | "issue";
export type StatusFilter = AccountStatus | "all";

export type RowActionMenuPosition = {
  left: number;
  top: number;
};

export type UiCopy = {
  searchPlaceholder: string;
  allStatuses: string;
  allPlans: string;
  proxyEnabled: string;
  statusUsing: string;
  statusAvailable: string;
  statusLow: string;
  statusExhausted: string;
  statusIssue: string;
  issueFallbackReason: string;
  tokenUsageTitle: string;
  tokenUsageError: string;
  detailsTitle: string;
  usageOverview: string;
  fiveHourUsage: string;
  weekUsage: string;
  remainingSuffix: (value: string) => string;
  resetTime: string;
  resetCreditsTitle: string;
  resetCreditsAvailable: (count: number | null) => string;
  resetCreditsExpiresAt: string;
  resetCreditsExpand: (hiddenCount: number) => string;
  resetCreditsCollapse: string;
  planType: string;
  recentSwitches: string;
  switchRecordAction: string;
  noSwitchRecords: string;
  fromPrefix: string;
  quickActions: string;
  reauthorize: string;
  warmup: string;
  warming: string;
  exportAccount: string;
  exportAll: string;
  deleteAccount: string;
  switchAccount: string;
  edit: string;
  save: string;
  cancel: string;
  noMatchesTitle: string;
  noMatchesDescription: string;
  emptyValue: string;
};

export type AccountsGridProps = {
  searchVisible: boolean;
  onCloseSearch: () => void;
  onShowAnalytics: () => void;
  onSmartSwitch: () => void;
  smartSwitching: boolean;
  accounts: AccountSummary[];
  loading: boolean;
  usageRefreshing: boolean;
  showInitialUsageRefresh: boolean;
  usageRefreshError: string | null;
  exportingAccounts: boolean;
  authBusy: boolean;
  switchingId: string | null;
  warmingAccountId: string | null;
  renamingAccountId: string | null;
  pendingDeleteId: string | null;
  onExportAll: () => void;
  onExport: (account: AccountSummary) => void;
  onReauthorize: (account: AccountSummary) => void;
  onWarmup: (account: AccountSummary) => Promise<boolean>;
  onRename: (account: AccountSummary, label: string) => Promise<boolean>;
  onToggleApiProxy: (
    account: AccountSummary,
    enabled: boolean,
  ) => Promise<boolean>;
  onSwitch: (account: AccountSummary) => Promise<boolean>;
  onDelete: (account: AccountSummary) => void;
};
