import type { AccountSummary, UsageWindow } from "../types/app";

const UNKNOWN_REMAINING = -1;

function windowRemainingPercent(window: UsageWindow | null | undefined): number {
  if (!window || window.usedPercent === null || window.usedPercent === undefined) {
    return UNKNOWN_REMAINING;
  }
  const remaining = 100 - window.usedPercent;
  return Math.max(0, Math.min(100, remaining));
}

function accountRemainingScore(account: AccountSummary): {
  oneWeek: number;
  fiveHour: number;
} {
  return {
    oneWeek: windowRemainingPercent(account.usage?.oneWeek),
    fiveHour: windowRemainingPercent(account.usage?.fiveHour),
  };
}

function accountHasPositiveRemaining(account: AccountSummary): boolean {
  const score = accountRemainingScore(account);
  return score.oneWeek > 0 || score.fiveHour > 0;
}

function accountHasKnownRemaining(account: AccountSummary): boolean {
  const score = accountRemainingScore(account);
  return score.oneWeek >= 0 || score.fiveHour >= 0;
}

export function compareAccountsByRemaining(a: AccountSummary, b: AccountSummary): number {
  if (a.sourceKind !== b.sourceKind) {
    return a.sourceKind === "chatgpt" ? -1 : 1;
  }

  const left = accountRemainingScore(a);
  const right = accountRemainingScore(b);

  // 优先比较 1week 余量，再比较 5h 余量，保证排序/智能切换口径一致。
  if (right.oneWeek !== left.oneWeek) {
    return right.oneWeek - left.oneWeek;
  }
  if (right.fiveHour !== left.fiveHour) {
    return right.fiveHour - left.fiveHour;
  }

  // 余量一致时，优先展示当前账号，再按标签稳定排序。
  if (a.isCurrent !== b.isCurrent) {
    return a.isCurrent ? -1 : 1;
  }
  return a.label.localeCompare(b.label, "zh-Hans-CN");
}

export function sortAccountsByRemaining(accounts: AccountSummary[]): AccountSummary[] {
  return [...accounts].sort(compareAccountsByRemaining);
}

export function pickBestRemainingAccount(accounts: AccountSummary[]): AccountSummary | null {
  if (accounts.length === 0) {
    return null;
  }
  return sortAccountsByRemaining(accounts)[0] ?? null;
}

export function pickBestSmartSwitchAccount(
  accounts: AccountSummary[],
  includeApiFallback: boolean,
): AccountSummary | null {
  const sorted = sortAccountsByRemaining(accounts);
  if (sorted.length === 0) {
    return null;
  }

  if (!includeApiFallback) {
    return sorted[0] ?? null;
  }

  const chatgptAccounts = sorted.filter((account) => account.sourceKind === "chatgpt");
  if (chatgptAccounts.length === 0) {
    return sorted[0] ?? null;
  }

  const bestChatgptWithRemaining = chatgptAccounts.find(accountHasPositiveRemaining);
  if (bestChatgptWithRemaining) {
    return bestChatgptWithRemaining;
  }

  const allChatgptKnownExhausted = chatgptAccounts.every(
    (account) => accountHasKnownRemaining(account) && !accountHasPositiveRemaining(account),
  );
  if (!allChatgptKnownExhausted) {
    return chatgptAccounts[0] ?? sorted[0] ?? null;
  }

  const relayAccounts = sorted.filter((account) => account.sourceKind === "relay");
  return relayAccounts[0] ?? chatgptAccounts[0] ?? sorted[0] ?? null;
}
