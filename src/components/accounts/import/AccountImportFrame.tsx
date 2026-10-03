import { useId, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../../../i18n/I18nProvider";
import { useModalFocus } from "../../../hooks/useModalFocus";
import { WorkspaceIcon } from "../../workspace/WorkspaceIcon";
import { AccountImportRouteIcon } from "./AccountImportRouteIcon";
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
}: {
  title: string;
  subtitle: string;
  routes: AccountImportRouteOption[];
  activeRoute: AccountImportRoute;
  description: string;
  closeBlocked: boolean;
  routeSwitchBlocked: boolean;
  onSelectRoute: (route: AccountImportRoute) => void;
  onClose: () => void;
  children: ReactNode;
}) {
  const { copy } = useI18n();
  const id = useId();
  const root = useRef<HTMLElement>(null);
  useModalFocus(root);
  return createPortal(
    <div
      className="settingsOverlay accountImportOverlay"
      onClick={(event) => {
        if (event.target === event.currentTarget && !closeBlocked) onClose();
      }}
    >
      <section
        ref={root}
        className={`settingsDialog addAuthDialog${routes.length === 1 ? " isSingleRoute" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={id}
        tabIndex={-1}
      >
        <header className="settingsHeader">
          <h2 id={id}>{title}</h2>
          <button
            type="button"
            className="toolbarIconButton closeButton"
            onClick={onClose}
            disabled={closeBlocked}
            title={copy.common.close}
            aria-label={copy.common.close}
          >
            <WorkspaceIcon name="close" />
          </button>
        </header>
        <p className="addAccountDialogSubtitle">{subtitle}</p>
        <div className="addAccountWorkspace">
          {routes.length > 1 ? (
            <nav
              className="addAccountTabs"
              aria-label={copy.addAccount.tabsAriaLabel}
            >
              {routes.map((route) => (
                <button
                  key={route.id}
                  type="button"
                  className={`addAccountTab${route.id === activeRoute ? " isActive" : ""}`}
                  aria-pressed={route.id === activeRoute}
                  disabled={routeSwitchBlocked}
                  onClick={() => onSelectRoute(route.id)}
                >
                  <span className="addAccountTabIcon">
                    <AccountImportRouteIcon route={route.id} />
                  </span>
                  <span className="addAccountTabContent">
                    <strong>{route.label}</strong>
                  </span>
                </button>
              ))}
            </nav>
          ) : null}
          <div
            className="addAccountPanel"
            aria-label={routes.find((route) => route.id === activeRoute)?.label}
          >
            <p className="addAccountRouteDescription">{description}</p>
            {children}
          </div>
        </div>
      </section>
    </div>,
    document.body,
  );
}
