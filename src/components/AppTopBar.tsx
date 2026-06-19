import { getCurrentWindow } from "@tauri-apps/api/window";
import type { PointerEvent } from "react";

import { useI18n } from "../i18n/I18nProvider";
import type { ThemeMode } from "../types/app";

type AppTab = "accounts" | "analytics" | "proxy" | "settings";

type AppTopBarProps = {
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
    <svg className="iconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
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
    <svg className="iconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M20.5 14.6A8.5 8.5 0 0 1 9.4 3.5 8.7 8.7 0 1 0 20.5 14.6Z" />
    </svg>
  );
}

export function AppTopBar({
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
  const navItems: Array<{ id: AppTab; label: string }> = [
    { id: "accounts", label: copy.bottomDock.accounts },
    { id: "analytics", label: copy.bottomDock.analytics },
    { id: "proxy", label: copy.bottomDock.proxy },
    { id: "settings", label: copy.bottomDock.settings },
  ];
  const handleStartWindowDrag = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || !event.isPrimary || !("__TAURI_INTERNALS__" in window)) {
      return;
    }

    event.preventDefault();
    void getCurrentWindow().startDragging().catch(() => {});
  };

  return (
    <header className="topbar">
      <button type="button" className="brandLine homeLink" onClick={onGoHome}>
        <img className="appLogo" src="/codex-tools.png" alt={copy.topBar.logoAlt} />
        <h1>{copy.topBar.appTitle}</h1>
      </button>
      <div
        className="topDragRegion"
        data-tauri-drag-region
        aria-hidden="true"
        onPointerDown={handleStartWindowDrag}
      />
      <nav className="topSegmentedNav" aria-label={copy.bottomDock.ariaLabel}>
        {navItems.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`topSegmentedButton${activeTab === item.id ? " isActive" : ""}`}
            onClick={() => onSelectTab(item.id)}
            aria-pressed={activeTab === item.id}
          >
            {item.label}
          </button>
        ))}
      </nav>
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
            title={refreshing ? copy.topBar.refreshing : copy.topBar.manualRefresh}
            aria-label={refreshing ? copy.topBar.refreshing : copy.topBar.manualRefresh}
          >
            <RefreshIcon spinning={refreshing} />
          </button>
        ) : null}
      </div>
    </header>
  );
}
