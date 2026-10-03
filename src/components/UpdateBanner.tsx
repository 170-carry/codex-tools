import { useI18n } from "../i18n/I18nProvider";
import type { PendingUpdateInfo } from "../types/app";
import {
  getChangelogEntryForVersion,
  normalizeReleaseNoteItems,
} from "../utils/changelog";
import { UtilityDialog } from "./workspace/UtilityDialog";

type UpdateBannerProps = {
  open: boolean;
  pendingUpdate: PendingUpdateInfo | null;
  updateProgress: string | null;
  installingUpdate: boolean;
  onClose: () => void;
  onManualDownload: () => void;
  onSkipVersion: () => void;
  onInstallNow: () => void;
};

export function UpdateBanner({
  open,
  pendingUpdate,
  updateProgress,
  installingUpdate,
  onClose,
  onManualDownload,
  onSkipVersion,
  onInstallNow,
}: UpdateBannerProps) {
  const { copy, locale } = useI18n();
  if (!open || !pendingUpdate) return null;
  const changelogEntry = getChangelogEntryForVersion(
    pendingUpdate.version,
    locale,
  );
  const releaseNoteItems = changelogEntry?.items.length
    ? changelogEntry.items
    : normalizeReleaseNoteItems(pendingUpdate.body, locale);
  return (
    <UtilityDialog
      className="updateDialog"
      title={copy.updateDialog.title(pendingUpdate.version)}
      description={copy.updateDialog.subtitle(pendingUpdate.currentVersion)}
      closeLabel={copy.updateDialog.close}
      onClose={onClose}
      dismissible={!installingUpdate}
      actions={
        <>
          <button
            type="button"
            className="ghost"
            onClick={onSkipVersion}
            disabled={installingUpdate}
          >
            {copy.updateDialog.skipThisVersion}
          </button>
          <button
            type="button"
            className="ghost"
            onClick={onManualDownload}
            disabled={installingUpdate}
          >
            {copy.updateDialog.manualDownload}
          </button>
          <button
            type="button"
            className="primary"
            onClick={onInstallNow}
            disabled={installingUpdate}
          >
            {installingUpdate
              ? copy.updateDialog.installingNow
              : copy.updateDialog.installNow}
          </button>
        </>
      }
    >
      <div className="updateText">
        {pendingUpdate.date ? (
          <span>{copy.updateDialog.publishedAt(pendingUpdate.date)}</span>
        ) : null}
        <span>
          {installingUpdate
            ? copy.updateDialog.statusInstalling
            : copy.updateDialog.statusReady}
        </span>
      </div>
      <div className="updateChangelog">
        <strong>{copy.updateDialog.changelogTitle}</strong>
        {releaseNoteItems.length > 0 ? (
          <ul className="updateChangelogList">
            {releaseNoteItems.map((item, index) => (
              <li key={`${index}-${item}`}>{item}</li>
            ))}
          </ul>
        ) : (
          <p className="updateChangelogEmpty">
            {copy.updateDialog.changelogEmpty}
          </p>
        )}
      </div>
      {updateProgress ? (
        <p className="updateProgress" role="status">
          {updateProgress}
        </p>
      ) : null}
    </UtilityDialog>
  );
}
