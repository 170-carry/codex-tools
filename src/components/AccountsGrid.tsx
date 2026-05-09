import { useMemo } from "react";
import type { AccountSummary } from "../types/app";
import { useI18n } from "../i18n/I18nProvider";
import { AccountCard } from "./AccountCard";
import { compareAccountsByRemaining } from "../utils/accountRanking";

type AccountGroup = {
  id: string;
  variants: AccountSummary[];
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
  const priorityDiff = planPriority(left.planType ?? left.usage?.planType) - planPriority(right.planType ?? right.usage?.planType);
  if (priorityDiff !== 0) {
    return priorityDiff;
  }

  if (left.isCurrent !== right.isCurrent) {
    return left.isCurrent ? -1 : 1;
  }

  return compareAccountsByRemaining(left, right);
}

type AccountsGridProps = {
  accounts: AccountSummary[];
  loading: boolean;
  exportingAccounts: boolean;
  switchingId: string | null;
  renamingAccountId: string | null;
  pendingDeleteId: string | null;
  onExport: (account: AccountSummary) => void;
  onReauthorize: (account: AccountSummary) => void;
  onRename: (account: AccountSummary, label: string) => Promise<boolean>;
  onSwitch: (account: AccountSummary) => void;
  onDelete: (account: AccountSummary) => void;
};

export function AccountsGrid({
  accounts,
  loading,
  exportingAccounts,
  switchingId,
  renamingAccountId,
  pendingDeleteId,
  onExport,
  onReauthorize,
  onRename,
  onSwitch,
  onDelete,
}: AccountsGridProps) {
  const { copy } = useI18n();
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

  return (
    <section className="cards" aria-busy={loading}>
      {groupedAccounts.length === 0 && !loading && (
        <div className="emptyState">
          <h3>{copy.accountsGrid.emptyTitle}</h3>
          <p>{copy.accountsGrid.emptyDescription}</p>
        </div>
      )}

      {groupedAccounts.map((group) => (
        <AccountCard
          key={group.id}
          accounts={group.variants}
          exportingAccounts={exportingAccounts}
          switchingId={switchingId}
          renamingAccountId={renamingAccountId}
          pendingDeleteId={pendingDeleteId}
          onExport={onExport}
          onReauthorize={onReauthorize}
          onRename={onRename}
          onSwitch={onSwitch}
          onDelete={onDelete}
        />
      ))}
    </section>
  );
}
