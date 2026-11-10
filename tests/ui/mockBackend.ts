import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { accounts as fixtures, analytics, tokenUsage, now } from "./fixtures";
import {
  DEFAULT_SETTINGS,
  DEFAULT_API_PROXY_STATUS,
  DEFAULT_CLOUDFLARED_STATUS,
} from "./defaults";
import type { AppSettings } from "../../src/types/app";
import type { ApiAccountEdit } from "../../src/hooks/useApiAccountEditor";
import { version } from "../../package.json";

export function installPreviewBackend() {
  const params = new URLSearchParams(location.search);
  let pendingRecovery = params.get("pendingRecovery") === "1";
  let accounts = structuredClone(fixtures);
  const accountCount = Number(params.get("accounts"));
  if (accountCount > 0 && accountCount <= 1000) {
    accounts = Array.from({ length: accountCount }, (_, index) => ({
      ...structuredClone(fixtures[0]),
      id: `synthetic-${index}`,
      accountKey: `synthetic-${index}`,
      label: `synthetic-${String(index).padStart(3, "0")}@example.com`,
      email: `synthetic-${String(index).padStart(3, "0")}@example.com`,
      isCurrent: index === 0,
      addedAt: now + index,
    }));
  }
  if (params.get("scenario") === "empty") accounts = [];
  if (params.get("scenario") === "no-active")
    accounts.forEach((a) => {
      a.isCurrent = false;
    });
  if (params.get("scenario") === "unknown")
    accounts.forEach((a) => {
      a.usage = null;
    });
  let settings: AppSettings = {
    ...DEFAULT_SETTINGS,
    codexAnalyticsWeeklyBudgetUsd: 50,
    preserveLocalProjects: params.get("preserve") !== "0",
  };
  const commands: string[] = [];
  const calls: { command: string; payload: unknown }[] = [];
  const proxyStatus = params.get("proxy") === "bound"
    ? {
        ...DEFAULT_API_PROXY_STATUS,
        running: true,
        port: 8787,
        baseUrl: "http://127.0.0.1:8787/v1",
        apiKey: "sk-preview-only",
        codexProxyBound: true,
        codexProxyRestoreAvailable: true,
        codexProxyBaseUrl: "http://127.0.0.1:8787/v1",
      }
    : DEFAULT_API_PROXY_STATUS;
  Object.assign(window, { __previewCommands: commands, __previewCalls: calls });
  mockWindows("main");
  mockIPC(
    async (command, payload) => {
      commands.push(command);
      calls.push({ command, payload: structuredClone(payload) });
      if (command === "recover_local_project_switch") {
        await new Promise((resolve) => setTimeout(resolve, 200));
        if (params.get("recoveryError") === "externalChange") throw new Error("PROJECT_SHARING_EXTERNAL_CHANGE /synthetic/private-state");
        const recovered = pendingRecovery;
        pendingRecovery = false;
        return recovered;
      }
      if (pendingRecovery && !command.startsWith("plugin:")) throw new Error("PROJECT_SHARING_RECOVERY_REQUIRED /synthetic/private-state");
      if (command === "preview_local_project_switch") {
        if (params.get("previewError")) throw new Error(`PROJECT_SHARING_${params.get("previewError")} /synthetic/private-state`);
        if (accounts.find((a) => a.id === payload?.id)?.sourceKind === "relay") throw new Error("PROJECT_SHARING_UNSUPPORTED_ROUTE");
        return {
          projectCount: 24, threadLinkCount: 81, unassignedThreadCount: 4,
          retainedTargetProjectCount: params.get("emptyTarget") === "1" ? 0 : 3,
          requiresMergeConfirmation: params.get("emptyTarget") !== "1",
          previewFingerprint: "synthetic-project-preview-v1", noOp: false,
        };
      }
      if (command === "list_accounts" || command === "refresh_all_usage")
        return structuredClone(accounts);
      if (command === "use_reset_credit") {
        const account = accounts.find(a => a.id === payload?.id);
        const credits = account?.usage?.resetCredits;
        if (!account || !credits) throw new Error("Preview credit unavailable");
        const card = credits.credits.find(c => c.id === payload?.creditId && c.status === "available");
        if (!card) throw new Error("Preview card unavailable");
        card.status = "redeemed";
        credits.availableCount = Math.max(0, (credits.availableCount ?? 0) - 1);
        return { code: "reset", windowsReset: 2, accounts: structuredClone(accounts), refreshError: null };
      }
      if (command === "get_app_settings") return settings;
      if (command === "update_app_settings") {
        settings = { ...settings, ...(payload?.patch as Partial<AppSettings>) };
        return settings;
      }
      if (command === "get_codex_token_usage") return tokenUsage;
      if (
        command === "get_cached_codex_cost_analytics" ||
        command === "refresh_codex_cost_analytics"
      )
        return analytics;
      if (command === "get_api_proxy_status" || command === "bind_codex_to_api_proxy") return proxyStatus;
      if (command === "get_cloudflared_status")
        return DEFAULT_CLOUDFLARED_STATUS;
      if (command === "get_api_proxy_supported_models")
        return ["gpt-6-astra", "gpt-6.1-sol", "gpt-6-sol", "gpt-6-luna", "gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna", "gpt-5.5", "gpt-5.4", "gpt-image-2"];
      if (
        [
          "list_api_proxy_keys",
          "get_api_proxy_key_usage_logs",
          "list_installed_editor_apps",
        ].includes(command)
      )
        return [];
      if (command === "get_api_proxy_usage_stats")
        return {
          updatedAt: now,
          rangeSeconds: 86400,
          bucketSeconds: 3600,
          series: [],
          keySeries: [],
        };
      if (command === "is_opencode_desktop_app_installed")
        return params.get("opencode") === "installed";
      if (command === "plugin:window|is_minimized" || command === "get_windows_widgets_enabled")
        return false;
      if (command === "plugin:app|version") return version;
      if (command === "plugin:app|name") return "Codex Tools Preview";
      if (command === "get_runtime_platform") return params.get("platform") ?? "macos";
      if (command === "get_tray_visual_previews") return [];
      if (command === "is_debug_build") return false;
      if (command === "plugin:updater|check") return null;
      if (command === "update_account_label") {
        accounts.forEach((a) => {
          if (a.accountKey === payload?.accountKey)
            a.label = String(payload?.label);
        });
        return payload?.label;
      }
      if (command === "update_api_account") {
        const input = payload?.input as ApiAccountEdit;
        accounts.forEach((a) => {
          if (a.id === payload?.id && a.sourceKind === "relay") {
            a.label = input.label;
            a.apiBaseUrl = input.baseUrl;
            a.modelName = input.modelName;
          }
        });
        return null;
      }
      if (command === "update_account_api_proxy_enabled") {
        if (params.get("proxy-toggle") === "delayed")
          await new Promise((resolve) => setTimeout(resolve, 2500));
        if (params.get("proxy-toggle") === "failed")
          throw new Error("Preview proxy participation save failed");
        accounts.forEach((a) => {
          if (a.accountKey === payload?.accountKey)
            a.apiProxyEnabled = Boolean(payload?.enabled);
        });
        return payload?.enabled;
      }
      if (command === "switch_account_and_launch") {
        if (settings.preserveLocalProjects) {
          if (params.get("switchError")) throw new Error(`PROJECT_SHARING_${params.get("switchError")} /synthetic/private-state`);
          if (payload?.localProjectPreviewFingerprint !== "synthetic-project-preview-v1") throw new Error("PROJECT_SHARING_PREVIEW_CHANGED");
          if (params.get("emptyTarget") !== "1" && payload?.acceptLocalProjectMerge !== true) throw new Error("PROJECT_SHARING_CONFIRMATION_REQUIRED");
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
        accounts.forEach((a) => {
          a.isCurrent = a.id === payload?.id;
        });
        const preservationStatus = params.get("preservationStatus") ?? (payload?.launchCodex ? "verified" : "prepared");
        return {
          localProjectPreservation: settings.preserveLocalProjects ? {
            status: preservationStatus,
            projectCount: 24, threadLinkCount: 81, unassignedThreadCount: 4,
            warning: preservationStatus === "pending" ? "PROJECT_SHARING_RESTORE_PENDING /synthetic/private-state" : null,
          } : null,
          accountId: payload?.id,
          launchedAppPath: null,
          usedFallbackCli: false,
          opencodeSynced: false,
          opencodeSyncError: null,
          opencodeDesktopRestarted: false,
          opencodeDesktopRestartError: null,
          restartedEditorApps: [],
          editorRestartError: null,
          providerSyncError: null,
        };
      }
      if (command === "warmup_account")
        return { id: payload?.id, status: "alreadyActive", accounts };
      if (command === "delete_account") {
        accounts = accounts.filter((a) => a.id !== payload?.id);
        return null;
      }
      if (command === "export_accounts_zip") return "/preview/accounts.zip";
      if (command === "prepare_oauth_login")
        return {
          authUrl: "https://example.com/preview-oauth",
          redirectUri: "http://localhost:1455/auth/callback",
        };
      if (command === "test_api_account_connection")
        return { ok: true, balanceText: "$128.40", message: "Preview" };
      if (command.startsWith("plugin:")) return null;
      throw new Error(`Preview has no handler for ${command}`);
    },
    { shouldMockEvents: true },
  );
}
