import {
  useClassicAccounts,
  type ClassicAccountsProps,
} from "./useClassicAccounts";
import { AccountsWorkspace } from "../ui/AccountsWorkspace";
import { ClassicAccountList } from "./ClassicAccountList";
import { AccountInspector } from "./AccountInspector";

export function ClassicAccountsGrid(props: ClassicAccountsProps) {
  const workspace = useClassicAccounts(props);
  return (
    <AccountsWorkspace className="classicAccountsWorkspace" summary={workspace.leadingContent} busy={props.loading}
      inspector={workspace.inspectorOpen ? <AccountInspector workspace={workspace} /> : undefined}>
      <ClassicAccountList workspace={workspace} />
    </AccountsWorkspace>
  );
}
