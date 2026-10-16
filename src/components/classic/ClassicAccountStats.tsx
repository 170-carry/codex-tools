import type { AccountSummary, CodexAccountTokenUsage } from "../../types/app";
import { formatTokenCount } from "../../utils/usage";
import {
  formatCreditsBalance,
  formatTps,
  getAccountStatsCopy,
} from "./accountTokenStats";

/** Compact per-account figures shown in each list row. */
export function ClassicAccountStats({
  account,
  usage,
  locale,
}: {
  account: AccountSummary;
  usage: CodexAccountTokenUsage | undefined;
  locale: string;
}) {
  const copy = getAccountStatsCopy(locale);
  const credits = formatCreditsBalance(account, locale, copy);
  const tps = usage?.outputTps24h ?? null;

  return (
    <div className="accountStatsCell">
      <span
        className="accountStat"
        title={copy.tokenTitle(
          formatTokenCount(usage?.last7d.totalTokens ?? 0, locale),
          formatTokenCount(usage?.last30d.totalTokens ?? 0, locale),
        )}
      >
        <em>{copy.tokens24h}</em>
        <b className={usage?.last24h.totalTokens ? undefined : "isEmpty"}>
          {usage ? formatTokenCount(usage.last24h.totalTokens, locale) : "—"}
        </b>
      </span>
      <span
        className="accountStat"
        title={copy.tpsTitle(
          formatTps(tps, locale),
          formatTps(usage?.latestOutputTps, locale),
          formatTps(usage?.outputTps7d, locale),
        )}
      >
        <em>{copy.tps}</em>
        <b className={tps === null ? "isEmpty" : undefined}>
          {formatTps(tps, locale)}
        </b>
      </span>
      <span
        className="accountStat"
        title={`${copy.creditsTitle} · ${credits.value}`}
      >
        <em>{copy.credits}</em>
        <b className={credits.empty ? "isEmpty" : undefined}>{credits.value}</b>
      </span>
    </div>
  );
}
