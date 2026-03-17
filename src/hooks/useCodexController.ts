import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { useI18n } from "../i18n/I18nProvider";
import { localizeBackendError } from "../i18n/backendErrors";
import { DEFAULT_LOCALE } from "../i18n/catalog";
import type { MessageCatalog } from "../i18n/catalog";
import type {
  AccountSummary,
  ApiProxyStatus,
  AppSettings,
  AddFlow,
  AuthJsonImportInput,
  CloudflaredStatus,
  CurrentAuthStatus,
  ImportAccountsResult,
  InstalledEditorApp,
  Notice,
  PendingUpdateInfo,
  RemoteDeployProgress,
  RemoteProxyStatus,
  RemoteServerConfig,
  StartCloudflaredTunnelInput,
  SwitchAccountResult,
  UpdateSettingsOptions,
} from "../types/app";
import { pickBestRemainingAccount, sortAccountsByRemaining } from "../utils/accountRanking";

const REFRESH_MS = 30_000;
const EDITOR_SCAN_MS = 60_000;
const ADD_FLOW_TIMEOUT_MS = 10 * 60_000;
const ADD_FLOW_POLL_MS = 2_500;
const API_PROXY_POLL_MS = 4_000;
const CLOUDFLARED_POLL_MS = 3_000;
const MANUAL_DOWNLOAD_URL = "https://github.com/170-carry/codex-tools/releases/latest";
const DEFAULT_SETTINGS: AppSettings = {
  launchAtStartup: false,
  trayUsageDisplayMode: "remaining",
  launchCodexAfterSwitch: true,
  syncOpencodeOpenaiAuth: false,
  restartOpencodeDesktopOnSwitch: false,
  restartEditorsOnSwitch: false,
  restartEditorTargets: [],
  autoStartApiProxy: false,
  apiProxyPort: 8787,
  remoteServers: [],
  locale: DEFAULT_LOCALE,
};
const DEFAULT_API_PROXY_STATUS: ApiProxyStatus = {
  running: false,
  port: null,
  apiKey: null,
  baseUrl: null,
  lanBaseUrl: null,
  activeAccountId: null,
  activeAccountLabel: null,
  lastError: null,
};
const DEFAULT_CLOUDFLARED_STATUS: CloudflaredStatus = {
  installed: false,
  binaryPath: null,
  running: false,
  tunnelMode: null,
  publicUrl: null,
  customHostname: null,
  useHttp2: false,
  lastError: null,
};

function buildImportNotice(
  result: ImportAccountsResult,
  prefix: string,
  notices: MessageCatalog["notices"],
  locale: string,
): Notice {
  const successCount = result.importedCount + result.updatedCount;
  const failureCount = result.failures.length;
  const firstFailure = result.failures[0];

  if (successCount === 0) {
    if (firstFailure) {
      return {
        type: "error",
        message: notices.importFailedWithSource(prefix, firstFailure.source, firstFailure.error),
      };
    }
    return {
      type: "error",
      message: notices.importFailedNoValidJson(prefix),
    };
  }

  const segments: string[] = [];
  if (result.importedCount > 0) {
    segments.push(notices.importSummaryAdded(result.importedCount));
  }
  if (result.updatedCount > 0) {
    segments.push(notices.importSummaryUpdated(result.updatedCount));
  }
  if (failureCount > 0) {
    segments.push(notices.importSummaryFailed(failureCount));
  }

  const suffix =
    failureCount > 0 && firstFailure
      ? notices.importSummaryFirstFailure(firstFailure.source, firstFailure.error)
      : "";
  const listFormatter = new Intl.ListFormat(locale, {
    style: "short",
    type: "conjunction",
  });

  return {
    type: failureCount > 0 ? "info" : "ok",
    message: notices.importSummaryDone(prefix, listFormatter.format(segments), suffix),
  };
}

function buildRemoteProxyFallback(
  server: RemoteServerConfig,
  lastError: string,
): RemoteProxyStatus {
  return {
    installed: false,
    serviceInstalled: false,
    running: false,
    enabled: false,
    serviceName: `codex-tools-proxyd-${server.id}.service`,
    pid: null,
    baseUrl: `http://${server.host}:${server.listenPort}/v1`,
    apiKey: null,
    lastError,
  };
}

