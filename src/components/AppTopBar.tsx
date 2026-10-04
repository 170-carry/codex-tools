import { getCurrentWindow } from "@tauri-apps/api/window";
import type { MouseEvent } from "react";
import { useI18n } from "../i18n/I18nProvider";
import type { AppTab } from "../types/workspace";
import { WorkspaceIcon } from "./workspace/WorkspaceIcon";
import { ViewTabs } from "./workspace/ViewTabs";
import { getCompactTableCopy } from "../i18n/compactTableCopy";
import { LayoutPicker } from "./layout/LayoutPicker";

export function AppTopBar({
  activeTab,
  onSelectTab,
  onRefresh,
  refreshing,
  showRefresh,
  searchOpen,
  onSearch,
  onAddAccount,
}: {
  activeTab: AppTab;
  onSelectTab: (tab: AppTab) => void;
  onRefresh: () => void;
  refreshing: boolean;
  showRefresh: boolean;
  searchOpen: boolean;
  onSearch: () => void;
  onAddAccount: () => void;
}) {
  const { copy, locale } = useI18n();
  const text = getCompactTableCopy(locale);
  const handleDrag = (event: MouseEvent<HTMLDivElement>) => {
    if (event.buttons !== 1 || !("__TAURI_INTERNALS__" in window)) return;
    event.preventDefault();
    const appWindow = getCurrentWindow();
    if (event.detail === 2) void appWindow.toggleMaximize().catch(() => {});
    else void appWindow.startDragging().catch(() => {});
  };
  return (
    <header className="topbar">
      <div
        className="topDragRegion"
        aria-hidden="true"
        onMouseDown={handleDrag}
      />
      <div className="windowNavigation">
        <ViewTabs activeTab={activeTab} onSelectTab={onSelectTab} />
      </div>
      <div className="topActions">
        <LayoutPicker />
        {activeTab === "accounts" ? (
          <button
            type="button"
            className={`toolbarIconButton accountSearchButton${searchOpen ? " isSelected" : ""}`}
            aria-label={text.search}
            title={`${text.search} · ⌘F`}
            aria-expanded={searchOpen}
            aria-controls={searchOpen ? "account-search-toolbar" : undefined}
            onClick={onSearch}
          >
            <WorkspaceIcon name="search" />
          </button>
        ) : null}
        <button
          type="button"
          className={`toolbarIconButton settingsButton${activeTab === "settings" ? " isSelected" : ""}`}
          onClick={() => onSelectTab("settings")}
          title={`${copy.bottomDock.settings} · ⌘,`}
          aria-label={copy.bottomDock.settings}
          aria-pressed={activeTab === "settings"}
        >
          <WorkspaceIcon name="settings" />
        </button>
        {showRefresh ? (
          <button
            type="button"
            className="toolbarIconButton"
            onClick={onRefresh}
            disabled={refreshing}
            title={`${copy.topBar.manualRefresh} · ⌘R`}
            aria-label={
              refreshing ? copy.topBar.refreshing : copy.topBar.manualRefresh
            }
          >
            <WorkspaceIcon name="refresh" spinning={refreshing} />
          </button>
        ) : null}
        {activeTab === "accounts" ? (
          <button
            type="button"
            className="primary toolbarAddAccount addAccountButton"
            onClick={onAddAccount}
            aria-label={copy.addAccount.startButton}
            title={`${copy.addAccount.startButton} · ⌘N`}
          >
            <WorkspaceIcon name="plus" />
            <span>{copy.addAccount.startButton}</span>
          </button>
        ) : null}
      </div>
    </header>
  );
}
