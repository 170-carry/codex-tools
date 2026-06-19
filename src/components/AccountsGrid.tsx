import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import type { AccountSummary, CodexTokenUsageSnapshot, UsageWindow } from "../types/app";
import { useI18n } from "../i18n/I18nProvider";
import { compareAccountsByRemaining } from "../utils/accountRanking";
import {
  formatPlan,
  formatTokenCount,
  formatWindowLabel,
  percent,
  planTone,
  remainingPercent,
  toProgressWidth,
} from "../utils/usage";

type AccountGroup = {
  id: string;
  variants: AccountSummary[];
};

type AccountRow = AccountGroup & {
  account: AccountSummary;
};

type SwitchRecord = {
  id: string;
  target: string;
  source: string;
  timestamp: number;
};

type AccountStatus = "using" | "available" | "low" | "exhausted" | "issue";
type StatusFilter = AccountStatus | "all";

type UiCopy = {
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
  planType: string;
  recentSwitches: string;
  switchRecordAction: string;
  noSwitchRecords: string;
  fromPrefix: string;
  quickActions: string;
  reauthorize: string;
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

type AccountsGridProps = {
  leadingContent?: ReactNode;
  toolbarActions?: ReactNode;
  accounts: AccountSummary[];
  tokenUsage: CodexTokenUsageSnapshot | null;
  tokenUsageError: string | null;
  loading: boolean;
  exportingAccounts: boolean;
  switchingId: string | null;
  renamingAccountId: string | null;
  pendingDeleteId: string | null;
  onExportAll: () => void;
  onExport: (account: AccountSummary) => void;
  onReauthorize: (account: AccountSummary) => void;
  onRename: (account: AccountSummary, label: string) => Promise<boolean>;
  onToggleApiProxy: (account: AccountSummary, enabled: boolean) => Promise<boolean>;
  onSwitch: (account: AccountSummary) => void;
  onDelete: (account: AccountSummary) => void;
};

const PLAN_PRIORITY: Record<string, number> = {
  api: 0,
  team: 0,
  enterprise: 1,
  business: 2,
  pro: 3,
  plus: 4,
  free: 5,
  unknown: 6,
};

function planPriority(planType: string | null | undefined): number {
  const normalized = planType?.trim().toLowerCase() ?? "";
  return PLAN_PRIORITY[normalized] ?? PLAN_PRIORITY.unknown;
}

function sortVariantsForGroup(left: AccountSummary, right: AccountSummary): number {
  const priorityDiff =
    planPriority(left.planType ?? left.usage?.planType) -
    planPriority(right.planType ?? right.usage?.planType);
  if (priorityDiff !== 0) {
    return priorityDiff;
  }

  if (left.isCurrent !== right.isCurrent) {
    return left.isCurrent ? -1 : 1;
  }

  return compareAccountsByRemaining(left, right);
}

function accountHasBlockingIssue(account: AccountSummary): boolean {
  return Boolean(
    account.authRefreshBlocked ||
      account.profileIntegrityError ||
      account.profileLastValidationError ||
      account.authRefreshError,
  );
}

function accountIssueReason(account: AccountSummary, fallbackReason: string): string | null {
  return (
    account.profileIntegrityError ||
    account.profileLastValidationError ||
    account.authRefreshError ||
    (account.authRefreshBlocked ? fallbackReason : null)
  );
}

function usedPercent(window: UsageWindow | null): number | null {
  if (!window) {
    return null;
  }
  return Math.max(0, Math.min(100, window.usedPercent));
}

function accountHasExhaustedWindow(account: AccountSummary): boolean {
  return [account.usage?.fiveHour ?? null, account.usage?.oneWeek ?? null].some((window) => {
    const remaining = remainingPercent(window);
    return remaining !== null && remaining <= 0;
  });
}

function lowestRemaining(account: AccountSummary): number | null {
  const values = [remainingPercent(account.usage?.fiveHour ?? null), remainingPercent(account.usage?.oneWeek ?? null)]
    .filter((value): value is number => value !== null && !Number.isNaN(value));
  return values.length > 0 ? Math.min(...values) : null;
}

function accountStatus(account: AccountSummary): AccountStatus {
  if (accountHasBlockingIssue(account)) {
    return "issue";
  }

  if (accountHasExhaustedWindow(account)) {
    return "exhausted";
  }

  const remaining = lowestRemaining(account);
  if (remaining !== null && remaining < 15) {
    return "low";
  }

  if (account.isCurrent) {
    return "using";
  }

  return "available";
}

function getUiCopy(locale: string): UiCopy {
  if (locale === "zh-CN") {
    return {
      searchPlaceholder: "搜索账号 / 邮箱",
      allStatuses: "全部状态",
      allPlans: "所有套餐",
      proxyEnabled: "反代开启",
      statusUsing: "使用中",
      statusAvailable: "可用",
      statusLow: "即将耗尽",
      statusExhausted: "已耗尽",
      statusIssue: "异常账号",
      issueFallbackReason: "查看账号授权和用量状态",
      tokenUsageTitle: "Token 使用量",
      tokenUsageError: "Token 用量读取失败",
      detailsTitle: "详情",
      usageOverview: "使用概览",
      fiveHourUsage: "5小时使用率",
      weekUsage: "周使用率",
      remainingSuffix: (value) => `剩余 ${value}`,
      resetTime: "重置时间",
      planType: "套餐类型",
      recentSwitches: "最近切换记录",
      switchRecordAction: "切换到此账号",
      noSwitchRecords: "暂无切换记录",
      fromPrefix: "从",
      quickActions: "快捷操作",
      reauthorize: "测试登录",
      exportAccount: "导出账号",
      exportAll: "全部导出",
      deleteAccount: "删除账号",
      switchAccount: "切换",
      edit: "编辑",
      save: "保存",
      cancel: "取消",
      noMatchesTitle: "没有匹配账号",
      noMatchesDescription: "调整搜索或筛选条件后再查看列表。",
      emptyValue: "--",
    };
  }

  return {
    searchPlaceholder: "Search account / email",
    allStatuses: "All statuses",
    allPlans: "All plans",
    proxyEnabled: "Proxy enabled",
    statusUsing: "In use",
    statusAvailable: "Available",
    statusLow: "Low quota",
    statusExhausted: "Exhausted",
    statusIssue: "Needs attention",
    issueFallbackReason: "Check account auth and usage state",
    tokenUsageTitle: "Token usage",
    tokenUsageError: "Token usage unavailable",
    detailsTitle: "Details",
    usageOverview: "Usage overview",
    fiveHourUsage: "5h usage",
    weekUsage: "Weekly usage",
    remainingSuffix: (value) => `${value} remaining`,
    resetTime: "Reset time",
    planType: "Plan",
    recentSwitches: "Recent switches",
    switchRecordAction: "Switched to this account",
    noSwitchRecords: "No switch records yet",
    fromPrefix: "from",
    quickActions: "Quick actions",
    reauthorize: "Test login",
    exportAccount: "Export account",
    exportAll: "Export all",
    deleteAccount: "Delete",
    switchAccount: "Switch",
    edit: "Edit",
    save: "Save",
    cancel: "Cancel",
    noMatchesTitle: "No matching accounts",
    noMatchesDescription: "Change the search or filters to view accounts.",
    emptyValue: "--",
  };
}

function statusLabel(status: AccountStatus, text: UiCopy): string {
  if (status === "using") return text.statusUsing;
  if (status === "exhausted") return text.statusExhausted;
  if (status === "low") return text.statusLow;
  if (status === "issue") return text.statusIssue;
  return text.statusAvailable;
}

function accountInitial(account: AccountSummary): string {
  const seed = account.email || account.label || account.accountId || account.accountKey;
  const normalized = seed.trim();
  return (normalized[0] || "?").toUpperCase();
}

function displayAccountAddress(account: AccountSummary, emptyValue: string): string {
  return account.email || account.accountId || account.apiBaseUrl || emptyValue;
}

function formatResetValue(epochSec: number | null | undefined, locale: string, emptyValue: string): string {
  if (!epochSec) {
    return emptyValue;
  }

  return new Date(epochSec * 1000).toLocaleString(locale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatFullDate(epochSec: number | null | undefined, locale: string, emptyValue: string): string {
  if (!epochSec) {
    return emptyValue;
  }

  return new Date(epochSec * 1000).toLocaleString(locale, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function eventTimestampToUnixSeconds(timestamp: number): number {
  const timeOrigin = typeof performance === "undefined" ? 0 : performance.timeOrigin;
  return Math.floor((timeOrigin + timestamp) / 1000);
}

function UsageMeter({
  label,
  window,
  text,
  className,
}: {
  label: string;
  window: UsageWindow | null;
  text: UiCopy;
  className?: string;
}) {
  const value = usedPercent(window);
  const remaining = remainingPercent(window);
  const tone = remaining !== null && remaining <= 0 ? "danger" : remaining !== null && remaining < 15 ? "warning" : "normal";

  return (
    <div className={`usageMeter tone-${tone}${className ? ` ${className}` : ""}`}>
      <div className="usageMeterHead">
        <span>{label}</span>
        <strong>{percent(value)}</strong>
      </div>
      <div className="usageBar" aria-hidden="true">
        <span style={{ width: toProgressWidth(value) }} />
      </div>
      <div className="usageMeterFoot">
        <span>{text.remainingSuffix(percent(remaining))}</span>
        <span>{formatWindowLabel(window, {
          fallback: label,
          oneWeek: "1w",
          hourSuffix: "h",
          minuteSuffix: "m",
        })}</span>
      </div>
      <span className="visuallyHidden">
        {label} {percent(value)} {text.remainingSuffix(percent(remaining))}
      </span>
    </div>
  );
}

function TokenUsageStrip({
  tokenUsage,
  tokenUsageError,
  locale,
  text,
  accountCount,
  exportingAccounts,
  onExportAll,
}: {
  tokenUsage: CodexTokenUsageSnapshot | null;
  tokenUsageError: string | null;
  locale: string;
  text: UiCopy;
  accountCount: number;
  exportingAccounts: boolean;
  onExportAll: () => void;
}) {
  const items = [
    { label: "24H", value: tokenUsage?.last24h.totalTokens },
    { label: "3D", value: tokenUsage?.last3d.totalTokens },
    { label: "7D", value: tokenUsage?.last7d.totalTokens },
    { label: "30D", value: tokenUsage?.last30d.totalTokens },
  ];

  return (
    <section className="accountTokenUsageStrip" aria-label={text.tokenUsageTitle}>
      <strong>{text.tokenUsageTitle}</strong>
      <div className="accountTokenUsageItems">
        {items.map((item) => (
          <span key={item.label} className="accountTokenUsageItem">
            <em>{item.label}</em>
            <b>{formatTokenCount(item.value, locale)}</b>
          </span>
        ))}
      </div>
      <div className="accountTokenUsageActions">
        {tokenUsageError ? <span className="accountTokenUsageError">{text.tokenUsageError}</span> : null}
        <button
          className="ghost accountTokenExportButton"
          type="button"
          onClick={onExportAll}
          disabled={exportingAccounts || accountCount === 0}
          aria-label={text.exportAll}
        >
          <ActionIcon type="export" />
          <span>{text.exportAll}</span>
        </button>
      </div>
    </section>
  );
}

function copyAccountText(value: string) {
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    void navigator.clipboard.writeText(value);
    return;
  }

  if (typeof document === "undefined") {
    return;
  }

  const input = document.createElement("textarea");
  input.value = value;
  input.setAttribute("readonly", "true");
  input.style.position = "fixed";
  input.style.opacity = "0";
  document.body.append(input);
  input.select();
  document.execCommand("copy");
  input.remove();
}

function SearchIcon() {
  return (
    <svg className="searchIcon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <circle cx="11" cy="11" r="7" />
      <path d="m16 16 4 4" />
    </svg>
  );
}

function MoreIcon() {
  return (
    <svg className="rowIcon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 6h.01" />
      <path d="M12 12h.01" />
      <path d="M12 18h.01" />
    </svg>
  );
}

function ActionIcon({ type }: { type: "login" | "export" | "delete" | "switch" | "edit" }) {
  if (type === "delete") {
    return (
      <svg className="actionIconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M3 6h18" />
        <path d="M8 6V4h8v2" />
        <path d="m19 6-1 14H6L5 6" />
        <path d="M10 11v6" />
        <path d="M14 11v6" />
      </svg>
    );
  }

  if (type === "export") {
    return (
      <svg className="actionIconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <rect x="8" y="8" width="10" height="12" rx="2" />
        <path d="M6 16H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v1" />
      </svg>
    );
  }

  if (type === "login") {
    return (
      <svg className="actionIconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M20 6 9 17l-5-5" />
      </svg>
    );
  }

  if (type === "edit") {
    return (
      <svg className="actionIconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M12 20h9" />
        <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" />
      </svg>
    );
  }

  return (
    <svg className="actionIconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 5v14" />
      <path d="m19 12-7 7-7-7" />
    </svg>
  );
}

export function AccountsGrid({
  leadingContent,
  toolbarActions,
  accounts,
  tokenUsage,
  tokenUsageError,
  loading,
  exportingAccounts,
  switchingId,
  renamingAccountId,
  pendingDeleteId,
  onExportAll,
  onExport,
  onReauthorize,
  onRename,
  onToggleApiProxy,
  onSwitch,
  onDelete,
}: AccountsGridProps) {
  const { copy, locale } = useI18n();
  const text = getUiCopy(locale);
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [planFilter, setPlanFilter] = useState("all");
  const [selectedAccountId, setSelectedAccountId] = useState<string | null>(null);
  const [preferredVariantByGroup, setPreferredVariantByGroup] = useState<Record<string, string>>({});
  const [editingAliasId, setEditingAliasId] = useState<string | null>(null);
  const [aliasDraft, setAliasDraft] = useState("");
  const [openMenuAccountId, setOpenMenuAccountId] = useState<string | null>(null);
  const openMenuRootRef = useRef<HTMLDivElement | null>(null);
  const [switchRecords, setSwitchRecords] = useState<SwitchRecord[]>([]);

  useEffect(() => {
    if (!openMenuAccountId || typeof document === "undefined") {
      return undefined;
    }

    const closeOpenMenu = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && openMenuRootRef.current?.contains(target)) {
        return;
      }

      setOpenMenuAccountId(null);
    };

    document.addEventListener("pointerdown", closeOpenMenu, true);
    return () => {
      document.removeEventListener("pointerdown", closeOpenMenu, true);
    };
  }, [openMenuAccountId]);

  const groupedAccounts = useMemo<AccountGroup[]>(() => {
    const groups = new Map<string, AccountSummary[]>();

    for (const account of accounts) {
      const existing = groups.get(account.accountKey);
      if (existing) {
        existing.push(account);
      } else {
        groups.set(account.accountKey, [account]);
      }
    }

    return Array.from(groups.entries()).map(([id, variants]) => ({
      id,
      variants: [...variants].sort(sortVariantsForGroup),
    }));
  }, [accounts]);

  const rows = useMemo<AccountRow[]>(() => {
    const mapped = groupedAccounts.map((group) => {
      const fallbackVariant = group.variants[0];
      if (!fallbackVariant) {
        throw new Error(`Account group ${group.id} has no variants`);
      }

      const preferredId = preferredVariantByGroup[group.id];
      const preferred = preferredId
        ? group.variants.find((account) => account.id === preferredId)
        : null;
      const activeVariant =
        group.variants.find((account) => account.id === switchingId) ||
        group.variants.find((account) => account.id === pendingDeleteId) ||
        group.variants.find((account) => account.isCurrent) ||
        preferred ||
        fallbackVariant;

      return {
        ...group,
        account: activeVariant,
      };
    });

    return mapped.sort((left, right) => {
      if (left.account.isCurrent !== right.account.isCurrent) {
        return left.account.isCurrent ? -1 : 1;
      }
      return compareAccountsByRemaining(left.account, right.account);
    });
  }, [groupedAccounts, pendingDeleteId, preferredVariantByGroup, switchingId]);

  const filteredRows = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();

    return rows.filter((row) => {
      const account = row.account;
      const status = accountStatus(account);
      const normalizedPlan = (account.planType || account.usage?.planType || "unknown").toLowerCase();
      const haystack = [
        account.label,
        account.email,
        account.accountId,
        account.accountKey,
        account.apiBaseUrl,
        account.modelName,
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase();

      return (
        (!normalizedQuery || haystack.includes(normalizedQuery)) &&
        (statusFilter === "all" || status === statusFilter) &&
        (planFilter === "all" || normalizedPlan === planFilter)
      );
    });
  }, [planFilter, query, rows, statusFilter]);

  const selectedRow =
    filteredRows.find((row) => row.account.id === selectedAccountId) ||
    rows.find((row) => row.account.id === selectedAccountId) ||
    filteredRows[0] ||
    rows[0] ||
    null;

  const selectAccount = (accountId: string) => {
    setEditingAliasId(null);
    setAliasDraft("");
    setOpenMenuAccountId(null);
    setSelectedAccountId(accountId);
  };

  const selectVariant = (groupId: string, account: AccountSummary) => {
    setPreferredVariantByGroup((current) => ({
      ...current,
      [groupId]: account.id,
    }));
    selectAccount(account.id);
  };

  const startAliasEdit = (account: AccountSummary) => {
    setEditingAliasId(account.id);
    setAliasDraft(account.label);
  };

  const cancelAliasEdit = () => {
    setEditingAliasId(null);
    setAliasDraft("");
  };

  const commitAliasEdit = async (account: AccountSummary) => {
    const normalized = aliasDraft.trim();
    if (!normalized || normalized === account.label.trim()) {
      cancelAliasEdit();
      return;
    }

    const updated = await onRename(account, normalized);
    if (updated) {
      cancelAliasEdit();
    }
  };

  const handleSwitch = (account: AccountSummary, eventTimestamp: number) => {
    const sourceAccount = selectedRow?.account;
    const now = eventTimestampToUnixSeconds(eventTimestamp);

    setOpenMenuAccountId(null);
    setSwitchRecords((current) => [
      {
        id: `${account.id}-${now}-${current.length}`,
        target: displayAccountAddress(account, text.emptyValue),
        source:
          sourceAccount && sourceAccount.id !== account.id
            ? displayAccountAddress(sourceAccount, text.emptyValue)
            : text.emptyValue,
        timestamp: now,
      },
      ...current,
    ].slice(0, 5));
    onSwitch(account);
  };

  return (
    <section className="accountsWorkspace" aria-busy={loading}>
      <div className="accountListStack">
        {leadingContent ? <div className="accountListLeading">{leadingContent}</div> : null}
        <div className="accountListPanel">
          <div className="accountToolbar">
            <label className="accountSearch">
              <SearchIcon />
              <input
                value={query}
                onChange={(event) => setQuery(event.currentTarget.value)}
                placeholder={text.searchPlaceholder}
                aria-label={text.searchPlaceholder}
              />
            </label>
            <div className="accountFilters">
              <select value={statusFilter} onChange={(event) => setStatusFilter(event.currentTarget.value as StatusFilter)}>
                <option value="all">{text.allStatuses}</option>
                <option value="using">{text.statusUsing}</option>
                <option value="available">{text.statusAvailable}</option>
                <option value="low">{text.statusLow}</option>
                <option value="exhausted">{text.statusExhausted}</option>
                <option value="issue">{text.statusIssue}</option>
              </select>
              <select value={planFilter} onChange={(event) => setPlanFilter(event.currentTarget.value)}>
                <option value="all">{text.allPlans}</option>
                <option value="pro">PRO</option>
                <option value="plus">PLUS</option>
                <option value="team">TEAM</option>
                <option value="enterprise">ENTERPRISE</option>
                <option value="business">BUSINESS</option>
                <option value="api">API</option>
                <option value="free">FREE</option>
              </select>
            </div>
            {toolbarActions ? <div className="accountToolbarActions">{toolbarActions}</div> : null}
          </div>

          {filteredRows.length === 0 && !loading ? (
            <div className="emptyState accountEmptyState">
              <h3>{accounts.length === 0 ? copy.accountsGrid.emptyTitle : text.noMatchesTitle}</h3>
              <p>{accounts.length === 0 ? copy.accountsGrid.emptyDescription : text.noMatchesDescription}</p>
            </div>
          ) : (
            <div className="accountListFrame">
              <TokenUsageStrip
                tokenUsage={tokenUsage}
                tokenUsageError={tokenUsageError}
                locale={locale}
                text={text}
                accountCount={accounts.length}
                exportingAccounts={exportingAccounts}
                onExportAll={onExportAll}
              />
              <div className="accountListHeader" aria-hidden="true">
                <span>{copy.bottomDock.accounts}</span>
                <span>{text.fiveHourUsage}</span>
                <span>{text.weekUsage}</span>
                <span>{text.resetTime}</span>
                <span>{text.proxyEnabled}</span>
                <span>{text.switchAccount}</span>
              </div>
              <div className="accountRows">
                {filteredRows.map((row) => {
                const account = row.account;
                const status = accountStatus(account);
                const normalizedPlan = account.planType || account.usage?.planType;
                const isSelected = selectedRow?.account.id === account.id;
                const isSwitching = switchingId === account.id;
                const isDeletePending = pendingDeleteId === account.id;
                const isMenuOpen = openMenuAccountId === account.id;
                const accountAddress = displayAccountAddress(account, text.emptyValue);
                const issueReason = accountIssueReason(account, text.issueFallbackReason);

                return (
                  <article
                    key={row.id}
                    className={`accountRow status-${status}${isSelected ? " isSelected" : ""}${isMenuOpen ? " isMenuOpen" : ""}`}
                    role="button"
                    tabIndex={0}
                    onClick={() => selectAccount(account.id)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        selectAccount(account.id);
                      }
                    }}
                  >
                    <div className="accountIdentityCell">
                      <span className={`accountAvatar tone-${planTone(normalizedPlan)}`}>
                        {accountInitial(account)}
                      </span>
                      <span className="accountIdentityText">
                        <span className="accountTitleLine">
                          {row.variants.map((variant) => {
                            const plan = formatPlan(
                              variant.planType || variant.usage?.planType,
                              copy.accountCard.planLabels,
                            );
                            const isVariantSelected = variant.id === account.id;

                            return (
                              <button
                                key={variant.id}
                                type="button"
                                className={`planChip tone-${planTone(variant.planType || variant.usage?.planType)}${
                                  isVariantSelected ? " isSelected" : ""
                                }`}
                                onClick={(event) => {
                                  event.stopPropagation();
                                  selectVariant(row.id, variant);
                                }}
                                aria-pressed={isVariantSelected}
                              >
                              {plan}
                            </button>
                          );
                          })}
                          <button
                            type="button"
                            className="accountNameButton"
                            title={accountAddress}
                            onClick={(event) => {
                              event.stopPropagation();
                              copyAccountText(accountAddress);
                            }}
                          >
                            {accountAddress}
                          </button>
                        </span>
                        <span
                          className={`statusText status-${status}`}
                          title={status === "issue" ? (issueReason ?? text.issueFallbackReason) : statusLabel(status, text)}
                        >
                          <span className="statusDot" />
                          <span className="statusLabel">{statusLabel(status, text)}</span>
                          {status === "issue" ? (
                            <span className="statusReason">{issueReason ?? text.issueFallbackReason}</span>
                          ) : null}
                        </span>
                      </span>
                    </div>
                    <UsageMeter
                      className="accountUsageFive"
                      label={text.fiveHourUsage}
                      window={account.usage?.fiveHour ?? null}
                      text={text}
                    />
                    <UsageMeter
                      className="accountUsageWeek"
                      label={text.weekUsage}
                      window={account.usage?.oneWeek ?? null}
                      text={text}
                    />
                    <div className="resetCell">
                      <span>{formatResetValue(account.usage?.fiveHour?.resetAt, locale, text.emptyValue)}</span>
                      <strong>{formatResetValue(account.usage?.oneWeek?.resetAt, locale, text.emptyValue)}</strong>
                    </div>
                    <label className="rowToggle" onClick={(event) => event.stopPropagation()}>
                      <input
                        type="checkbox"
                        checked={account.apiProxyEnabled}
                        onChange={(event) => {
                          void onToggleApiProxy(account, event.currentTarget.checked);
                        }}
                        aria-label={copy.accountCard.apiProxyToggle}
                      />
                      <span />
                    </label>
                    <div className="rowActions" ref={isMenuOpen ? openMenuRootRef : undefined}>
                      <button
                        type="button"
                        className="rowSwitchButton"
                        disabled={isSwitching}
                        onClick={(event) => {
                          event.stopPropagation();
                          handleSwitch(account, event.timeStamp);
                        }}
                      >
                        {isSwitching ? copy.accountCard.launching : text.switchAccount}
                      </button>
                      <button
                        type="button"
                        className={`rowMoreButton${isDeletePending ? " isPending" : ""}`}
                        onClick={(event) => {
                          event.stopPropagation();
                          setOpenMenuAccountId(isMenuOpen ? null : account.id);
                        }}
                        title={text.quickActions}
                        aria-label={text.quickActions}
                        aria-expanded={isMenuOpen}
                      >
                        <MoreIcon />
                      </button>
                      {isMenuOpen ? (
                        <div className="rowActionMenu" onClick={(event) => event.stopPropagation()}>
                          <button
                            type="button"
                            onClick={() => {
                              setOpenMenuAccountId(null);
                              onReauthorize(account);
                            }}
                          >
                            <ActionIcon type="login" />
                            {text.reauthorize}
                          </button>
                          <button
                            type="button"
                            disabled={exportingAccounts}
                            onClick={() => {
                              setOpenMenuAccountId(null);
                              onExport(account);
                            }}
                          >
                            <ActionIcon type="export" />
                            {text.exportAccount}
                          </button>
                          <button
                            type="button"
                            className="dangerMenuItem"
                            onClick={() => {
                              setOpenMenuAccountId(null);
                              onDelete(account);
                            }}
                          >
                            <ActionIcon type="delete" />
                            {isDeletePending ? copy.accountCard.deleteConfirm : text.deleteAccount}
                          </button>
                        </div>
                      ) : null}
                    </div>
                  </article>
                );
                })}
              </div>
            </div>
          )}
        </div>
      </div>

      <aside className="accountDetailPanel" aria-label={text.detailsTitle}>
        {selectedRow ? (
          <>
            <header className="detailHeader">
              <div className="detailIdentity">
                <span className={`accountAvatar detailAvatar tone-${planTone(selectedRow.account.planType || selectedRow.account.usage?.planType)}`}>
                  {accountInitial(selectedRow.account)}
                </span>
                <div className="detailTitleBlock">
                  <span className={`planChip tone-${planTone(selectedRow.account.planType || selectedRow.account.usage?.planType)} isStatic`}>
                    {formatPlan(
                      selectedRow.account.planType || selectedRow.account.usage?.planType,
                      copy.accountCard.planLabels,
                    )}
                  </span>
                  {editingAliasId === selectedRow.account.id ? (
                    <div className="detailAliasEditor">
                      <input
                        value={aliasDraft}
                        onChange={(event) => setAliasDraft(event.currentTarget.value)}
                        disabled={renamingAccountId === selectedRow.account.accountKey}
                        onKeyDown={(event) => {
                          if (event.key === "Escape") {
                            event.preventDefault();
                            cancelAliasEdit();
                          }
                          if (event.key === "Enter") {
                            event.preventDefault();
                            void commitAliasEdit(selectedRow.account);
                          }
                        }}
                        autoFocus
                      />
                      <button type="button" onClick={() => void commitAliasEdit(selectedRow.account)}>
                        {text.save}
                      </button>
                      <button type="button" onClick={cancelAliasEdit}>
                        {text.cancel}
                      </button>
                    </div>
                  ) : (
                    <h2>{selectedRow.account.label}</h2>
                  )}
                  <span
                    className={`statusText status-${accountStatus(selectedRow.account)}`}
                    title={
                      accountStatus(selectedRow.account) === "issue"
                        ? (accountIssueReason(selectedRow.account, text.issueFallbackReason) ?? text.issueFallbackReason)
                        : statusLabel(accountStatus(selectedRow.account), text)
                    }
                  >
                    <span className="statusDot" />
                    <span className="statusLabel">{statusLabel(accountStatus(selectedRow.account), text)}</span>
                    {accountStatus(selectedRow.account) === "issue" ? (
                      <span className="statusReason">
                        {accountIssueReason(selectedRow.account, text.issueFallbackReason) ?? text.issueFallbackReason}
                      </span>
                    ) : null}
                  </span>
                </div>
              </div>
              <button
                type="button"
                className="detailEditButton"
                onClick={() => startAliasEdit(selectedRow.account)}
                disabled={editingAliasId === selectedRow.account.id || renamingAccountId === selectedRow.account.accountKey}
              >
                <ActionIcon type="edit" />
                {text.edit}
              </button>
            </header>

            <section className="detailCard">
              <h3>{text.usageOverview}</h3>
              <UsageMeter
                label={text.fiveHourUsage}
                window={selectedRow.account.usage?.fiveHour ?? null}
                text={text}
              />
              <UsageMeter
                label={text.weekUsage}
                window={selectedRow.account.usage?.oneWeek ?? null}
                text={text}
              />
            </section>

            <section className="detailMetaGrid">
              <div>
                <span>{text.resetTime}</span>
                <strong>{formatFullDate(selectedRow.account.usage?.fiveHour?.resetAt, locale, text.emptyValue)}</strong>
              </div>
              <div>
                <span>{text.planType}</span>
                <strong>
                  {formatPlan(
                    selectedRow.account.planType || selectedRow.account.usage?.planType,
                    copy.accountCard.planLabels,
                  )}
                </strong>
              </div>
            </section>

            <section className="detailCard recentSwitchCard">
              <div className="detailSectionTitle">
                <h3>{text.recentSwitches}</h3>
              </div>
              {switchRecords.length > 0 ? (
                <ul className="recentSwitchList">
                  {switchRecords.map((record) => (
                    <li key={record.id}>
                      <span className="statusDot status-using" />
                      <span>{text.switchRecordAction}</span>
                      <strong>{formatFullDate(record.timestamp, locale, text.emptyValue)}</strong>
                      <em>{record.source === text.emptyValue ? record.target : `${text.fromPrefix} ${record.source}`}</em>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="recentSwitchEmpty">{text.noSwitchRecords}</p>
              )}
            </section>

            <section className="detailCard quickActionCard">
              <h3>{text.quickActions}</h3>
              <div className="quickActionGrid">
                <button type="button" onClick={() => onReauthorize(selectedRow.account)}>
                  <ActionIcon type="login" />
                  <span>{text.reauthorize}</span>
                </button>
                <button
                  type="button"
                  onClick={(event) => handleSwitch(selectedRow.account, event.timeStamp)}
                  disabled={switchingId === selectedRow.account.id}
                >
                  <ActionIcon type="switch" />
                  <span>{text.switchAccount}</span>
                </button>
                <button
                  type="button"
                  onClick={() => onExport(selectedRow.account)}
                  disabled={exportingAccounts}
                >
                  <ActionIcon type="export" />
                  <span>{text.exportAccount}</span>
                </button>
                <button
                  type="button"
                  className="dangerAction"
                  onClick={() => onDelete(selectedRow.account)}
                >
                  <ActionIcon type="delete" />
                  <span>{pendingDeleteId === selectedRow.account.id ? copy.accountCard.deleteConfirm : text.deleteAccount}</span>
                </button>
              </div>
            </section>
          </>
        ) : (
          <div className="detailEmpty">
            <h3>{copy.accountsGrid.emptyTitle}</h3>
            <p>{copy.accountsGrid.emptyDescription}</p>
          </div>
        )}
      </aside>
    </section>
  );
}
