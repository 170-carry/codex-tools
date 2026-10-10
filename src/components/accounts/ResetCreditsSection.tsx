import { useState } from "react";
import { useI18n } from "../../i18n/I18nProvider";
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
  busy = false,
  onUseCredit,
}: {
  account: AccountSummary;
  expanded: boolean;
  locale: string;
  text: UiCopy;
  onToggle: () => void;
  busy?: boolean;
  onUseCredit?: (account: AccountSummary, creditId: string) => Promise<boolean>;
}) {
  const { copy } = useI18n();
  const [confirming, setConfirming] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const resetCredits = account.usage?.resetCredits;
  if (!resetCredits || !hasResetCredits(account)) {
    return null;
  }

  const availableCredits = resetCredits.credits.filter(
    (credit) => !credit.status || credit.status === "available",
  );
  const selectedCredit = availableCredits.find((credit) => credit.id === confirming);
  const visibleCredits = expanded
    ? availableCredits
    : availableCredits.slice(0, 2);
  const hiddenCount = Math.max(
    0,
    availableCredits.length - visibleCredits.length,
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
              key={credit.id ?? `${credit.grantedAt ?? "unknown"}-${credit.expiresAt ?? "unknown"}-${index}`}
            >
              <span className="resetCreditIndex">{index + 1}</span>
              <div>
                <span>{text.resetCreditsExpiresAt}</span>
                <strong>
                  {formatFullDate(credit.expiresAt, locale, text.emptyValue)}
                </strong>
              </div>
              {onUseCredit && credit.id && credit.status === "available" &&
              (!credit.expiresAt || credit.expiresAt > Date.now() / 1000) ? (
                <button
                  type="button"
                  className="ghost"
                  disabled={busy || submitting}
                  onClick={() => setConfirming(credit.id!)}
                >
                  {copy.resetCredit.use}
                </button>
              ) : null}
            </div>
          ))}
        </div>
      ) : null}
      {confirming && selectedCredit ? (
        <div className="resetCreditConfirmation" role="alert">
          <p>
            {copy.resetCredit.confirm} · {account.label} · {formatFullDate(
              selectedCredit.expiresAt, locale, text.emptyValue,
            )}
          </p>
          <div className="settingActionGroup">
            <button
              type="button"
              className="ghost"
              disabled={submitting}
              onClick={() => setConfirming(null)}
            >
              {text.cancel}
            </button>
            <button
              type="button"
              disabled={busy || submitting}
              onClick={async () => {
                setSubmitting(true);
                try {
                  await onUseCredit?.(account, confirming);
                } finally {
                  setSubmitting(false);
                  setConfirming(null);
                }
              }}
            >
              {submitting ? copy.resetCredit.submitting : copy.resetCredit.confirmUse}
            </button>
          </div>
        </div>
      ) : null}
      {availableCredits.length > 2 ? (
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
