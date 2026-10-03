import type { AccountSummary } from "../../types/app";

export function accountInitial(account: AccountSummary): string {
  const seed =
    account.email || account.label || account.accountId || account.accountKey;
  const normalized = seed.trim();
  return (normalized[0] || "?").toUpperCase();
}

export function displayAccountAddress(
  account: AccountSummary,
  emptyValue: string,
): string {
  return account.email || account.accountId || account.apiBaseUrl || emptyValue;
}

export function formatResetValue(
  epochSec: number | null | undefined,
  locale: string,
  emptyValue: string,
): string {
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

export function hasResetCredits(account: AccountSummary): boolean {
  const resetCredits = account.usage?.resetCredits;
  return Boolean(
    resetCredits &&
      (resetCredits.availableCount !== null || resetCredits.credits.length > 0),
  );
}
