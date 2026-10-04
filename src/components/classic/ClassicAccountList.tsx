import { ClassicAccountToolbar } from "./ClassicAccountToolbar";
import { ClassicAccountRow } from "./ClassicAccountRow";
import { TokenUsageStrip } from "./ClassicTokenUsage";
import type { ClassicAccountsWorkspace } from "./useClassicAccounts";
export function ClassicAccountList({
  workspace,
}: {
  workspace: ClassicAccountsWorkspace;
}) {
  const {
    copy,
    locale,
    text,
    filteredRows,
    leadingContent,
    accounts,
    tokenUsage,
    tokenUsageError,
    loading,
    exportingAccounts,
    onExportAll,
  } = workspace;

  return (
    <div className="accountListStack">
      {leadingContent ? (
        <div className="accountListLeading">{leadingContent}</div>
      ) : null}
      <div className="accountListPanel">
        <ClassicAccountToolbar workspace={workspace} />

        {filteredRows.length === 0 && !loading ? (
          <div className="emptyState accountEmptyState">
            <h3>
              {accounts.length === 0
                ? copy.accountsGrid.emptyTitle
                : text.noMatchesTitle}
            </h3>
            <p>
              {accounts.length === 0
                ? copy.accountsGrid.emptyDescription
                : text.noMatchesDescription}
            </p>
          </div>
        ) : (
          <div className="accountListFrame">
            <TokenUsageStrip
              tokenUsage={tokenUsage}
              tokenUsageError={tokenUsageError}
              locale={locale}
              text={text}
              accountCount={accounts.length}
              exportingAccounts={exportingAccounts}
              onExportAll={onExportAll}
            />
            <div className="accountListHeader" aria-hidden="true">
              <span>{copy.bottomDock.accounts}</span>
              <span>{text.fiveHourUsage}</span>
              <span>{text.weekUsage}</span>
              <span>{text.resetTime}</span>
              <span>{text.proxyEnabled}</span>
              <span>{text.switchAccount}</span>
            </div>
            <div className="accountRows">
              {filteredRows.map((row) => (
                <ClassicAccountRow
                  key={row.id}
                  row={row}
                  workspace={workspace}
                />
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
