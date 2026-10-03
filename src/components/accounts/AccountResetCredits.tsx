import type { AccountSummary } from "../../types/app";
import type { UiCopy } from "./types";
import { getCompactTableCopy } from "../../i18n/compactTableCopy";
import { useI18n } from "../../i18n/I18nProvider";
import { WorkspaceIcon } from "../workspace/WorkspaceIcon";

export function AccountResetCredits({
  account,
  text,
}: {
  account: AccountSummary;
  text: UiCopy;
}) {
  const { locale } = useI18n();
  if (account.sourceKind === "relay")
    return (
      <span
        className="accountResetCredits isUnavailable"
        title={`${text.resetCreditsTitle} · ${getCompactTableCopy(locale).unavailable}`}
        aria-label={`${text.resetCreditsTitle} · ${getCompactTableCopy(locale).unavailable}`}
      >
        <WorkspaceIcon name="refresh" />
        —
      </span>
    );
  const available = account.usage?.resetCredits?.availableCount;
  const count =
    typeof available === "number" &&
    Number.isFinite(available) &&
    available >= 0
      ? Math.floor(available)
      : null;
  const description = `${text.resetCreditsTitle} · ${text.resetCreditsAvailable(count)}`;
  return (
    <span
      className={`accountResetCredits${count === 0 ? " isEmpty" : ""}`}
      title={description}
      aria-label={description}
    >
      <WorkspaceIcon name="refresh" />
      <strong>{count ?? "—"}</strong>
    </span>
  );
}
