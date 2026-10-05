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
  let accounts = structuredClone(fixtures);
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
  };
  const commands: string[] = [];
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
  Object.assign(window, { __previewCommands: commands });
  mockWindows("main");
  mockIPC(
    async (command, payload) => {
      commands.push(command);
      if (command === "list_accounts" || command === "refresh_all_usage")
        return structuredClone(accounts);
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
        await new Promise((resolve) => setTimeout(resolve, 200));
        accounts.forEach((a) => {
          a.isCurrent = a.id === payload?.id;
        });
        return {
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
