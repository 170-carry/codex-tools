import { useMemo, useState } from "react";
import type { AccountSummary } from "../types/app";
import type {
  AccountGroup,
  AccountRow,
  AccountsGridProps,
  StatusFilter,
  SwitchRecord,
} from "../components/accounts/types";
import {
  accountStatus,
  sortVariantsForGroup,
} from "../components/accounts/accountModel";
import { displayAccountAddress } from "../components/accounts/accountPresentation";
import { compareAccountsByRemaining } from "../utils/accountRanking";

export function useAccountsWorkspace(props: AccountsGridProps) {
  const { accounts, switchingId, authBusy, onSwitch } = props;
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [planFilter, setPlanFilter] = useState("all");
  const [inspectedId, setInspectedId] = useState<string | null>(null);
  const [preferredVariants, setPreferredVariants] = useState<
    Record<string, string>
  >({});
  const [switchRecords, setSwitchRecords] = useState<SwitchRecord[]>([]);

  const rows = useMemo<AccountRow[]>(() => {
    const grouped = new Map<string, AccountSummary[]>();
    for (const account of accounts) {
      const existing = grouped.get(account.accountKey);
      if (existing) existing.push(account);
      else grouped.set(account.accountKey, [account]);
    }
    const groups: AccountGroup[] = Array.from(grouped, ([id, variants]) => ({
      id,
      variants: [...variants].sort(sortVariantsForGroup),
    }));
    return groups
      .map((group) => ({
        ...group,
        account:
          group.variants.find((a) => a.id === switchingId) ||
          group.variants.find((a) => a.id === preferredVariants[group.id]) ||
          group.variants.find((a) => a.isCurrent) ||
          group.variants[0],
      }))
      .sort((a, b) =>
        a.account.isCurrent !== b.account.isCurrent
          ? a.account.isCurrent
            ? -1
            : 1
          : compareAccountsByRemaining(a.account, b.account),
      );
  }, [accounts, preferredVariants, switchingId]);

  const filteredRows = useMemo(() => {
    const search = query.trim().toLowerCase();
    return rows.filter(({ account, variants }) => {
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
        (!search || haystack.includes(search)) &&
        (statusFilter === "all" ||
          (statusFilter === "using"
            ? variants.some((a) => a.isCurrent)
            : accountStatus(account) === statusFilter)) &&
        (planFilter === "all" ||
          (
            account.planType ||
            account.usage?.planType ||
            "unknown"
          ).toLowerCase() === planFilter)
      );
    });
  }, [rows, query, statusFilter, planFilter]);

  const inspectedAccount = accounts.find((a) => a.id === inspectedId) ?? null;
  const currentAccount = accounts.find((a) => a.isCurrent) ?? null;
  const selectVariant = (groupId: string, accountId: string) =>
    setPreferredVariants((current) => ({ ...current, [groupId]: accountId }));
  const switchAccount = async (account: AccountSummary) => {
    if (authBusy || account.isCurrent) return;
    const source = currentAccount;
    if (!(await onSwitch(account))) return;
    const timestamp = Math.floor(Date.now() / 1000);
    setSwitchRecords((current) =>
      [
        {
          id: `${account.id}-${timestamp}-${current.length}`,
          target: displayAccountAddress(account, "--"),
          source: source ? displayAccountAddress(source, "--") : "--",
          timestamp,
        },
        ...current,
      ].slice(0, 5),
    );
  };
  return {
    rows,
    filteredRows,
    currentAccount,
    inspectedAccount,
    inspect: setInspectedId,
    query,
    setQuery,
    statusFilter,
    setStatusFilter,
    planFilter,
    setPlanFilter,
    selectVariant,
    switchAccount,
    switchRecords,
  };
}
