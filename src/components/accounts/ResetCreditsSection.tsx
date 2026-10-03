import type { AccountSummary } from "../../types/app";
import type { UiCopy } from "./types";
import { formatFullDate } from "../../utils/dateFormatting";
import { hasResetCredits } from "./accountPresentation";

export function ResetCreditsSection({
  account,
  expanded,
  locale,
  text,
  onToggle,
}: {
  account: AccountSummary;
  expanded: boolean;
  locale: string;
  text: UiCopy;
  onToggle: () => void;
}) {
  const resetCredits = account.usage?.resetCredits;
  if (!resetCredits || !hasResetCredits(account)) {
    return null;
  }

  const visibleCredits = expanded
    ? resetCredits.credits
    : resetCredits.credits.slice(0, 2);
  const hiddenCount = Math.max(
    0,
    resetCredits.credits.length - visibleCredits.length,
  );

  return (
    <section className="detailCard resetCreditsCard">
      <div className="detailSectionTitle">
        <h3>{text.resetCreditsTitle}</h3>
        <span className="resetCreditsCount">
          {text.resetCreditsAvailable(resetCredits.availableCount ?? null)}
        </span>
      </div>
      {visibleCredits.length > 0 ? (
        <div className="resetCreditList">
          {visibleCredits.map((credit, index) => (
            <div
              className="resetCreditItem"
              key={`${credit.grantedAt ?? "unknown"}-${credit.expiresAt ?? "unknown"}-${index}`}
            >
              <span className="resetCreditIndex">{index + 1}</span>
              <div>
                <span>{text.resetCreditsExpiresAt}</span>
                <strong>
                  {formatFullDate(credit.expiresAt, locale, text.emptyValue)}
                </strong>
              </div>
            </div>
          ))}
        </div>
      ) : null}
      {resetCredits.credits.length > 2 ? (
        <button
          className="ghost resetCreditsToggle"
          type="button"
          onClick={onToggle}
          aria-expanded={expanded}
        >
          <svg
            className={
              expanded
                ? "resetCreditsToggleIcon isExpanded"
                : "resetCreditsToggleIcon"
            }
            viewBox="0 0 24 24"
            aria-hidden="true"
            focusable="false"
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
          <span>
            {expanded
              ? text.resetCreditsCollapse
              : text.resetCreditsExpand(hiddenCount)}
          </span>
        </button>
      ) : null}
    </section>
  );
}
