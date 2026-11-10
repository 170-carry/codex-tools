import { Presence } from "../ui/Presence";
import { AddAccountDialog } from "../AddAccountDialog";
import { EditApiAccountDialog } from "../accounts/EditApiAccountDialog";
import { DeleteAccountDialog } from "../DeleteAccountDialog";
import { NoticeBanner } from "../NoticeBanner";
import { QuotaDisplayOnboardingDialog } from "../QuotaDisplayOnboardingDialog";
import { RemoteDeployProgressToast } from "../RemoteDeployProgressToast";
import { UpdateBanner } from "../UpdateBanner";
import { LocalProjectMergeDialog, LocalProjectRecoveryGate } from "../LocalProjectDialogs";
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
    <LocalProjectRecoveryGate
      blocked={c.recoveryRequired}
      confirmationOpen={c.localProjectRecoveryConfirmation !== null}
      onAnswer={c.answerLocalProjectRecovery}
    >
      <LocalProjectMergeDialog request={c.localProjectMergeRequest} onAnswer={c.answerLocalProjectMerge} />
      <Presence present={!!c.apiAccountEditor.account}>{c.apiAccountEditor.account ? (
        <EditApiAccountDialog
          account={c.apiAccountEditor.account}
          saving={c.apiAccountEditor.saving}
          error={c.apiAccountEditor.error}
          onSave={c.apiAccountEditor.save}
          onClose={c.apiAccountEditor.close}
        />
      ) : null}</Presence>
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
      <Presence present={!!c.deleteCandidate}><DeleteAccountDialog
        account={c.deleteCandidate}
        deleting={c.deletingAccountId === c.deleteCandidate?.id}
        onCancel={c.onCancelDelete}
        onConfirm={() => void c.onConfirmDelete()}
      />

      </Presence>
      <NoticeBanner notice={c.notice} />
      <RemoteDeployProgressToast progress={c.remoteDeployProgress} />
      <Presence present={c.updateDialogOpen && !!c.pendingUpdate}><UpdateBanner
        open={c.updateDialogOpen}
        pendingUpdate={c.pendingUpdate}
        updateProgress={c.updateProgress}
        installingUpdate={c.installingUpdate}
        onClose={c.closeUpdateDialog}
        onManualDownload={() => void c.openManualDownloadPage()}
        onSkipVersion={() => void c.skipPendingUpdateVersion()}
        onInstallNow={() => void c.installPendingUpdate()}
      />
      </Presence>
      <Presence present={shouldOpenQuotaOnboarding({platform: quotaOnboardingPlatform, settingsLoaded: c.settingsLoaded, windowsCompleted: c.settings.windowsQuotaOnboardingCompleted, macosCompleted: c.settings.macosQuotaOnboardingCompleted})}><QuotaDisplayOnboardingDialog
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
      </Presence>
    </LocalProjectRecoveryGate>
  );
}
