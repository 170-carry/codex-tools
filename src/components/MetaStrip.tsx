import { useI18n } from "../i18n/I18nProvider";
import type { AccountSummary } from "../types/app";
import {
  accountHasBlockingIssue,
  accountHasExhaustedWindow,
} from "./accounts/accountModel";
import { WorkspaceIcon } from "./workspace/WorkspaceIcon";

export function MetaStrip({
  accounts,
  exportingAccounts,
  onExportAccounts,
}: {
  accounts: AccountSummary[];
  exportingAccounts: boolean;
  onExportAccounts: () => void;
}) {
  const { copy, locale } = useI18n();
  const chinese = locale === "zh-CN";
  const issues = accounts.filter(accountHasBlockingIssue).length;
  const exhausted = accounts.filter(
    (a) => !accountHasBlockingIssue(a) && accountHasExhaustedWindow(a),
  ).length;
  const available = accounts.filter(
    (a) =>
      !accountHasBlockingIssue(a) &&
      !accountHasExhaustedWindow(a) &&
      a.profileAuthReady,
  ).length;
  return (
    <section className="accountSummary" aria-label={copy.metaStrip.ariaLabel}>
      <div className="accountSummaryCounts">
        <span>
          {accounts.length} {chinese ? "个账号" : "accounts"}
        </span>
        <span className="summaryAvailable">
          {available} {chinese ? "可用" : "available"}
        </span>
        <span>
          {exhausted} {chinese ? "已耗尽" : "exhausted"}
        </span>
        {issues ? (
          <span className="summaryIssue">
            {issues} {chinese ? "异常" : "need attention"}
          </span>
        ) : null}
      </div>
      <button
        type="button"
        className="accountExportAll"
        onClick={onExportAccounts}
        disabled={exportingAccounts || accounts.length === 0}
      >
        <WorkspaceIcon name="export" />
        {copy.metaStrip.exportAll}
      </button>
    </section>
  );
}
