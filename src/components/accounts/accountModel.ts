import type { AccountSummary } from "../../types/app";
import type { AccountStatus } from "./types";
import { compareAccountsByRemaining } from "../../utils/accountRanking";
import { remainingPercent } from "../../utils/usage";

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

export function planPriority(planType: string | null | undefined): number {
  const normalized = planType?.trim().toLowerCase() ?? "";
  return PLAN_PRIORITY[normalized] ?? PLAN_PRIORITY.unknown;
}

export function sortVariantsForGroup(
  left: AccountSummary,
  right: AccountSummary,
): number {
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

export function accountHasBlockingIssue(account: AccountSummary): boolean {
  return Boolean(
    account.authRefreshBlocked ||
      account.profileIntegrityError ||
      account.profileLastValidationError ||
      account.authRefreshError,
  );
}

export function accountIssueReason(
  account: AccountSummary,
  fallbackReason: string,
): string | null {
  return (
    account.profileIntegrityError ||
    account.profileLastValidationError ||
    account.authRefreshError ||
    (account.authRefreshBlocked ? fallbackReason : null)
  );
}

export function accountHasExhaustedWindow(account: AccountSummary): boolean {
  return [account.usage?.fiveHour ?? null, account.usage?.oneWeek ?? null].some(
    (window) => {
      const remaining = remainingPercent(window);
      return remaining !== null && remaining <= 0;
    },
  );
}

export function lowestRemaining(account: AccountSummary): number | null {
  const values = [
    remainingPercent(account.usage?.fiveHour ?? null),
    remainingPercent(account.usage?.oneWeek ?? null),
  ].filter((value): value is number => value !== null && !Number.isNaN(value));
  return values.length > 0 ? Math.min(...values) : null;
}

export function accountStatus(account: AccountSummary): AccountStatus {
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
