import { NavigationIcon } from "../workspace/NavigationIcon";
import {
  ProviderSwitch,
  type AccountProvider,
} from "../workspace/ProviderSwitch";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { MouseEvent } from "react";
import { preloadWorkspaceView } from "../workspace/viewModules";

import { useI18n } from "../../i18n/I18nProvider";
import type { ThemeMode } from "../../types/app";

type AppTab = "accounts" | "analytics" | "proxy" | "settings";

type AppTopBarProps = {
  provider: AccountProvider;
  onProviderChange: (provider: AccountProvider) => void;
  activeTab: AppTab;
  onSelectTab: (tab: AppTab) => void;
  themeMode: ThemeMode;
  onToggleTheme: () => void;
  onRefresh: () => void;
  refreshing: boolean;
  onGoHome: () => void;
  showRefresh: boolean;
};

function RefreshIcon({ spinning }: { spinning: boolean }) {
  return (
    <svg
      className={`iconGlyph ${spinning ? "isSpinning" : ""}`}
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M21 12a9 9 0 1 1-2.64-6.36" />
      <path d="M21 3v6h-6" />
    </svg>
  );
}

function SunIcon() {
  return (
    <svg
      className="iconGlyph"
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2" />
      <path d="M12 20v2" />
      <path d="m4.93 4.93 1.41 1.41" />
      <path d="m17.66 17.66 1.41 1.41" />
      <path d="M2 12h2" />
      <path d="M20 12h2" />
      <path d="m6.34 17.66-1.41 1.41" />
      <path d="m19.07 4.93-1.41 1.41" />
    </svg>
  );
}

function MoonIcon() {
  return (
    <svg
      className="iconGlyph"
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M20.5 14.6A8.5 8.5 0 0 1 9.4 3.5 8.7 8.7 0 1 0 20.5 14.6Z" />
    </svg>
  );
}

export function ClassicTopBar({
  provider,
  onProviderChange,
  activeTab,
  onSelectTab,
  themeMode,
  onToggleTheme,
  onRefresh,
  refreshing,
  onGoHome,
  showRefresh,
}: AppTopBarProps) {
  const { copy } = useI18n();
  const allNavItems: Array<{ id: AppTab; label: string }> = [
    { id: "accounts", label: copy.bottomDock.accounts },
    { id: "analytics", label: copy.bottomDock.analytics },
    { id: "proxy", label: copy.bottomDock.proxy },
    { id: "settings", label: copy.bottomDock.settings },
  ];
  const navItems =
    provider === "claude"
      ? allNavItems.filter(
          (item) => item.id === "accounts" || item.id === "settings",
        )
      : allNavItems;
  const handleStartWindowDrag = (event: MouseEvent<HTMLDivElement>) => {
    if (event.buttons !== 1 || !("__TAURI_INTERNALS__" in window)) {
      return;
    }

    event.preventDefault();
    const appWindow = getCurrentWindow();
    // Tauri 手动拖动契约使用 mousedown 的点击计数；双击不能再进入 startDragging。
    if (event.detail === 2) {
      void appWindow.toggleMaximize().catch(() => {});
      return;
    }

    void appWindow.startDragging().catch(() => {});
  };

  return (
    <>
      <aside className="appSidebar">
        <div
          className="sidebarDragRegion"
          aria-hidden="true"
          onMouseDown={handleStartWindowDrag}
        />
        <button
          type="button"
          className="sidebarBrand"
          onClick={onGoHome}
          title={copy.topBar.appTitle}
        >
          <img src="/codex-tools.png" alt={copy.topBar.logoAlt} />
          <span>Codex Tools</span>
        </button>
        <ProviderSwitch provider={provider} onChange={onProviderChange} />
        <nav
          className="sidebarNavigation"
          aria-label={copy.bottomDock.ariaLabel}
        >
          {navItems.map((item) => (
            <button
              key={item.id}
              type="button"
              className={`sidebarItem${activeTab === item.id ? " isActive" : ""}${item.id === "settings" ? " isSettings" : ""}`}
              onClick={() => onSelectTab(item.id)}
              onPointerEnter={() => preloadWorkspaceView(item.id)}
              onFocus={() => preloadWorkspaceView(item.id)}
              aria-current={activeTab === item.id ? "page" : undefined}
              title={item.label}
              aria-label={item.label}
            >
              <NavigationIcon name={item.id} />
              <span>{item.label}</span>
            </button>
          ))}
        </nav>
      </aside>
      <header className="appToolbar">
        <div className="toolbarHeading">
          <span>{activeTab === "settings" ? "Codex Tools" : provider === "codex" ? "Codex" : "Claude"}</span>
          <h2>{navItems.find((item) => item.id === activeTab)?.label}</h2>
        </div>
        <div
          className="toolbarDragRegion"
          aria-hidden="true"
          onMouseDown={handleStartWindowDrag}
        />
        <div className="topActions">
          <button
            className="iconButton"
            onClick={onToggleTheme}
            title={copy.settings.theme.switchAriaLabel}
            aria-label={copy.settings.theme.switchAriaLabel}
            type="button"
          >
            {themeMode === "dark" ? <SunIcon /> : <MoonIcon />}
          </button>
          {showRefresh ? (
            <button
              className="iconButton"
              onClick={onRefresh}
              disabled={refreshing}
              title={
                refreshing ? copy.topBar.refreshing : copy.topBar.manualRefresh
              }
              aria-label={
                refreshing ? copy.topBar.refreshing : copy.topBar.manualRefresh
              }
            >
              <RefreshIcon spinning={refreshing} />
            </button>
          ) : null}
        </div>
      </header>
    </>
  );
}
