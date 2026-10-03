import type { KeyboardEvent } from "react";
import { useI18n } from "../../i18n/I18nProvider";
import { getWorkspaceCopy } from "../../i18n/workspaceCopy";
import type { AppTab } from "../../types/workspace";
import { WorkspaceIcon } from "./WorkspaceIcon";

const tabs: AppTab[] = ["accounts", "analytics", "proxy"];

export function ViewTabs({
  activeTab,
  onSelectTab,
}: {
  activeTab: AppTab;
  onSelectTab: (tab: AppTab) => void;
}) {
  const { copy, locale } = useI18n();
  const onKeyDown = (
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? tabs.length - 1
          : (index + (event.key === "ArrowLeft" ? -1 : 1) + tabs.length) %
            tabs.length;
    const target =
      event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>(
        "[role=tab]",
      )[next];
    onSelectTab(tabs[next]);
    target?.focus();
  };
  return (
    <nav
      className="viewTabs"
      role="tablist"
      aria-label={getWorkspaceCopy(locale).navigation}
    >
      {tabs.map((tab, index) => (
        <button
          key={tab}
          type="button"
          role="tab"
          className={`viewTab${activeTab === tab ? " isActive" : ""}`}
          aria-selected={activeTab === tab}
          aria-controls="workspace-content"
          tabIndex={
            activeTab === tab || (activeTab === "settings" && index === 0)
              ? 0
              : -1
          }
          onClick={() => onSelectTab(tab)}
          onKeyDown={(event) => onKeyDown(event, index)}
          title={`${copy.bottomDock[tab]} · ⌘${index + 1}`}
        >
          <WorkspaceIcon name={tab} />
          <span>{copy.bottomDock[tab]}</span>
        </button>
      ))}
    </nav>
  );
}
