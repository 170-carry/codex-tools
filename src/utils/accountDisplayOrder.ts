import type { AccountSummary } from "../types/app";

// The list uses immutable import order; quota ranking is reserved for smart switch.
export function sortAccountsForDisplay(accounts: AccountSummary[]): AccountSummary[] {
  return [...accounts].sort((left, right) =>
    left.addedAt - right.addedAt || left.id.localeCompare(right.id),
  );
}
