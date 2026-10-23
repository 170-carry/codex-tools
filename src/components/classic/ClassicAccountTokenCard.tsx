import type { AccountSummary, CodexAccountTokenUsage } from "../../types/app";
import { formatTokenCount } from "../../utils/usage";
import { formatFullDate } from "../../utils/dateFormatting";
import {
  formatCreditsBalance,
  formatLastActive,
  formatTps,
  getAccountStatsCopy,
} from "./accountTokenStats";

/** Detail-panel breakdown of local token usage, output speed and credits. */
export function ClassicAccountTokenCard({
  account,
  usage,
  locale,
  emptyValue,
}: {
  account: AccountSummary;
  usage: CodexAccountTokenUsage | undefined;
  locale: string;
  emptyValue: string;
}) {
  const copy = getAccountStatsCopy(locale);
  const credits = formatCreditsBalance(account, locale, copy);
  const windows = [
    { label: copy.last24h, value: usage?.last24h.totalTokens },
    { label: copy.last7d, value: usage?.last7d.totalTokens },
    { label: copy.last30d, value: usage?.last30d.totalTokens },
  ];

  return (
    <section className="detailCard accountTokenCard">
      <div className="detailCardTitle">
        <h3 title={copy.tokenCardHint}>{copy.tokenCardTitle}</h3>
        <span className="accountTokenCardCredits" title={copy.creditsTitle}>
          {copy.credits}
          <strong className={credits.empty ? "isEmpty" : undefined}>
            {credits.value}
          </strong>
        </span>
      </div>
      <p className="pageSectionDescription">{copy.tokenCardHint}</p>
      {usage ? (
        <>
          <div className="accountTokenWindows">
            {windows.map((item) => (
              <span key={item.label}>
                <em>{item.label}</em>
                <b>{usage ? formatTokenCount(item.value ?? 0, locale) : "—"}</b>
              </span>
            ))}
          </div>
          <div className="accountTpsRow">
            <div className="accountTpsHero">
              <em>{copy.avgTps}</em>
              <strong>
                {formatTps(usage?.outputTps24h, locale)}
                <small>tok/s</small>
              </strong>
            </div>
            <dl className="accountTpsFacts">
              <div>
                <dt>{copy.latestTps}</dt>
                <dd>{formatTps(usage?.latestOutputTps, locale)}</dd>
              </div>
              <div>
                <dt>{copy.tps7d}</dt>
                <dd>{formatTps(usage?.outputTps7d, locale)}</dd>
              </div>
              <div>
                <dt>{copy.responses}</dt>
                <dd>{usage ? usage.responseCount24h : "—"}</dd>
              </div>
              <div>
                <dt>{copy.lastActive}</dt>
                <dd
                  title={
                    usage?.lastActiveAt
                      ? formatFullDate(usage.lastActiveAt, locale, emptyValue)
                      : undefined
                  }
                >
                  {formatLastActive(usage?.lastActiveAt, locale, copy)}
                </dd>
              </div>
            </dl>
          </div>
        </>
      ) : (
        <p className="accountTokenEmpty">{copy.noUsage}</p>
      )}
    </section>
  );
}
