import type { AccountSummary } from "../types/app";

// 用户调整的账号组顺序优先，其余账号继续按导入时间排列；刷新额度不改变位置。
export function sortAccountsForDisplay(accounts: AccountSummary[], order: readonly string[] = []): AccountSummary[] {
  const ranks = new Map(order.map((key, index) => [key, index]));
  return [...accounts].sort((left, right) =>
    (ranks.get(left.accountKey) ?? Number.MAX_SAFE_INTEGER) -
      (ranks.get(right.accountKey) ?? Number.MAX_SAFE_INTEGER) ||
      left.addedAt - right.addedAt || left.id.localeCompare(right.id),
  );
}

export function moveAccountGroup(accounts: AccountSummary[], order: readonly string[], accountKey: string, direction: -1 | 1): string[] {
  const keys = [...new Set(sortAccountsForDisplay(accounts, order).map(account => account.accountKey))];
  const index = keys.indexOf(accountKey);
  const next = index + direction;
  if (index >= 0 && next >= 0 && next < keys.length) {
    [keys[index], keys[next]] = [keys[next], keys[index]];
  }
  return keys;
}
