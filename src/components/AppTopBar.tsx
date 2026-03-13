import { useI18n } from "../i18n/I18nProvider";

type AppTopBarProps = {
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

export function AppTopBar({
  onRefresh,
  refreshing,
  onGoHome,
  showRefresh,
}: AppTopBarProps) {
  const { copy } = useI18n();

  return (
    <header className="topbar">
      <button type="button" className="brandLine homeLink" onClick={onGoHome}>
        <img className="appLogo" src="/codex-tools.png" alt={copy.topBar.logoAlt} />
        <h1>{copy.topBar.appTitle}</h1>
      </button>
      <div className="topDragRegion" data-tauri-drag-region aria-hidden="true" />
      <div className="topActions">
        {showRefresh ? (
          <button
            className="iconButton primary"
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
