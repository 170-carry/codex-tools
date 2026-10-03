import { useI18n } from "../i18n/I18nProvider";
import { useAccountsWorkspace } from "../hooks/useAccountsWorkspace";
import type { AccountsGridProps } from "./accounts/types";
import { getUiCopy } from "./accounts/accountCopy";
import { AccountToolbar } from "./accounts/AccountToolbar";
import { AccountListRow } from "./accounts/AccountListRow";
import { AccountInspector } from "./accounts/AccountInspector";
import { AccountsFooter } from "./accounts/AccountsFooter";
import { getCompactTableCopy } from "../i18n/compactTableCopy";

export function AccountsGrid(props: AccountsGridProps) {
  const { copy, locale } = useI18n();
  const text = getUiCopy(locale);
  const tableText = getCompactTableCopy(locale);
  const workspace = useAccountsWorkspace(props);
  return (
    <section className="accountsWorkspace" aria-busy={props.loading}>
      <div className="accountListPanel">
        {props.searchVisible ? (
          <AccountToolbar
            query={workspace.query}
            onQuery={workspace.setQuery}
            status={workspace.statusFilter}
            onStatus={workspace.setStatusFilter}
            plan={workspace.planFilter}
            onPlan={workspace.setPlanFilter}
            text={text}
            locale={locale}
            onClose={props.onCloseSearch}
          />
        ) : null}
        <div className="accountRows">
          {props.loading && workspace.rows.length === 0 ? (
            <div className="accountLoading" role="status">
              {copy.accountsGrid.usageRefreshing}
            </div>
          ) : workspace.filteredRows.length === 0 ? (
            <div className="emptyState accountEmptyState">
              <h3>
                {props.accounts.length === 0
                  ? copy.accountsGrid.emptyTitle
                  : text.noMatchesTitle}
              </h3>
              <p>
                {props.accounts.length === 0
                  ? copy.accountsGrid.emptyDescription
                  : text.noMatchesDescription}
              </p>
            </div>
          ) : (
            <table className="accountTable">
              <colgroup>
                <col className="accountIdentityColumn" />
                <col className="accountQuotaColumn" />
                <col className="accountQuotaColumn" />
                <col className="accountCreditsColumn" />
                <col className="accountActionsColumn" />
              </colgroup>
              <thead>
                <tr>
                  <th scope="col">{copy.bottomDock.accounts}</th>
                  <th scope="col">
                    {tableText.fiveHourRemaining}
                    <span className="accountColumnHint">
                      {tableText.resetTime}
                    </span>
                  </th>
                  <th scope="col">
                    {tableText.weeklyRemaining}
                    <span className="accountColumnHint">
                      {tableText.resetTime}
                    </span>
                  </th>
                  <th scope="col" className="accountCreditsHead">
                    {tableText.resetCredits}
                  </th>
                  <th scope="col">
                    <span className="visuallyHidden">{text.quickActions}</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {workspace.filteredRows.map((row) => (
                  <AccountListRow
                    key={row.id}
                    row={row}
                    actions={props}
                    text={text}
                    onInspect={workspace.inspect}
                    onVariant={workspace.selectVariant}
                    onSwitch={workspace.switchAccount}
                  />
                ))}
              </tbody>
            </table>
          )}
        </div>
        <AccountsFooter actions={props} />
      </div>
      {workspace.inspectedAccount ? (
        <AccountInspector
          key={workspace.inspectedAccount.id}
          account={workspace.inspectedAccount}
          actions={props}
          text={text}
          records={workspace.switchRecords}
          onClose={() => workspace.inspect(null)}
          onSwitch={workspace.switchAccount}
        />
      ) : null}
    </section>
  );
}
