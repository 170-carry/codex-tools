import { ClassicAccountDetail } from "./ClassicAccountDetail";
import { Inspector } from "../ui/Inspector";
import type { ClassicAccountsWorkspace } from "./useClassicAccounts";
export function AccountInspector({ workspace }: { workspace: ClassicAccountsWorkspace }) {
  return <Inspector title={workspace.selectedRow?.account.label || workspace.text.detailsTitle} onClose={workspace.closeInspector}><ClassicAccountDetail workspace={workspace} /></Inspector>;
}
