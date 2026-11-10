import type { AccountSummary, CodexAccountTokenUsage } from "../../types/app";
import type { UiCopy } from "../accounts/types";
import { AccountResetCredits } from "../accounts/AccountResetCredits";
import { formatTokenCount } from "../../utils/usage";
import { formatCreditsBalance, getAccountStatsCopy } from "./accountTokenStats";

export function AccountRowMetrics({ account, usage, locale, text }: {
  account: AccountSummary; usage?: CodexAccountTokenUsage; locale: string; text: UiCopy;
}) {
  const copy = getAccountStatsCopy(locale);
  const balance = formatCreditsBalance(account, locale, copy);
  return <span className="accountRowMetrics">
    <AccountResetCredits account={account} text={text} />
    <span title={`${copy.creditsTitle}: ${balance.value}`}><span>{copy.credits}</span><b>{balance.value}</b></span>
    <span title={`${copy.last24h} Token: ${formatTokenCount(usage?.last24h.totalTokens, locale)}`}><span>24h</span><b>{formatTokenCount(usage?.last24h.totalTokens, locale)}</b></span>
  </span>;
}
