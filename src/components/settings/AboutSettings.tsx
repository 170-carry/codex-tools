import { getWorkspaceCopy } from "../../i18n/workspaceCopy";
import {
  PROJECT_CHANGELOG_URL,
  PROJECT_ISSUES_URL,
  PROJECT_RELEASES_URL,
  PROJECT_REPOSITORY_DISPLAY,
  PROJECT_REPOSITORY_URL,
} from "../../constants/externalLinks";
import type { SettingsWorkspace } from "./useSettingsWorkspace";
import { GitHubIcon } from "./GitHubIcon";
export function AboutSettings({ workspace }: { workspace: SettingsWorkspace }) {
  const {
    developerContent,
    locale,
    checkingUpdate,
    onCheckUpdate,
    onOpenExternalUrl,
    copy,
    versionValue,
  } = workspace;
  return (
    <div className="settingsGroup">
      <div className="settingRow">
        <div className="settingMeta settingMetaInline">
          <strong>{copy.settings.projectInfo.versionLabel}</strong>
          <span className="settingInlineValue">{versionValue}</span>
        </div>
        <div className="settingActionGroup">
          <button
            className="primary"
            onClick={onCheckUpdate}
            disabled={checkingUpdate}
          >
            {checkingUpdate
              ? copy.topBar.checkingUpdate
              : copy.topBar.checkUpdate}
          </button>
        </div>
      </div>

      <div className="settingRow">
        <a
          className="settingLink"
          href={PROJECT_REPOSITORY_URL}
          title={PROJECT_REPOSITORY_DISPLAY}
          onClick={(event) => {
            event.preventDefault();
            onOpenExternalUrl(PROJECT_REPOSITORY_URL);
          }}
        >
          <GitHubIcon />
          <span className="settingLinkLabel">{PROJECT_REPOSITORY_DISPLAY}</span>
        </a>
        <div className="settingActionGroup">
          <button
            className="ghost"
            onClick={() => onOpenExternalUrl(PROJECT_ISSUES_URL)}
          >
            {copy.settings.projectInfo.openIssues}
          </button>
        </div>
      </div>

      <div className="settingRow">
        <div className="settingMeta">
          <strong>{copy.settings.projectInfo.releasesLabel}</strong>
        </div>
        <div className="settingActionGroup">
          <button
            className="ghost"
            onClick={() => onOpenExternalUrl(PROJECT_RELEASES_URL)}
          >
            {copy.settings.projectInfo.openReleases}
          </button>
          <button
            className="ghost"
            onClick={() => onOpenExternalUrl(PROJECT_CHANGELOG_URL)}
          >
            {copy.settings.projectInfo.openChangelog}
          </button>
        </div>
      </div>
      {developerContent ? (
        <div className="settingRow">
          <div className="settingMeta">
            <strong>{getWorkspaceCopy(locale).developer}</strong>
          </div>
          <div className="settingActionGroup">{developerContent}</div>
        </div>
      ) : null}
    </div>
  );
}
