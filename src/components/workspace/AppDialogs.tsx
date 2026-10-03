import { AddAccountDialog } from "../AddAccountDialog";
import { EditApiAccountDialog } from "../accounts/EditApiAccountDialog";
import { DeleteAccountDialog } from "../DeleteAccountDialog";
import { NoticeBanner } from "../NoticeBanner";
import { QuotaDisplayOnboardingDialog } from "../QuotaDisplayOnboardingDialog";
import { RemoteDeployProgressToast } from "../RemoteDeployProgressToast";
import { UpdateBanner } from "../UpdateBanner";
import {
  shouldOpenQuotaOnboarding,
  type QuotaOnboardingPlatform,
} from "../../utils/quotaDisplayOnboarding";
import type { ThemeMode } from "../../types/app";
import type { CodexController } from "../../types/workspace";

export function AppDialogs({
  c,
  themeMode,
  quotaOnboardingPlatform,
}: {
  c: CodexController;
  themeMode: ThemeMode;
  quotaOnboardingPlatform: QuotaOnboardingPlatform;
}) {
  return (
    <>
      {c.apiAccountEditor.account ? (
        <EditApiAccountDialog
          account={c.apiAccountEditor.account}
          saving={c.apiAccountEditor.saving}
          error={c.apiAccountEditor.error}
          onSave={c.apiAccountEditor.save}
          onClose={c.apiAccountEditor.close}
        />
      ) : null}
      <AddAccountDialog
        open={c.addDialogOpen}
        reauthorizeAccount={c.reauthorizeAccount}
        importingAccounts={c.importingAccounts}
        oauthWaitingForCallback={c.oauthWaitingForCallback}
        onPrepareOauth={c.onPrepareOauthLogin}
        onOpenOauthPage={c.onOpenOauthAuthorizationPage}
        onCompleteOauth={c.onCompleteOauthCallbackLogin}
        onCancelOauth={c.onCancelOauthLogin}
        onImportCurrentAuth={c.onImportCurrentAuth}
        onCreateApiAccount={c.onCreateApiAccount}
        onTestApiConnection={c.onTestApiAccountConnection}
        onImportFiles={c.onImportAuthFiles}
        onClose={c.onCloseAddDialog}
      />
      <DeleteAccountDialog
        account={c.deleteCandidate}
        deleting={c.deletingAccountId === c.deleteCandidate?.id}
        onCancel={c.onCancelDelete}
        onConfirm={() => void c.onConfirmDelete()}
      />

      <NoticeBanner notice={c.notice} />
      <RemoteDeployProgressToast progress={c.remoteDeployProgress} />
      <UpdateBanner
        open={c.updateDialogOpen}
        pendingUpdate={c.pendingUpdate}
        updateProgress={c.updateProgress}
        installingUpdate={c.installingUpdate}
        onClose={c.closeUpdateDialog}
        onManualDownload={() => void c.openManualDownloadPage()}
        onSkipVersion={() => void c.skipPendingUpdateVersion()}
        onInstallNow={() => void c.installPendingUpdate()}
      />
      <QuotaDisplayOnboardingDialog
        open={shouldOpenQuotaOnboarding({
          platform: quotaOnboardingPlatform,
          settingsLoaded: c.settingsLoaded,
          windowsCompleted: c.settings.windowsQuotaOnboardingCompleted,
          macosCompleted: c.settings.macosQuotaOnboardingCompleted,
        })}
        platform={quotaOnboardingPlatform === "macos" ? "macos" : "windows"}
        lightTheme={themeMode !== "dark"}
        settings={c.settings}
        saving={c.savingSettings}
        onPreviewSettings={(patch) =>
          c.updateSettings(patch, {
            silent: true,
            throwOnError: true,
            keepInteractive: true,
          })
        }
        onConfirm={(patch) =>
          c.updateSettings(patch, { silent: true, throwOnError: true })
        }
      />
    </>
  );
}