export function useCodexController() {
  const { copy, locale } = useI18n();
  const [accounts, setAccounts] = useState<AccountSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [startingAdd, setStartingAdd] = useState(false);
  const [addFlow, setAddFlow] = useState<AddFlow | null>(null);
  const [importingUpload, setImportingUpload] = useState(false);
  const [exportingAccounts, setExportingAccounts] = useState(false);
  const [apiProxyStatus, setApiProxyStatus] = useState<ApiProxyStatus>(DEFAULT_API_PROXY_STATUS);
  const [cloudflaredStatus, setCloudflaredStatus] = useState<CloudflaredStatus>(DEFAULT_CLOUDFLARED_STATUS);
  const [remoteProxyStatusesRaw, setRemoteProxyStatusesRaw] = useState<Record<string, RemoteProxyStatus>>({});
  const [remoteProxyLogs, setRemoteProxyLogs] = useState<Record<string, string>>({});
  const [remoteDeployProgress, setRemoteDeployProgress] = useState<RemoteDeployProgress | null>(null);
  const [startingApiProxy, setStartingApiProxy] = useState(false);
  const [stoppingApiProxy, setStoppingApiProxy] = useState(false);
  const [refreshingApiProxyKey, setRefreshingApiProxyKey] = useState(false);
  const [refreshingRemoteProxyId, setRefreshingRemoteProxyId] = useState<string | null>(null);
  const [deployingRemoteProxyId, setDeployingRemoteProxyId] = useState<string | null>(null);
  const [startingRemoteProxyId, setStartingRemoteProxyId] = useState<string | null>(null);
  const [stoppingRemoteProxyId, setStoppingRemoteProxyId] = useState<string | null>(null);
  const [readingRemoteLogsId, setReadingRemoteLogsId] = useState<string | null>(null);
  const [installingDependencyName, setInstallingDependencyName] = useState<string | null>(null);
  const [installingDependencyTargetId, setInstallingDependencyTargetId] = useState<string | null>(null);
  const [installingCloudflared, setInstallingCloudflared] = useState(false);
  const [startingCloudflared, setStartingCloudflared] = useState(false);
  const [stoppingCloudflared, setStoppingCloudflared] = useState(false);
  const [switchingId, setSwitchingId] = useState<string | null>(null);
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [installingUpdate, setInstallingUpdate] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<string | null>(null);
  const [pendingUpdate, setPendingUpdate] = useState<PendingUpdateInfo | null>(null);
  const [updateDialogOpen, setUpdateDialogOpen] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [savingSettings, setSavingSettings] = useState(false);
  const [installedEditorApps, setInstalledEditorApps] = useState<InstalledEditorApp[]>([]);
  const [hasOpencodeDesktopApp, setHasOpencodeDesktopApp] = useState(false);
  const installingUpdateRef = useRef(false);
  const addFlowCancelledRef = useRef(false);
  const deleteConfirmTimerRef = useRef<number | null>(null);
  const settingsUpdateQueueRef = useRef<Promise<void>>(Promise.resolve());

  const sortedAccounts = useMemo(() => sortAccountsByRemaining(accounts), [accounts]);

  const localizeError = useCallback(
    (error: string) => localizeBackendError(error, locale),
    [locale],
  );

  const localizeAccounts = useCallback(
    (items: AccountSummary[]) =>
      items.map((account) => ({
        ...account,
        usageError: account.usageError ? localizeError(account.usageError) : null,
      })),
    [localizeError],
  );

  const localizeApiProxyStatus = useCallback(
    (status: ApiProxyStatus): ApiProxyStatus => ({
      ...status,
      lastError: status.lastError ? localizeError(status.lastError) : null,
    }),
    [localizeError],
  );

  const localizeCloudflaredStatus = useCallback(
    (status: CloudflaredStatus): CloudflaredStatus => ({
      ...status,
      lastError: status.lastError ? localizeError(status.lastError) : null,
    }),
    [localizeError],
  );

  const localizeRemoteProxyStatus = useCallback(
    (status: RemoteProxyStatus): RemoteProxyStatus => ({
      ...status,
      lastError: status.lastError ? localizeError(status.lastError) : null,
    }),
    [localizeError],
  );

  const localizeImportResult = useCallback(
    (result: ImportAccountsResult): ImportAccountsResult => ({
      ...result,
      failures: result.failures.map((failure) => ({
        ...failure,
        error: localizeError(failure.error),
      })),
    }),
    [localizeError],
  );

  const remoteProxyStatuses = useMemo<Record<string, RemoteProxyStatus>>(
    () =>
      Object.fromEntries(
        Object.entries(remoteProxyStatusesRaw).map(([id, status]) => [
          id,
          localizeRemoteProxyStatus(status),
        ]),
      ),
    [localizeRemoteProxyStatus, remoteProxyStatusesRaw],
  );

  const loadAccounts = useCallback(async () => {
    const data = await invoke<AccountSummary[]>("list_accounts");
    setAccounts(localizeAccounts(data));
  }, [localizeAccounts]);

  const loadSettings = useCallback(async () => {
    const data = await invoke<AppSettings>("get_app_settings");
    setSettings(data);
  }, []);

  const loadInstalledEditorApps = useCallback(async () => {
    try {
      const data = await invoke<InstalledEditorApp[]>("list_installed_editor_apps");
      setInstalledEditorApps(data);
    } catch {
      setInstalledEditorApps([]);
    }
  }, []);

  const loadOpencodeDesktopAppInstalled = useCallback(async () => {
    try {
      const installed = await invoke<boolean>("is_opencode_desktop_app_installed");
      setHasOpencodeDesktopApp(installed);
    } catch {
      setHasOpencodeDesktopApp(false);
    }
  }, []);

  const loadApiProxyStatus = useCallback(async () => {
    try {
      const data = await invoke<ApiProxyStatus>("get_api_proxy_status");
      setApiProxyStatus(localizeApiProxyStatus(data));
    } catch {
      setApiProxyStatus(DEFAULT_API_PROXY_STATUS);
    }
  }, [localizeApiProxyStatus]);

  const loadCloudflaredStatus = useCallback(async () => {
    try {
      const data = await invoke<CloudflaredStatus>("get_cloudflared_status");
      setCloudflaredStatus(localizeCloudflaredStatus(data));
    } catch {
      setCloudflaredStatus(DEFAULT_CLOUDFLARED_STATUS);
    }
  }, [localizeCloudflaredStatus]);

  const updateSettings = useCallback(
    async (patch: Partial<AppSettings>, options?: UpdateSettingsOptions) => {
      const shouldLockUi = !options?.keepInteractive;
      const task = async () => {
        if (shouldLockUi) {
          setSavingSettings(true);
        }

        try {
          const data = await invoke<AppSettings>("update_app_settings", { patch });
          setSettings(data);
          if (!options?.silent) {
            setNotice({ type: "ok", message: copy.notices.settingsUpdated });
          }
        } catch (error) {
          setNotice({
            type: "error",
            message: copy.notices.updateSettingsFailed(localizeError(String(error))),
          });
        } finally {
          if (shouldLockUi) {
            setSavingSettings(false);
          }
        }
      };

      const run = settingsUpdateQueueRef.current.then(task, task);
      settingsUpdateQueueRef.current = run.then(
        () => undefined,
        () => undefined,
      );
      return run;
    },
    [copy.notices, localizeError],
  );

  const refreshUsage = useCallback(async (quiet = false) => {
    try {
      if (!quiet) {
        setRefreshing(true);
      }
      const data = await invoke<AccountSummary[]>("refresh_all_usage", {
        forceAuthRefresh: !quiet,
      });
      setAccounts(localizeAccounts(data));
      if (!quiet) {
        setNotice({ type: "ok", message: copy.notices.usageRefreshed });
      }
    } catch (error) {
      if (!quiet) {
        setNotice({
          type: "error",
          message: copy.notices.refreshFailed(localizeError(String(error))),
        });
      }
    } finally {
      if (!quiet) {
        setRefreshing(false);
      }
    }
  }, [copy.notices, localizeAccounts, localizeError]);

  const restoreAuthAfterAddFlow = useCallback(async () => {
    try {
      await invoke<boolean>("restore_auth_after_add_flow");
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.restoreAuthFailed(localizeError(String(error))),
      });
    }
  }, [copy.notices, localizeError]);

  const applyImportResult = useCallback(
    async (result: ImportAccountsResult, prefix: string) => {
      const successCount = result.importedCount + result.updatedCount;
      if (successCount > 0) {
        await loadAccounts();
      }

      if (successCount > 0 && result.failures.length === 0) {
        setAddDialogOpen(false);
      }

      setNotice(buildImportNotice(result, prefix, copy.notices, locale));
    },
    [copy.notices, loadAccounts, locale],
  );

  useEffect(() => {
    installingUpdateRef.current = installingUpdate;
  }, [installingUpdate]);

  useEffect(() => {
    if (!notice) {
      return;
    }
    const ttl = notice.type === "error" ? 6_000 : 3_500;
    const timer = window.setTimeout(() => {
      setNotice((current) => (current === notice ? null : current));
    }, ttl);
    return () => {
      window.clearTimeout(timer);
    };
  }, [notice]);

  useEffect(
    () => () => {
      if (deleteConfirmTimerRef.current !== null) {
        window.clearTimeout(deleteConfirmTimerRef.current);
        deleteConfirmTimerRef.current = null;
      }
    },
    [],
  );

  const installPendingUpdate = useCallback(
    async (knownUpdate?: NonNullable<Awaited<ReturnType<typeof check>>>) => {
      if (installingUpdateRef.current) {
        return;
      }

      setInstallingUpdate(true);
      setUpdateProgress(copy.notices.preparingUpdateDownload);
      try {
        const update = knownUpdate ?? (await check());
        if (!update) {
          setPendingUpdate(null);
          setUpdateDialogOpen(false);
          setNotice({ type: "ok", message: copy.notices.alreadyLatest });
          return;
        }

        let totalBytes = 0;
        let downloadedBytes = 0;
        await update.downloadAndInstall((event) => {
          if (event.event === "Started") {
            totalBytes = event.data.contentLength ?? 0;
            downloadedBytes = 0;
            setUpdateProgress(copy.notices.updateDownloadStarted);
          } else if (event.event === "Progress") {
            downloadedBytes += event.data.chunkLength;
            if (totalBytes > 0) {
              const percentValue = Math.min(
                100,
                Math.round((downloadedBytes / totalBytes) * 100),
              );
              setUpdateProgress(copy.notices.updateDownloadingPercent(percentValue));
            } else {
              setUpdateProgress(copy.notices.updateDownloading);
            }
          } else if (event.event === "Finished") {
            setUpdateProgress(copy.notices.updateDownloadFinished);
          }
        });

        setUpdateProgress(copy.notices.updateInstalling);
        await relaunch();
      } catch (error) {
        setNotice({
          type: "error",
          message: copy.notices.updateInstallFailed(localizeError(String(error))),
        });
        setUpdateProgress(null);
      } finally {
        setInstallingUpdate(false);
      }
    },
    [copy.notices, localizeError],
  );

  const checkForAppUpdate = useCallback(
    async (quiet = false) => {
      if (!quiet) {
        setCheckingUpdate(true);
      }
      try {
        const update = await check();
        if (update) {
          setPendingUpdate({
            currentVersion: update.currentVersion,
            version: update.version,
            body: update.body,
            date: update.date,
          });
          setUpdateDialogOpen(true);
          if (!quiet) {
            setNotice({
              type: "info",
              message: copy.notices.foundNewVersion(update.version, update.currentVersion),
            });
          }
          void installPendingUpdate(update);
        } else {
          setPendingUpdate(null);
          setUpdateDialogOpen(false);
          if (!quiet) {
            setNotice({ type: "ok", message: copy.notices.alreadyLatest });
          }
        }
      } catch (error) {
        if (!quiet) {
          setNotice({
            type: "error",
            message: copy.notices.updateCheckFailed(localizeError(String(error))),
          });
        }
      } finally {
        if (!quiet) {
          setCheckingUpdate(false);
        }
      }
    },
    [copy.notices, installPendingUpdate, localizeError],
  );

  const openManualDownloadPage = useCallback(async () => {
    try {
      await invoke("open_external_url", { url: MANUAL_DOWNLOAD_URL });
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.openManualDownloadFailed(localizeError(String(error))),
      });
    }
  }, [copy.notices, localizeError]);

  const closeUpdateDialog = useCallback(() => {
    setUpdateDialogOpen(false);
  }, []);

  useEffect(() => {
    let cancelled = false;

    const bootstrap = async () => {
      try {
        await loadInstalledEditorApps();
        await loadOpencodeDesktopAppInstalled();
        await loadSettings();
        await loadAccounts();
        await loadApiProxyStatus();
        await loadCloudflaredStatus();
        await refreshUsage(true);
        await checkForAppUpdate(true);
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    void bootstrap();

    const usageTimer = setInterval(() => {
      void refreshUsage(true);
    }, REFRESH_MS);

    const editorTimer = setInterval(() => {
      void loadInstalledEditorApps();
      void loadOpencodeDesktopAppInstalled();
    }, EDITOR_SCAN_MS);

    return () => {
      cancelled = true;
      clearInterval(usageTimer);
      clearInterval(editorTimer);
    };
  }, [
    checkForAppUpdate,
    loadAccounts,
    loadApiProxyStatus,
    loadCloudflaredStatus,
    loadInstalledEditorApps,
    loadOpencodeDesktopAppInstalled,
    loadSettings,
    refreshUsage,
  ]);

  useEffect(() => {
    if (loading) {
      return;
    }

    void loadAccounts();
    void loadApiProxyStatus();
    void loadCloudflaredStatus();
  }, [loadAccounts, loadApiProxyStatus, loadCloudflaredStatus, loading, locale]);

  useEffect(() => {
    setRemoteProxyStatusesRaw((current) => {
      const activeIds = new Set(settings.remoteServers.map((server) => server.id));
      let changed = false;
      const next: Record<string, RemoteProxyStatus> = {};

      for (const [id, status] of Object.entries(current)) {
        if (activeIds.has(id)) {
          next[id] = status;
        } else {
          changed = true;
        }
      }

      return changed ? next : current;
    });
    setRemoteProxyLogs((current) => {
      const activeIds = new Set(settings.remoteServers.map((server) => server.id));
      let changed = false;
      const next: Record<string, string> = {};

      for (const [id, logText] of Object.entries(current)) {
        if (activeIds.has(id)) {
          next[id] = logText;
        } else {
          changed = true;
        }
      }

      return changed ? next : current;
    });
  }, [settings.remoteServers]);

  useEffect(() => {
    if (loading || settings.remoteServers.length === 0) {
      return;
    }

    let cancelled = false;

    void Promise.all(
      settings.remoteServers.map(async (server) => {
        try {
          const status = await invoke<RemoteProxyStatus>("get_remote_proxy_status", { server });
          return [server.id, status] as const;
        } catch (error) {
          return [server.id, buildRemoteProxyFallback(server, String(error))] as const;
        }
      }),
    ).then((entries) => {
      if (cancelled) {
        return;
      }
      setRemoteProxyStatusesRaw((current) => ({
        ...current,
        ...Object.fromEntries(entries),
      }));
    });

    return () => {
      cancelled = true;
    };
  }, [loading, settings.remoteServers]);

  useEffect(() => {
    if (!apiProxyStatus.running) {
      return;
    }

    const timer = setInterval(() => {
      void loadApiProxyStatus();
    }, API_PROXY_POLL_MS);

    return () => {
      clearInterval(timer);
    };
  }, [apiProxyStatus.running, loadApiProxyStatus]);

  useEffect(() => {
    if (!cloudflaredStatus.running) {
      return;
    }

    const timer = setInterval(() => {
      void loadCloudflaredStatus();
    }, CLOUDFLARED_POLL_MS);

    return () => {
      clearInterval(timer);
    };
  }, [cloudflaredStatus.running, loadCloudflaredStatus]);

  useEffect(() => {
    let disposed = false;
    let unlisten: UnlistenFn | null = null;

    void listen<RemoteDeployProgress>("remote-deploy-progress", (event) => {
      if (!disposed) {
        setRemoteDeployProgress(event.payload);
      }
    })
      .then((fn) => {
        if (disposed) {
          void fn();
          return;
        }
        unlisten = fn;
      })
      .catch(() => {});

    return () => {
      disposed = true;
      if (unlisten) {
        void unlisten();
      }
    };
  }, []);

  useEffect(() => {
    if (!addFlow) {
      return;
    }

    let cancelled = false;
    let inFlight = false;

    const poll = async () => {
      if (cancelled || inFlight) {
        return;
      }
      inFlight = true;

      try {
        const current = await invoke<CurrentAuthStatus>("get_current_auth_status");
        if (!current.available || !current.fingerprint) {
          return;
        }

        if (current.fingerprint === addFlow.baselineFingerprint) {
          return;
        }

        await invoke<AccountSummary>("import_current_auth_account", { label: null });
        await restoreAuthAfterAddFlow();
        await refreshUsage(true);
        await loadAccounts();

        if (!cancelled) {
          setAddFlow(null);
          setAddDialogOpen(false);
          setNotice({ type: "ok", message: copy.notices.addAccountSuccess });
        }
      } catch (error) {
        await restoreAuthAfterAddFlow();
        if (!cancelled) {
          setAddFlow(null);
          setNotice({
            type: "error",
            message: copy.notices.addAccountAutoImportFailed(localizeError(String(error))),
          });
        }
      } finally {
        inFlight = false;
      }
    };

    void poll();

    const timer = setInterval(() => {
      void poll();
    }, ADD_FLOW_POLL_MS);

    const timeoutTimer = setTimeout(() => {
      if (!cancelled) {
        setAddFlow(null);
        void restoreAuthAfterAddFlow();
        setNotice({ type: "error", message: copy.notices.addAccountTimeout });
      }
    }, ADD_FLOW_TIMEOUT_MS);

    return () => {
      cancelled = true;
      clearInterval(timer);
      clearTimeout(timeoutTimer);
    };
  }, [
    addFlow,
    copy.notices,
    loadAccounts,
    localizeError,
    refreshUsage,
    restoreAuthAfterAddFlow,
  ]);

  const onStartAddAccount = useCallback(async () => {
    if (addFlow || startingAdd || importingUpload) {
      return;
    }

    addFlowCancelledRef.current = false;
    setAddDialogOpen(true);
    setStartingAdd(true);
    try {
      const baseline = await invoke<CurrentAuthStatus>("get_current_auth_status");
      if (addFlowCancelledRef.current) {
        return;
      }
      await invoke<void>("launch_codex_login");
      if (addFlowCancelledRef.current) {
        await restoreAuthAfterAddFlow();
        return;
      }
      setAddFlow({
        baselineFingerprint: baseline.fingerprint,
      });
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.startLoginFlowFailed(localizeError(String(error))),
      });
    } finally {
      setStartingAdd(false);
    }
  }, [
    addFlow,
    copy.notices,
    importingUpload,
    localizeError,
    restoreAuthAfterAddFlow,
    startingAdd,
  ]);

  const onOpenAddDialog = useCallback(() => {
    setAddDialogOpen(true);
  }, []);

  const onCloseAddDialog = useCallback(() => {
    if (importingUpload) {
      return;
    }

    addFlowCancelledRef.current = true;
    if (addFlow) {
      setAddFlow(null);
      void restoreAuthAfterAddFlow();
    }
    setAddDialogOpen(false);
  }, [addFlow, importingUpload, restoreAuthAfterAddFlow]);

  const onImportAuthFiles = useCallback(
    async (items: AuthJsonImportInput[]) => {
      if (items.length === 0) {
        setNotice({ type: "error", message: copy.notices.importFilesRequired });
        return;
      }

      setImportingUpload(true);
      try {
        const result = await invoke<ImportAccountsResult>("import_auth_json_accounts", {
          items,
        });
        await applyImportResult(localizeImportResult(result), copy.notices.fileImportPrefix);
      } catch (error) {
        setNotice({
          type: "error",
          message: copy.notices.importFailedPlain(
            copy.notices.fileImportPrefix,
            localizeError(String(error)),
          ),
        });
      } finally {
        setImportingUpload(false);
      }
    },
    [applyImportResult, copy.notices, localizeError, localizeImportResult],
  );

  const onExportAccounts = useCallback(async () => {
    if (exportingAccounts) {
      return;
    }

    setExportingAccounts(true);
    try {
      const exportedPath = await invoke<string | null>("export_accounts_zip");
      if (exportedPath) {
        setNotice({ type: "ok", message: copy.notices.accountsExported });
      }
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.accountsExportFailed(localizeError(String(error))),
      });
    } finally {
      setExportingAccounts(false);
    }
  }, [copy.notices, exportingAccounts, localizeError]);

  const onStartApiProxy = useCallback(async (port?: number | null) => {
    if (startingApiProxy || apiProxyStatus.running) {
      return;
    }

    setStartingApiProxy(true);
    try {
      const status = await invoke<ApiProxyStatus>("start_api_proxy", {
        port: port ?? null,
      });
      setApiProxyStatus(localizeApiProxyStatus(status));
      const target = status.port ? `127.0.0.1:${status.port}` : copy.notices.proxyLocalTargetFallback;
      setNotice({ type: "ok", message: copy.notices.proxyStarted(target) });
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.proxyStartFailed(localizeError(String(error))),
      });
    } finally {
      setStartingApiProxy(false);
    }
  }, [
    apiProxyStatus.running,
    copy.notices,
    localizeApiProxyStatus,
    localizeError,
    startingApiProxy,
  ]);

  const onStopApiProxy = useCallback(async () => {
    if (stoppingApiProxy || !apiProxyStatus.running) {
      return;
    }

    setStoppingApiProxy(true);
    try {
      const status = await invoke<ApiProxyStatus>("stop_api_proxy");
      setApiProxyStatus(localizeApiProxyStatus(status));
      setNotice({ type: "ok", message: copy.notices.proxyStopped });
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.proxyStopFailed(localizeError(String(error))),
      });
    } finally {
      setStoppingApiProxy(false);
    }
  }, [
    apiProxyStatus.running,
    copy.notices,
    localizeApiProxyStatus,
    localizeError,
    stoppingApiProxy,
  ]);

  const onRefreshApiProxyKey = useCallback(async () => {
    if (refreshingApiProxyKey) {
      return;
    }

    setRefreshingApiProxyKey(true);
    try {
      const status = await invoke<ApiProxyStatus>("refresh_api_proxy_key");
      setApiProxyStatus(localizeApiProxyStatus(status));
      setNotice({ type: "ok", message: copy.notices.proxyKeyRefreshed });
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.proxyKeyRefreshFailed(localizeError(String(error))),
      });
    } finally {
      setRefreshingApiProxyKey(false);
    }
  }, [copy.notices, localizeApiProxyStatus, localizeError, refreshingApiProxyKey]);

  const ensureRemoteLocalDependency = useCallback(
    async (server: RemoteServerConfig) => {
      if (server.authMode !== "password") {
        return true;
      }
      if (installingDependencyName) {
        return false;
      }

      try {
        const available = await invoke<boolean>("is_sshpass_available");
        if (available) {
          return true;
        }

        setInstallingDependencyName("sshpass");
        setInstallingDependencyTargetId(server.id);
        setNotice({
          type: "info",
          message: copy.notices.installingDependency("sshpass"),
        });
        await invoke("install_sshpass");
        setNotice({
          type: "ok",
          message: copy.notices.dependencyInstalled("sshpass"),
        });
        return true;
      } catch (error) {
        setNotice({
          type: "error",
          message: copy.notices.dependencyInstallFailed("sshpass", localizeError(String(error))),
        });
        return false;
      } finally {
        setInstallingDependencyName(null);
        setInstallingDependencyTargetId(null);
      }
    },
    [copy.notices, installingDependencyName, localizeError],
  );

  const onRefreshRemoteProxyStatus = useCallback(async (server: RemoteServerConfig) => {
    if (refreshingRemoteProxyId === server.id) {
      return;
    }

    if (!(await ensureRemoteLocalDependency(server))) {
      return;
    }

    setRefreshingRemoteProxyId(server.id);
    try {
      const status = await invoke<RemoteProxyStatus>("get_remote_proxy_status", { server });
      setRemoteProxyStatusesRaw((current) => ({
        ...current,
        [server.id]: status,
      }));
    } catch (error) {
      setRemoteProxyStatusesRaw((current) => ({
        ...current,
        [server.id]: buildRemoteProxyFallback(server, String(error)),
      }));
      setNotice({
        type: "error",
        message: copy.notices.remoteStatusFailed(server.label, localizeError(String(error))),
      });
    } finally {
      setRefreshingRemoteProxyId(null);
    }
  }, [copy.notices, ensureRemoteLocalDependency, localizeError, refreshingRemoteProxyId]);

  const onDeployRemoteProxy = useCallback(async (server: RemoteServerConfig) => {
    if (deployingRemoteProxyId === server.id) {
      return;
    }

    setRemoteDeployProgress({
      serverId: server.id,
      label: server.label,
      stage: "validating",
      progress: 6,
      detail: null,
    });

    if (!(await ensureRemoteLocalDependency(server))) {
      setRemoteDeployProgress((current) =>
        current?.serverId === server.id ? null : current,
      );
      return;
    }

    setDeployingRemoteProxyId(server.id);
    try {
      const status = await invoke<RemoteProxyStatus>("deploy_remote_proxy", {
        input: {
          server,
        },
      });
      setRemoteProxyStatusesRaw((current) => ({
        ...current,
        [server.id]: status,
      }));
      setNotice({ type: "ok", message: copy.notices.remoteProxyDeployed(server.label) });
    } catch (error) {
      setRemoteProxyStatusesRaw((current) => ({
        ...current,
        [server.id]: buildRemoteProxyFallback(server, String(error)),
      }));
      setNotice({
        type: "error",
        message: copy.notices.remoteProxyDeployFailed(server.label, localizeError(String(error))),
      });
    } finally {
      setRemoteDeployProgress((current) =>
        current?.serverId === server.id ? null : current,
      );
      setDeployingRemoteProxyId(null);
    }
  }, [copy.notices, deployingRemoteProxyId, ensureRemoteLocalDependency, localizeError]);

  const onStartRemoteProxy = useCallback(async (server: RemoteServerConfig) => {
    if (startingRemoteProxyId === server.id) {
      return;
    }

    if (!(await ensureRemoteLocalDependency(server))) {
      return;
    }

    setStartingRemoteProxyId(server.id);
    try {
      const status = await invoke<RemoteProxyStatus>("start_remote_proxy", { server });
      setRemoteProxyStatusesRaw((current) => ({
        ...current,
        [server.id]: status,
      }));
      setNotice({ type: "ok", message: copy.notices.remoteProxyStarted(server.label) });
    } catch (error) {
      setRemoteProxyStatusesRaw((current) => ({
        ...current,
        [server.id]: buildRemoteProxyFallback(server, String(error)),
      }));
      setNotice({
        type: "error",
        message: copy.notices.remoteProxyStartFailed(server.label, localizeError(String(error))),
      });
    } finally {
      setStartingRemoteProxyId(null);
    }
  }, [copy.notices, ensureRemoteLocalDependency, localizeError, startingRemoteProxyId]);

  const onStopRemoteProxy = useCallback(async (server: RemoteServerConfig) => {
    if (stoppingRemoteProxyId === server.id) {
      return;
    }

    if (!(await ensureRemoteLocalDependency(server))) {
      return;
    }

    setStoppingRemoteProxyId(server.id);
    try {
      const status = await invoke<RemoteProxyStatus>("stop_remote_proxy", { server });
      setRemoteProxyStatusesRaw((current) => ({
        ...current,
        [server.id]: status,
      }));
      setNotice({ type: "ok", message: copy.notices.remoteProxyStopped(server.label) });
    } catch (error) {
      setRemoteProxyStatusesRaw((current) => ({
        ...current,
        [server.id]: buildRemoteProxyFallback(server, String(error)),
      }));
      setNotice({
        type: "error",
        message: copy.notices.remoteProxyStopFailed(server.label, localizeError(String(error))),
      });
    } finally {
      setStoppingRemoteProxyId(null);
    }
  }, [copy.notices, ensureRemoteLocalDependency, localizeError, stoppingRemoteProxyId]);

  const onReadRemoteProxyLogs = useCallback(async (server: RemoteServerConfig) => {
    if (readingRemoteLogsId === server.id) {
      return;
    }

    if (!(await ensureRemoteLocalDependency(server))) {
      return;
    }

    setReadingRemoteLogsId(server.id);
    try {
      const output = await invoke<string>("read_remote_proxy_logs", {
        server,
        lines: 120,
      });
      setRemoteProxyLogs((current) => ({
        ...current,
        [server.id]: output.trim(),
      }));
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.remoteLogsFailed(server.label, localizeError(String(error))),
      });
    } finally {
      setReadingRemoteLogsId(null);
    }
  }, [copy.notices, ensureRemoteLocalDependency, localizeError, readingRemoteLogsId]);

  const onPickLocalIdentityFile = useCallback(async () => {
    try {
      return await invoke<string | null>("pick_local_identity_file");
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.pickIdentityFileFailed(localizeError(String(error))),
      });
      return null;
    }
  }, [copy.notices, localizeError]);

  const onInstallCloudflared = useCallback(async () => {
    if (installingCloudflared) {
      return;
    }

    setInstallingCloudflared(true);
    try {
      const status = await invoke<CloudflaredStatus>("install_cloudflared");
      setCloudflaredStatus(localizeCloudflaredStatus(status));
      setNotice({ type: "ok", message: copy.notices.cloudflaredInstalled });
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.cloudflaredInstallFailed(localizeError(String(error))),
      });
    } finally {
      setInstallingCloudflared(false);
    }
  }, [copy.notices, installingCloudflared, localizeCloudflaredStatus, localizeError]);

  const onStartCloudflared = useCallback(async (input: StartCloudflaredTunnelInput) => {
    if (startingCloudflared || cloudflaredStatus.running) {
      return;
    }

    setStartingCloudflared(true);
    try {
      const status = await invoke<CloudflaredStatus>("start_cloudflared_tunnel", { input });
      setCloudflaredStatus(localizeCloudflaredStatus(status));
      const target = status.publicUrl ?? copy.notices.cloudflaredPublicUrlFallback;
      setNotice({ type: "ok", message: copy.notices.cloudflaredStarted(target) });
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.cloudflaredStartFailed(localizeError(String(error))),
      });
    } finally {
      setStartingCloudflared(false);
    }
  }, [
    cloudflaredStatus.running,
    copy.notices,
    localizeCloudflaredStatus,
    localizeError,
    startingCloudflared,
  ]);

  const onStopCloudflared = useCallback(async () => {
    if (stoppingCloudflared || !cloudflaredStatus.running) {
      return;
    }

    setStoppingCloudflared(true);
    try {
      const status = await invoke<CloudflaredStatus>("stop_cloudflared_tunnel");
      setCloudflaredStatus(localizeCloudflaredStatus(status));
      setNotice({ type: "ok", message: copy.notices.cloudflaredStopped });
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.cloudflaredStopFailed(localizeError(String(error))),
      });
    } finally {
      setStoppingCloudflared(false);
    }
  }, [
    cloudflaredStatus.running,
    copy.notices,
    localizeCloudflaredStatus,
    localizeError,
    stoppingCloudflared,
  ]);

  const onDelete = useCallback(async (account: AccountSummary) => {
    if (pendingDeleteId !== account.id) {
      setPendingDeleteId(account.id);
      if (deleteConfirmTimerRef.current !== null) {
        window.clearTimeout(deleteConfirmTimerRef.current);
      }
      deleteConfirmTimerRef.current = window.setTimeout(() => {
        setPendingDeleteId((current) => (current === account.id ? null : current));
        deleteConfirmTimerRef.current = null;
      }, 5_000);
      setNotice({ type: "info", message: copy.notices.deleteConfirm(account.label) });
      return;
    }

    if (deleteConfirmTimerRef.current !== null) {
      window.clearTimeout(deleteConfirmTimerRef.current);
      deleteConfirmTimerRef.current = null;
    }
    setPendingDeleteId(null);

    try {
      await invoke<void>("delete_account", { id: account.id });
      setAccounts((prev) => prev.filter((item) => item.id !== account.id));
      setNotice({ type: "ok", message: copy.notices.accountDeleted });
    } catch (error) {
      setNotice({
        type: "error",
        message: copy.notices.deleteFailed(localizeError(String(error))),
      });
    }
  }, [copy.notices, localizeError, pendingDeleteId]);

  const onSwitch = useCallback(
    async (account: AccountSummary) => {
      setSwitchingId(account.id);
      try {
        const result = await invoke<SwitchAccountResult>("switch_account_and_launch", {
          id: account.id,
          workspacePath: null,
          launchCodex: settings.launchCodexAfterSwitch,
          restartEditorsOnSwitch: settings.restartEditorsOnSwitch,
          restartEditorTargets: settings.restartEditorTargets,
        });
        await loadAccounts();

        let baseNotice: Notice;
        if (!settings.launchCodexAfterSwitch) {
          baseNotice = { type: "ok", message: copy.notices.switchedOnly };
        } else if (result.usedFallbackCli) {
          baseNotice = {
            type: "info",
            message: copy.notices.switchedAndLaunchByCli,
          };
        } else {
          baseNotice = { type: "ok", message: copy.notices.switchedAndLaunching };
        }

        if (settings.syncOpencodeOpenaiAuth) {
          if (result.opencodeSyncError) {
            baseNotice = {
              type: "error",
              message: copy.notices.opencodeSyncFailed(
                baseNotice.message,
                localizeError(result.opencodeSyncError),
              ),
            };
          } else if (result.opencodeSynced) {
            baseNotice = {
              ...baseNotice,
              message: copy.notices.opencodeSynced(baseNotice.message),
            };
          }

          if (settings.restartOpencodeDesktopOnSwitch) {
            if (result.opencodeDesktopRestartError) {
              baseNotice = {
                type: "error",
                message: copy.notices.opencodeDesktopRestartFailed(
                  baseNotice.message,
                  localizeError(result.opencodeDesktopRestartError),
                ),
              };
            } else if (result.opencodeDesktopRestarted) {
              baseNotice = {
                ...baseNotice,
                message: copy.notices.opencodeDesktopRestarted(baseNotice.message),
              };
            }
          }
        }

        if (settings.restartEditorsOnSwitch) {
          if (result.editorRestartError) {
            baseNotice = {
              type: "error",
              message: copy.notices.editorRestartFailed(
                baseNotice.message,
                localizeError(result.editorRestartError),
              ),
            };
          } else if (result.restartedEditorApps.length > 0) {
            const restartedLabels = result.restartedEditorApps
              .map((id) => copy.editorAppLabels[id] ?? id)
              .join(" / ");
            baseNotice = {
              ...baseNotice,
              message: copy.notices.editorsRestarted(baseNotice.message, restartedLabels),
            };
          } else {
            baseNotice = {
              ...baseNotice,
              message: copy.notices.noEditorRestarted(baseNotice.message),
            };
          }
        }

        setNotice(baseNotice);
      } catch (error) {
        setNotice({
          type: "error",
          message: copy.notices.switchFailed(localizeError(String(error))),
        });
      } finally {
        setSwitchingId(null);
      }
    },
    [
      copy.editorAppLabels,
      copy.notices,
      loadAccounts,
      localizeError,
      settings.launchCodexAfterSwitch,
      settings.syncOpencodeOpenaiAuth,
      settings.restartOpencodeDesktopOnSwitch,
      settings.restartEditorsOnSwitch,
      settings.restartEditorTargets,
    ],
  );

  const onSmartSwitch = useCallback(async () => {
    if (switchingId) {
      return;
    }

    const target = pickBestRemainingAccount(sortedAccounts);
    if (!target) {
      setNotice({ type: "info", message: copy.notices.smartSwitchNoTarget });
      return;
    }
    if (target.isCurrent) {
      setNotice({
        type: "info",
        message: copy.notices.smartSwitchAlreadyBest,
      });
      return;
    }

    await onSwitch(target);
  }, [copy.notices, onSwitch, sortedAccounts, switchingId]);

  const onUpdateRemoteServers = useCallback(
    async (remoteServers: RemoteServerConfig[]) => {
      await updateSettings(
        { remoteServers },
        { silent: true, keepInteractive: true },
      );
    },
    [updateSettings],
  );

  return {
    accounts: sortedAccounts,
    loading,
    refreshing,
    addDialogOpen,
    startingAdd,
    addFlow,
    importingUpload,
    exportingAccounts,
    apiProxyStatus,
    cloudflaredStatus,
    remoteProxyStatuses,
    remoteProxyLogs,
    remoteDeployProgress,
    startingApiProxy,
    stoppingApiProxy,
    refreshingApiProxyKey,
    refreshingRemoteProxyId,
    deployingRemoteProxyId,
    startingRemoteProxyId,
    stoppingRemoteProxyId,
    readingRemoteLogsId,
    installingDependencyName,
    installingDependencyTargetId,
    installingCloudflared,
    startingCloudflared,
    stoppingCloudflared,
    switchingId,
    pendingDeleteId,
    checkingUpdate,
    installingUpdate,
    updateProgress,
    pendingUpdate,
    updateDialogOpen,
    notice,
    settings,
    savingSettings,
    installedEditorApps,
    hasOpencodeDesktopApp,
    refreshUsage,
    checkForAppUpdate,
    installPendingUpdate,
    openManualDownloadPage,
    closeUpdateDialog,
    updateSettings,
    onOpenAddDialog,
    onStartAddAccount,
    onCloseAddDialog,
    onImportAuthFiles,
    onExportAccounts,
    loadApiProxyStatus,
    onStartApiProxy,
    onStopApiProxy,
    onRefreshApiProxyKey,
    onRefreshRemoteProxyStatus,
    onDeployRemoteProxy,
    onStartRemoteProxy,
    onStopRemoteProxy,
    onReadRemoteProxyLogs,
    onPickLocalIdentityFile,
    loadCloudflaredStatus,
    onInstallCloudflared,
    onStartCloudflared,
    onStopCloudflared,
    onDelete,
    onSwitch,
    onSmartSwitch,
    onUpdateRemoteServers,
    smartSwitching: switchingId !== null,
  };
}
