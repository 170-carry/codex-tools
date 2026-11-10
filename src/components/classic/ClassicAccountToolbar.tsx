import type { StatusFilter } from "../accounts/types";
import { SearchField } from "../ui/SearchField";
import type { ClassicAccountsWorkspace } from "./useClassicAccounts";
export function ClassicAccountToolbar({
  workspace,
}: {
  workspace: ClassicAccountsWorkspace;
}) {
  const {
    text,
    query,
    setQuery,
    statusFilter,
    setStatusFilter,
    planFilter,
    setPlanFilter,
    toolbarActions,
  } = workspace;

  return (
    <div className="accountToolbar">
      <SearchField className="accountSearch" data-account-search value={query} onValueChange={setQuery}
        placeholder={text.searchPlaceholder} aria-label={text.searchPlaceholder} />
      <div className="accountFilters">
        <select
          aria-label={text.allStatuses}
          value={statusFilter}
          onChange={(event) =>
            setStatusFilter(event.currentTarget.value as StatusFilter)
          }
        >
          <option value="all">{text.allStatuses}</option>
          <option value="using">{text.statusUsing}</option>
          <option value="available">{text.statusAvailable}</option>
          <option value="low">{text.statusLow}</option>
          <option value="exhausted">{text.statusExhausted}</option>
          <option value="issue">{text.statusIssue}</option>
        </select>
        <select
          aria-label={text.allPlans}
          value={planFilter}
          onChange={(event) => setPlanFilter(event.currentTarget.value)}
        >
          <option value="all">{text.allPlans}</option>
          <option value="pro">PRO</option>
          <option value="plus">PLUS</option>
          <option value="team">TEAM</option>
          <option value="enterprise">ENTERPRISE</option>
          <option value="business">BUSINESS</option>
          <option value="api">API</option>
          <option value="free">FREE</option>
        </select>
      </div>
      {toolbarActions ? (
        <div className="accountToolbarActions">{toolbarActions}</div>
      ) : null}
    </div>
  );
}
