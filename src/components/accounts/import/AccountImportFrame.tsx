import type { ReactNode } from "react";
import { useI18n } from "../../../i18n/I18nProvider";
import { UtilityDialog } from "../../workspace/UtilityDialog";
import { ImportMethods } from "../../ui/ImportMethods";
import type { AccountImportRoute, AccountImportRouteOption } from "./types";

export function AccountImportFrame({
  title,
  subtitle,
  routes,
  activeRoute,
  description,
  closeBlocked,
  routeSwitchBlocked,
  onSelectRoute,
  onClose,
  children,
  actions,
}: {
  title: string;
  subtitle?: string;
  routes: AccountImportRouteOption[];
  activeRoute: AccountImportRoute;
  description?: string;
  closeBlocked: boolean;
  routeSwitchBlocked: boolean;
  onSelectRoute: (route: AccountImportRoute) => void;
  onClose: () => void;
  children: ReactNode;
  actions?: ReactNode;
}) {
  const { copy } = useI18n();
  return <UtilityDialog className="addAuthDialog" title={title} description={subtitle} closeLabel={copy.common.close} onClose={onClose} dismissible={!closeBlocked} actions={actions}>
    <div className={`uiImportWorkspace${routes.length === 1 ? " isSingleRoute" : ""}`}>
      {routes.length > 1 && <ImportMethods methods={routes} value={activeRoute} disabled={routeSwitchBlocked} onChange={onSelectRoute} />}
      <div className="uiImportBody addAccountPanel" aria-label={routes.find(route => route.id === activeRoute)?.label}>{description && <p className="addAccountRouteDescription">{description}</p>}{children}</div>
    </div>
  </UtilityDialog>;
}
