type AppTopBarProps = {
  onOpenSettings: () => void;
  onCheckUpdate: () => void;
  checkingUpdate: boolean;
  installingUpdate: boolean;
  onRefresh: () => void;
  refreshing: boolean;
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

function UpdateIcon({ spinning }: { spinning: boolean }) {
  return (
    <svg
      className={`iconGlyph ${spinning ? "isSpinning" : ""}`}
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M3 12a9 9 0 0 1 15.54-6.36" />
      <path d="M18.54 3.64v4.92h-4.9" />
      <path d="M21 12a9 9 0 0 1-15.54 6.36" />
      <path d="M5.46 20.36v-4.92h4.9" />
    </svg>
  );
}

function SettingsIcon() {
  return (
    <svg className="iconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z" />
      <path d="M19.4 15a1 1 0 0 0 .2 1.1l.1.1a1.9 1.9 0 1 1-2.7 2.7l-.1-.1a1 1 0 0 0-1.1-.2 1 1 0 0 0-.6.9V20a1.9 1.9 0 1 1-3.8 0v-.2a1 1 0 0 0-.6-.9 1 1 0 0 0-1.1.2l-.1.1a1.9 1.9 0 1 1-2.7-2.7l.1-.1a1 1 0 0 0 .2-1.1 1 1 0 0 0-.9-.6H4a1.9 1.9 0 1 1 0-3.8h.2a1 1 0 0 0 .9-.6 1 1 0 0 0-.2-1.1l-.1-.1a1.9 1.9 0 1 1 2.7-2.7l.1.1a1 1 0 0 0 1.1.2h.1a1 1 0 0 0 .6-.9V4a1.9 1.9 0 1 1 3.8 0v.2a1 1 0 0 0 .6.9 1 1 0 0 0 1.1-.2l.1-.1a1.9 1.9 0 1 1 2.7 2.7l-.1.1a1 1 0 0 0-.2 1.1v.1a1 1 0 0 0 .9.6H20a1.9 1.9 0 1 1 0 3.8h-.2a1 1 0 0 0-.9.6Z" />
    </svg>
  );
}

export function AppTopBar({
  onOpenSettings,
  onCheckUpdate,
  checkingUpdate,
  installingUpdate,
  onRefresh,
  refreshing,
}: AppTopBarProps) {
  const checking = checkingUpdate || installingUpdate;

  return (
    <header className="topbar">
      <div className="topDragRegion" data-tauri-drag-region>
        <div className="brandLine">
          <img className="appLogo" src="/codex-tools.png" alt="Codex Tools logo" />
          <h1>Codex Tools</h1>
        </div>
      </div>
      <div className="topActions">
        <button
          className="iconButton ghost"
          onClick={onCheckUpdate}
          disabled={checking}
          title={checking ? "检查更新中..." : "检查更新"}
          aria-label={checking ? "检查更新中" : "检查更新"}
        >
          <UpdateIcon spinning={checking} />
        </button>
        <button
          className="iconButton primary"
          onClick={onRefresh}
          disabled={refreshing}
          title={refreshing ? "刷新中..." : "手动刷新"}
          aria-label={refreshing ? "刷新中" : "手动刷新"}
        >
          <RefreshIcon spinning={refreshing} />
        </button>
        <button
          className="iconButton ghost"
          onClick={onOpenSettings}
          title="打开设置"
          aria-label="打开设置"
        >
          <SettingsIcon />
        </button>
      </div>
    </header>
  );
}
