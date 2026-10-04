import {
  useClassicAccounts,
  type ClassicAccountsProps,
} from "./useClassicAccounts";
import { ClassicAccountList } from "./ClassicAccountList";
import { ClassicAccountDetail } from "./ClassicAccountDetail";

export function ClassicAccountsGrid(props: ClassicAccountsProps) {
  const workspace = useClassicAccounts(props);
  return (
    <section
      className="accountsWorkspace classicAccountsWorkspace"
      aria-busy={props.loading}
    >
      <ClassicAccountList workspace={workspace} />
      <ClassicAccountDetail workspace={workspace} />
    </section>
  );
}
