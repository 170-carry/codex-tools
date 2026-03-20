import { useEffect, useState } from "react";

import { useI18n } from "../i18n/I18nProvider";
import { EditorMultiSelect, type MultiSelectOption } from "./EditorMultiSelect";
import type {
  ApiProxyStatus,
  CloudflaredStatus,
  CloudflaredTunnelMode,
  RemoteAuthMode,
  RemoteProxyStatus,
  RemoteServerConfig,
  StartCloudflaredTunnelInput,
} from "../types/app";

const DEFAULT_PROXY_PORT = "8787";
const DEFAULT_REMOTE_SSH_PORT = "22";
const DEFAULT_REMOTE_LISTEN_PORT = "8787";
const REMOTE_DRAFTS_CACHE_KEY = "codex-tools:proxy-remote-drafts";
const REMOTE_EXPANDED_CACHE_KEY = "codex-tools:proxy-remote-expanded-id";
const REMOTE_SELECTED_CACHE_KEY = "codex-tools:proxy-remote-selected-id";
const REMOTE_HISTORY_CACHE_KEY = "codex-tools:proxy-remote-history";

type RemoteServerDraft = {
  id: string;
  label: string;
  host: string;
  sshPort: string;
  sshUser: string;
  authMode: RemoteAuthMode;
  identityFile: string;
  privateKey: string;
  password: string;
  remoteDir: string;
  listenPort: string;
};

type ApiProxyPanelProps = {
  status: ApiProxyStatus;
  cloudflaredStatus: CloudflaredStatus;
  accountCount: number;
  autoStartEnabled: boolean;
  savedPort: number;
  remoteServers: RemoteServerConfig[];
  remoteStatuses: Record<string, RemoteProxyStatus>;
  remoteLogs: Record<string, string>;
  savingSettings: boolean;
  starting: boolean;
  stopping: boolean;
  refreshingApiKey: boolean;
  refreshingRemoteId: string | null;
  deployingRemoteId: string | null;
  startingRemoteId: string | null;
  stoppingRemoteId: string | null;
  readingRemoteLogsId: string | null;
  installingDependencyName: string | null;
  installingDependencyTargetId: string | null;
  installingCloudflared: boolean;
  startingCloudflared: boolean;
  stoppingCloudflared: boolean;
  onStart: (port: number | null) => Promise<void> | void;
  onStop: () => void;
  onRefreshApiKey: () => void;
  onRefresh: () => void;
  onToggleAutoStart: (enabled: boolean) => void;
  onPersistPort: (port: number) => Promise<void> | void;
  onUpdateRemoteServers: (servers: RemoteServerConfig[]) => void;
  onRefreshRemoteStatus: (server: RemoteServerConfig) => void;
  onDeployRemote: (server: RemoteServerConfig) => void;
  onStartRemote: (server: RemoteServerConfig) => void;
  onStopRemote: (server: RemoteServerConfig) => void;
  onReadRemoteLogs: (server: RemoteServerConfig) => void;
  onPickLocalIdentityFile: () => Promise<string | null>;
  onRefreshCloudflared: () => void;
  onInstallCloudflared: () => void;
  onStartCloudflared: (input: StartCloudflaredTunnelInput) => void;
  onStopCloudflared: () => void;
};

function copyText(value: string | null) {
  if (!value) {
    return;
  }
  void navigator.clipboard?.writeText(value).catch(() => {});
}

function createRemoteServerId() {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `remote-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function createRemoteDraft(): RemoteServerDraft {
  return {
    id: createRemoteServerId(),
    label: "",
    host: "",
    sshPort: DEFAULT_REMOTE_SSH_PORT,
    sshUser: "root",
    authMode: "keyPath",
    identityFile: "",
    privateKey: "",
    password: "",
    remoteDir: "/opt/codex-tools",
    listenPort: DEFAULT_REMOTE_LISTEN_PORT,
  };
}

function configToDraft(server: RemoteServerConfig): RemoteServerDraft {
  return {
    id: server.id,
    label: server.label,
    host: server.host,
    sshPort: String(server.sshPort),
    sshUser: server.sshUser,
    authMode: server.authMode,
    identityFile: server.identityFile ?? "",
    privateKey: server.privateKey ?? "",
    password: server.password ?? "",
    remoteDir: server.remoteDir,
    listenPort: String(server.listenPort),
  };
}

function draftToConfig(draft: RemoteServerDraft): RemoteServerConfig {
  return {
    id: draft.id,
    label: draft.label.trim(),
    host: draft.host.trim(),
    sshPort: Number.parseInt(draft.sshPort, 10) || 0,
    sshUser: draft.sshUser.trim(),
    authMode: draft.authMode,
    identityFile: draft.identityFile.trim() || null,
    privateKey: draft.privateKey.trim() || null,
    password: draft.password.trim() || null,
    remoteDir: draft.remoteDir.trim(),
    listenPort: Number.parseInt(draft.listenPort, 10) || 0,
  };
}

function buildRemoteBaseUrl(draft: RemoteServerDraft) {
  const host = draft.host.trim();
  const port = draft.listenPort.trim();
  if (!host || !port) {
    return "--";
  }
  return `http://${host}:${port}/v1`;
}

function readStorageValue(key: string, scope: "session" | "local" = "session") {
  if (typeof window === "undefined") {
    return null;
  }
  try {
    return (scope === "local" ? window.localStorage : window.sessionStorage).getItem(key);
  } catch {
    return null;
  }
}

function writeStorageValue(
  key: string,
  value: string | null,
  scope: "session" | "local" = "session",
) {
  if (typeof window === "undefined") {
    return;
  }
  try {
    const storage = scope === "local" ? window.localStorage : window.sessionStorage;
    if (value === null) {
      storage.removeItem(key);
    } else {
      storage.setItem(key, value);
    }
  } catch {
    // Ignore storage failures in constrained environments.
  }
}

function readCachedRemoteDrafts(remoteServers: RemoteServerConfig[]) {
  const cached = readStorageValue(REMOTE_DRAFTS_CACHE_KEY);
  if (!cached) {
    return remoteServers.map(configToDraft);
  }

  try {
    const parsed = JSON.parse(cached);
    if (!Array.isArray(parsed)) {
      return remoteServers.map(configToDraft);
    }

    return parsed
      .map((item) => {
        if (!item || typeof item !== "object") {
          return null;
        }

        const raw = item as Partial<Record<keyof RemoteServerDraft, unknown>>;
        const authMode =
          raw.authMode === "keyContent" ||
          raw.authMode === "keyFile" ||
          raw.authMode === "keyPath" ||
          raw.authMode === "password"
            ? raw.authMode
            : "keyPath";

        return {
          id: typeof raw.id === "string" && raw.id ? raw.id : createRemoteServerId(),
          label: typeof raw.label === "string" ? raw.label : "",
          host: typeof raw.host === "string" ? raw.host : "",
          sshPort: typeof raw.sshPort === "string" ? raw.sshPort : DEFAULT_REMOTE_SSH_PORT,
          sshUser: typeof raw.sshUser === "string" ? raw.sshUser : "root",
          authMode,
          identityFile: typeof raw.identityFile === "string" ? raw.identityFile : "",
          privateKey: typeof raw.privateKey === "string" ? raw.privateKey : "",
          password: typeof raw.password === "string" ? raw.password : "",
          remoteDir: typeof raw.remoteDir === "string" ? raw.remoteDir : "/opt/codex-tools",
          listenPort:
            typeof raw.listenPort === "string" ? raw.listenPort : DEFAULT_REMOTE_LISTEN_PORT,
        } satisfies RemoteServerDraft;
      })
      .filter((item): item is RemoteServerDraft => item !== null);
  } catch {
    return remoteServers.map(configToDraft);
  }
}

function readCachedEditingRemoteId(remoteServers: RemoteServerConfig[]) {
  const drafts = readCachedRemoteDrafts(remoteServers);
  const cached = readStorageValue(REMOTE_EXPANDED_CACHE_KEY);
  if (cached && drafts.some((draft) => draft.id === cached)) {
    return cached;
  }
  return null;
}

function readCachedSelectedRemoteId(remoteServers: RemoteServerConfig[]) {
  const drafts = readCachedRemoteDrafts(remoteServers);
  const cached = readStorageValue(REMOTE_SELECTED_CACHE_KEY, "local");
  if (cached && drafts.some((draft) => draft.id === cached)) {
    return cached;
  }
  return drafts[0]?.id ?? null;
}

function readCachedRemoteHistory(remoteServers: RemoteServerConfig[]) {
  const activeIds = new Set(remoteServers.map((server) => server.id));
  const cached = readStorageValue(REMOTE_HISTORY_CACHE_KEY, "local");
  if (!cached) {
    return {} as Record<string, number>;
  }

  try {
    const parsed = JSON.parse(cached);
    if (!parsed || typeof parsed !== "object") {
      return {} as Record<string, number>;
    }

    const next: Record<string, number> = {};
    for (const [id, value] of Object.entries(parsed)) {
      if (activeIds.has(id) && typeof value === "number" && Number.isFinite(value) && value > 0) {
        next[id] = value;
      }
    }
    return next;
  } catch {
    return {} as Record<string, number>;
  }
}

function isRemoteDraftConfigured(draft: RemoteServerDraft) {
  const sshPort = Number.parseInt(draft.sshPort, 10);
  const listenPort = Number.parseInt(draft.listenPort, 10);

  if (
    !draft.label.trim() ||
    !draft.host.trim() ||
    !draft.sshUser.trim() ||
    !draft.remoteDir.trim() ||
    !Number.isInteger(sshPort) ||
    sshPort <= 0 ||
    !Number.isInteger(listenPort) ||
    listenPort <= 0
  ) {
    return false;
  }

  if (draft.authMode === "keyContent") {
    return draft.privateKey.trim() !== "";
  }
  if (draft.authMode === "password") {
    return draft.password.trim() !== "";
  }
  return draft.identityFile.trim() !== "";
}

function formatRemoteHistoryTime(locale: string, timestamp: number) {
  try {
    return new Intl.DateTimeFormat(locale, {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    }).format(timestamp);
  } catch {
    return new Date(timestamp).toLocaleString();
  }
}

const REMOTE_AUTH_OPTIONS: MultiSelectOption<RemoteAuthMode>[] = [
  { id: "keyContent", label: "keyContent" },
  { id: "keyFile", label: "keyFile" },
  { id: "keyPath", label: "keyPath" },
  { id: "password", label: "password" },
];

export function ApiProxyPanel({
  status,
  cloudflaredStatus,
  accountCount,
  autoStartEnabled,
  savedPort,
  remoteServers,
  remoteStatuses,
  remoteLogs,
  savingSettings,
  starting,
  stopping,
  refreshingApiKey,
  refreshingRemoteId,
  deployingRemoteId,
  startingRemoteId,
  stoppingRemoteId,
  readingRemoteLogsId,
  installingDependencyName,
  installingDependencyTargetId,
  installingCloudflared,
  startingCloudflared,
  stoppingCloudflared,
  onStart,
  onStop,
  onRefreshApiKey,
  onRefresh,
  onToggleAutoStart,
  onPersistPort,
  onUpdateRemoteServers,
  onRefreshRemoteStatus,
  onDeployRemote,
  onStartRemote,
  onStopRemote,
  onReadRemoteLogs,
  onPickLocalIdentityFile,
  onRefreshCloudflared,
  onInstallCloudflared,
  onStartCloudflared,
  onStopCloudflared,
}: ApiProxyPanelProps) {
  const { copy, locale } = useI18n();
  const proxyCopy = copy.apiProxy;
  const remoteAuthOptions = REMOTE_AUTH_OPTIONS.map((option) => ({
    ...option,
    label:
      option.id === "keyContent"
        ? proxyCopy.remoteAuthKeyContent
        : option.id === "keyFile"
          ? proxyCopy.remoteAuthKeyFile
          : option.id === "keyPath"
            ? proxyCopy.remoteAuthKeyPath
            : proxyCopy.remoteAuthPassword,
  }));
  const busy = starting || stopping;
  const cloudflaredBusy = installingCloudflared || startingCloudflared || stoppingCloudflared;
  const [portDraft, setPortDraft] = useState<string | null>(null);
  const [publicAccessEnabled, setPublicAccessEnabled] = useState(cloudflaredStatus.running);
  const [tunnelMode, setTunnelMode] = useState<CloudflaredTunnelMode>(
    cloudflaredStatus.tunnelMode ?? "quick",
  );
  const [useHttp2, setUseHttp2] = useState(cloudflaredStatus.useHttp2);
  const [remoteDrafts, setRemoteDrafts] = useState<RemoteServerDraft[]>(() =>
    readCachedRemoteDrafts(remoteServers),
  );
  const [selectedRemoteId, setSelectedRemoteId] = useState<string | null>(() =>
    readCachedSelectedRemoteId(remoteServers),
  );
  const [editingRemoteId, setEditingRemoteId] = useState<string | null>(() =>
    readCachedEditingRemoteId(remoteServers),
  );
  const [diagnosticsRemoteId, setDiagnosticsRemoteId] = useState<string | null>(null);
  const [remoteHistory, setRemoteHistory] = useState<Record<string, number>>(() =>
    readCachedRemoteHistory(remoteServers),
  );
  const [namedInput, setNamedInput] = useState({
    apiToken: "",
    accountId: "",
    zoneId: "",
    hostname: cloudflaredStatus.customHostname ?? "",
  });
  const cloudflaredEnabled = publicAccessEnabled || cloudflaredStatus.running;

  const effectiveRemoteDrafts =
    remoteDrafts.length === 0 && remoteServers.length > 0
      ? remoteServers.map(configToDraft)
      : remoteDrafts;

  useEffect(() => {
    writeStorageValue(REMOTE_DRAFTS_CACHE_KEY, JSON.stringify(effectiveRemoteDrafts));
  }, [effectiveRemoteDrafts]);

  useEffect(() => {
    const resolvedEditingRemoteId =
      editingRemoteId && effectiveRemoteDrafts.some((draft) => draft.id === editingRemoteId)
        ? editingRemoteId
        : null;
    writeStorageValue(REMOTE_EXPANDED_CACHE_KEY, resolvedEditingRemoteId);
  }, [effectiveRemoteDrafts, editingRemoteId]);

  useEffect(() => {
    const resolvedSelectedRemoteId =
      selectedRemoteId && effectiveRemoteDrafts.some((draft) => draft.id === selectedRemoteId)
        ? selectedRemoteId
        : effectiveRemoteDrafts[0]?.id ?? null;
    writeStorageValue(REMOTE_SELECTED_CACHE_KEY, resolvedSelectedRemoteId, "local");
  }, [effectiveRemoteDrafts, selectedRemoteId]);

  useEffect(() => {
    writeStorageValue(REMOTE_HISTORY_CACHE_KEY, JSON.stringify(remoteHistory), "local");
  }, [remoteHistory]);

  const portInput = portDraft ?? String(status.port ?? savedPort ?? DEFAULT_PROXY_PORT);
  const rawPort = portInput.trim();
  const effectivePort = !rawPort
    ? 8787
    : Number.isInteger(Number(rawPort)) && Number(rawPort) >= 1 && Number(rawPort) <= 65535
      ? Number(rawPort)
      : null;
  const hasRemoteServers = effectiveRemoteDrafts.length > 0;
  const resolvedSelectedRemoteId =
    selectedRemoteId && effectiveRemoteDrafts.some((draft) => draft.id === selectedRemoteId)
      ? selectedRemoteId
      : effectiveRemoteDrafts[0]?.id ?? null;
  const selectedRemoteDraft =
    resolvedSelectedRemoteId === null
      ? null
      : effectiveRemoteDrafts.find((draft) => draft.id === resolvedSelectedRemoteId) ?? null;
  const selectedRemoteConfig = selectedRemoteDraft ? draftToConfig(selectedRemoteDraft) : null;
  const selectedRemoteStatus = selectedRemoteDraft ? remoteStatuses[selectedRemoteDraft.id] : null;
  const selectedRemoteLog = selectedRemoteDraft ? remoteLogs[selectedRemoteDraft.id] : undefined;
  const selectedRemoteConfigured = selectedRemoteDraft
    ? isRemoteDraftConfigured(selectedRemoteDraft)
    : false;
  const selectedRemoteIdentity = selectedRemoteDraft
    ? selectedRemoteDraft.label.trim() || selectedRemoteDraft.host.trim() || proxyCopy.remoteTitle
    : proxyCopy.remoteTitle;
  const selectedRefreshing =
    selectedRemoteDraft !== null && refreshingRemoteId === selectedRemoteDraft.id;
  const selectedDeploying =
    selectedRemoteDraft !== null && deployingRemoteId === selectedRemoteDraft.id;
  const selectedStarting =
    selectedRemoteDraft !== null && startingRemoteId === selectedRemoteDraft.id;
  const selectedStopping =
    selectedRemoteDraft !== null && stoppingRemoteId === selectedRemoteDraft.id;
  const selectedReadingLogs =
    selectedRemoteDraft !== null && readingRemoteLogsId === selectedRemoteDraft.id;
  const selectedInstallingDependency =
    selectedRemoteDraft !== null &&
    installingDependencyName === "sshpass" &&
    installingDependencyTargetId === selectedRemoteDraft.id;
  const selectedRemoteBusy =
    selectedRefreshing ||
    selectedDeploying ||
    selectedStarting ||
    selectedStopping ||
    selectedInstallingDependency;
  const selectedRemoteLastChecked =
    selectedRemoteDraft !== null ? remoteHistory[selectedRemoteDraft.id] ?? 0 : 0;
  const selectedRemoteCheckedLabel =
    selectedRemoteLastChecked > 0
      ? formatRemoteHistoryTime(locale, selectedRemoteLastChecked)
      : proxyCopy.remoteNeverChecked;
  const editingSelectedRemote =
    selectedRemoteDraft !== null && editingRemoteId === selectedRemoteDraft.id;
  const diagnosticsOpen =
    selectedRemoteDraft !== null && diagnosticsRemoteId === selectedRemoteDraft.id;
  const selectedRemoteRunningText = selectedRemoteStatus
    ? selectedRemoteStatus.running
      ? proxyCopy.statusRunning
      : proxyCopy.statusStopped
    : proxyCopy.remoteStatusUnknown;
  const selectedRemoteInstalledText = selectedRemoteStatus
    ? selectedRemoteStatus.installed
      ? proxyCopy.remoteInstalledYes
      : proxyCopy.remoteInstalledNo
    : proxyCopy.remoteStatusUnknown;
  const selectedRemoteSystemdText = selectedRemoteStatus
    ? selectedRemoteStatus.serviceInstalled
      ? proxyCopy.remoteInstalledYes
      : proxyCopy.remoteInstalledNo
    : proxyCopy.remoteStatusUnknown;
  const selectedRemoteEnabledText = selectedRemoteStatus
    ? selectedRemoteStatus.enabled
      ? proxyCopy.remoteInstalledYes
      : proxyCopy.remoteInstalledNo
    : proxyCopy.remoteStatusUnknown;
  const remoteOrder = Object.fromEntries(
    effectiveRemoteDrafts.map((draft, index) => [draft.id, index]),
  );
  const orderedRemoteDrafts = [...effectiveRemoteDrafts].sort((left, right) => {
    const historyDelta = (remoteHistory[right.id] ?? 0) - (remoteHistory[left.id] ?? 0);
    if (historyDelta !== 0) {
      return historyDelta;
    }
    return (remoteOrder[left.id] ?? 0) - (remoteOrder[right.id] ?? 0);
  });

  const namedReady =
    namedInput.apiToken.trim() !== "" &&
    namedInput.accountId.trim() !== "" &&
    namedInput.zoneId.trim() !== "" &&
    namedInput.hostname.trim() !== "";

  const canStartCloudflared =
    status.running &&
    status.port !== null &&
    cloudflaredStatus.installed &&
    !cloudflaredBusy &&
    (tunnelMode === "quick" || namedReady);

  const cloudflaredInput: StartCloudflaredTunnelInput | null =
    status.port === null
      ? null
      : {
          apiProxyPort: status.port,
          useHttp2,
          mode: tunnelMode,
          named:
            tunnelMode === "named"
              ? {
                  apiToken: namedInput.apiToken.trim(),
                  accountId: namedInput.accountId.trim(),
                  zoneId: namedInput.zoneId.trim(),
                  hostname: namedInput.hostname.trim(),
                }
              : null,
        };

  const persistRemoteDrafts = (drafts: RemoteServerDraft[]) => {
    onUpdateRemoteServers(drafts.map(draftToConfig));
  };

  const persistPortIfNeeded = async (explicitPort?: number | null) => {
    const nextPort = explicitPort ?? effectivePort;
    if (nextPort === null || nextPort === savedPort) {
      return;
    }
    await onPersistPort(nextPort);
  };

  const handleStart = async () => {
    await persistPortIfNeeded(effectivePort);
    await onStart(effectivePort);
    setPortDraft(null);
  };

  const updateRemoteDraft = (
    id: string,
    key: keyof Omit<RemoteServerDraft, "id">,
    value: string | RemoteAuthMode,
  ) => {
    setRemoteDrafts((current) =>
      (current.length === 0 && remoteServers.length > 0 ? remoteServers.map(configToDraft) : current).map((draft) => {
        if (draft.id !== id) {
          return draft;
        }
        const next = { ...draft, [key]: value } as RemoteServerDraft;
        if (key === "authMode") {
          if (value !== "keyContent") {
            next.privateKey = "";
          }
          if (value !== "password") {
            next.password = "";
          }
          if (value === "keyContent" || value === "password") {
            next.identityFile = "";
          }
        }
        return next;
      }),
    );
  };

  const addRemoteDraft = () => {
    const nextDraft = createRemoteDraft();
    setRemoteDrafts((current) => [
      ...(current.length === 0 && remoteServers.length > 0 ? remoteServers.map(configToDraft) : current),
      nextDraft,
    ]);
    setSelectedRemoteId(nextDraft.id);
    setEditingRemoteId(nextDraft.id);
    setDiagnosticsRemoteId(null);
  };

  const removeRemoteDraft = (id: string) => {
    const next = effectiveRemoteDrafts.filter((draft) => draft.id !== id);
    setRemoteDrafts(next);
    persistRemoteDrafts(next);
    setSelectedRemoteId((current) => (current === id ? next[0]?.id ?? null : current));
    setEditingRemoteId((current) => (current === id ? null : current));
    setDiagnosticsRemoteId((current) => (current === id ? null : current));
    setRemoteHistory((current) => {
      if (!(id in current)) {
        return current;
      }
      const nextHistory = { ...current };
      delete nextHistory[id];
      return nextHistory;
    });
  };

  const selectRemoteDraft = (id: string) => {
    setSelectedRemoteId(id);
    setDiagnosticsRemoteId(null);
    setRemoteHistory((current) => ({ ...current, [id]: Date.now() }));

    const targetDraft = effectiveRemoteDrafts.find((draft) => draft.id === id);
    if (!targetDraft) {
      return;
    }

    if (!isRemoteDraftConfigured(targetDraft)) {
      setEditingRemoteId(id);
      return;
    }

    onRefreshRemoteStatus(draftToConfig(targetDraft));
  };

  const toggleSelectedDiagnostics = () => {
    if (!selectedRemoteDraft) {
      return;
    }

    const nextOpenId = diagnosticsOpen ? null : selectedRemoteDraft.id;
    setDiagnosticsRemoteId(nextOpenId);

    if (
      nextOpenId &&
      selectedRemoteConfigured &&
      selectedRemoteConfig &&
      !remoteLogs[selectedRemoteDraft.id] &&
      !selectedReadingLogs
    ) {
      onReadRemoteLogs(selectedRemoteConfig);
    }
  };

  let remoteGuideTitle = proxyCopy.remoteStatusUnknown;
  let remoteGuideDescription = proxyCopy.remoteDescription;

  if (selectedRemoteDraft && !selectedRemoteConfigured) {
    remoteGuideTitle = proxyCopy.remoteGuideSetupTitle;
    remoteGuideDescription = proxyCopy.remoteGuideSetupDescription;
  } else if (selectedRefreshing) {
    remoteGuideTitle = proxyCopy.remoteRefreshing;
    remoteGuideDescription = proxyCopy.remoteDescription;
  } else if (selectedRemoteStatus?.running) {
    remoteGuideTitle = proxyCopy.remoteGuideReadyTitle;
    remoteGuideDescription = proxyCopy.remoteGuideReadyDescription;
  } else if (selectedRemoteStatus?.installed) {
    remoteGuideTitle = proxyCopy.remoteGuideStartTitle;
    remoteGuideDescription = proxyCopy.remoteGuideStartDescription;
  } else if (selectedRemoteDraft && selectedRemoteConfigured) {
    remoteGuideTitle = proxyCopy.remoteGuideDeployTitle;
    remoteGuideDescription = proxyCopy.remoteGuideDeployDescription;
  }

  return (
    <section className="proxyPage">
      <div className="proxyShell">
        <section className="proxySectionCard proxySectionCardPrimary">
          <div className="proxyHeaderStats">
            <span className="proxyHeaderStat">
              <span className={`proxyStatusDot${status.running ? " isRunning" : ""}`} aria-hidden="true" />
              <span>{proxyCopy.statusLabel}</span>
              <strong>{status.running ? proxyCopy.statusRunning : proxyCopy.statusStopped}</strong>
            </span>
            <span className="proxyHeaderStat">
              <span>{proxyCopy.portLabel}</span>
              <strong>{status.port ?? "--"}</strong>
            </span>
            <span className="proxyHeaderStat">
              <span>{proxyCopy.accountCountLabel}</span>
              <strong>{accountCount}</strong>
            </span>
          </div>

          <div className="proxyControlRow">
            <label className="proxyCompactField">
              <span>{proxyCopy.portLabel}</span>
              <input
                className="proxyPortInput"
                inputMode="numeric"
                aria-label={proxyCopy.portInputAriaLabel}
                placeholder={DEFAULT_PROXY_PORT}
                value={portInput}
                onChange={(event) => setPortDraft(event.target.value)}
                onBlur={() => {
                  void (async () => {
                    await persistPortIfNeeded();
                    if (effectivePort !== null) {
                      setPortDraft(null);
                    }
                  })();
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.currentTarget.blur();
                  }
                }}
                disabled={busy || status.running}
              />
            </label>

            <div className="proxyInlineSetting">
              <span className="proxyInlineLabel">{proxyCopy.defaultStartLabel}</span>
              <label className="themeSwitch" aria-label={proxyCopy.defaultStartLabel}>
                <input
                  type="checkbox"
                  checked={autoStartEnabled}
                  disabled={savingSettings}
                  onChange={(event) => onToggleAutoStart(event.target.checked)}
                />
                <span className="themeSwitchTrack" aria-hidden="true">
                  <span className="themeSwitchThumb" />
                </span>
                <span className="themeSwitchText">
                  {autoStartEnabled
                    ? proxyCopy.defaultStartEnabled
                    : proxyCopy.defaultStartDisabled}
                </span>
              </label>
            </div>

            <div className="proxyControlActions">
              <button className="ghost" onClick={onRefresh} disabled={busy}>
                {proxyCopy.refreshStatus}
              </button>
              {status.running ? (
                <button className="danger" onClick={onStop} disabled={busy}>
                  {stopping ? proxyCopy.stopping : proxyCopy.stop}
                </button>
              ) : (
                <button
                  className="primary"
                  onClick={() => {
                    void handleStart();
                  }}
                  disabled={busy || accountCount === 0 || effectivePort === null}
                >
                  {starting ? proxyCopy.starting : proxyCopy.start}
                </button>
              )}
            </div>
          </div>

          <div className="proxyDetailGrid">
            <article className="proxyDetailCard proxyEndpointCard">
              <span className="proxyLabel">{proxyCopy.baseUrlLabel}</span>
              <div className="proxyEndpointList">
                <div className="proxyEndpointRow">
                  <div className="proxyEndpointMeta">
                    <span>{proxyCopy.localBaseUrlLabel}</span>
                    <code>{status.baseUrl ?? proxyCopy.baseUrlPlaceholder}</code>
                  </div>
                  <button
                    className="ghost proxyCopyButton"
                    onClick={() => copyText(status.baseUrl)}
                    disabled={!status.baseUrl}
                  >
                    {proxyCopy.copy}
                  </button>
                </div>

                {status.lanBaseUrl ? (
                  <div className="proxyEndpointRow">
                    <div className="proxyEndpointMeta">
                      <span>{proxyCopy.lanBaseUrlLabel}</span>
                      <code>{status.lanBaseUrl}</code>
                    </div>
                    <button
                      className="ghost proxyCopyButton"
                      onClick={() => copyText(status.lanBaseUrl)}
                      disabled={!status.lanBaseUrl}
                    >
                      {proxyCopy.copy}
                    </button>
                  </div>
                ) : null}
              </div>
            </article>

            <article className="proxyDetailCard">
              <div className="proxyDetailHeader">
                <span className="proxyLabel">{proxyCopy.apiKeyLabel}</span>
                <div className="proxyDetailActions">
                  <button
                    className="ghost proxyCopyButton"
                    onClick={onRefreshApiKey}
                    disabled={refreshingApiKey}
                  >
                    {refreshingApiKey ? proxyCopy.refreshingKey : proxyCopy.refreshKey}
                  </button>
                  <button
                    className="ghost proxyCopyButton"
                    onClick={() => copyText(status.apiKey)}
                    disabled={!status.apiKey}
                  >
                    {proxyCopy.copy}
                  </button>
                </div>
              </div>
              <code>{status.apiKey ?? proxyCopy.apiKeyPlaceholder}</code>
            </article>

            <article className="proxyDetailCard">
              <span className="proxyLabel">{proxyCopy.activeAccountLabel}</span>
              <strong>{status.activeAccountLabel ?? proxyCopy.activeAccountEmptyTitle}</strong>
              <p>{status.activeAccountId ?? proxyCopy.activeAccountEmptyDescription}</p>
            </article>

            <article className="proxyDetailCard">
              <span className="proxyLabel">{proxyCopy.lastErrorLabel}</span>
              <p className="proxyErrorText">{status.lastError ?? proxyCopy.none}</p>
            </article>
          </div>
        </section>

        <section className="proxySectionCard">
          <div className="proxySectionHeader">
            <div className="remoteSectionHeading">
              <h3>{proxyCopy.remoteTitle}</h3>
              <p>{proxyCopy.remoteDescription}</p>
            </div>
            <button className="primary" onClick={addRemoteDraft}>
              {proxyCopy.remoteAddServer}
            </button>
          </div>

          {hasRemoteServers ? (
            <div className="remoteWorkspace">
              <aside className="remoteHistoryPanel">
                <div className="remoteHistoryHeader">
                  <span className="proxyLabel">{proxyCopy.remoteHistoryTitle}</span>
                  <strong>{orderedRemoteDrafts.length}</strong>
                </div>
                <div className="remoteHistoryList">
                  {orderedRemoteDrafts.map((draft) => {
                    const remoteStatus = remoteStatuses[draft.id];
                    const remoteIdentity =
                      draft.label.trim() || draft.host.trim() || proxyCopy.remoteTitle;
                    const recentCheckedAt = remoteHistory[draft.id] ?? 0;
                    const historyStateText =
                      refreshingRemoteId === draft.id
                        ? proxyCopy.remoteRefreshing
                        : remoteStatus?.running
                          ? proxyCopy.statusRunning
                          : remoteStatus?.installed
                            ? proxyCopy.statusStopped
                            : isRemoteDraftConfigured(draft)
                              ? proxyCopy.remoteInstalledNo
                              : proxyCopy.remoteStatusUnknown;

                    return (
                      <button
                        key={draft.id}
                        type="button"
                        className={`remoteHistoryItem${
                          resolvedSelectedRemoteId === draft.id ? " isSelected" : ""
                        }`}
                        onClick={() => selectRemoteDraft(draft.id)}
                      >
                        <div className="remoteHistoryItemTop">
                          <div className="remoteHistoryIdentity">
                            <strong>{remoteIdentity}</strong>
                            <span>{draft.host.trim() || "--"}</span>
                          </div>
                          <span
                            className={`remoteServerState${
                              remoteStatus?.running ? " isRunning" : ""
                            }`}
                          >
                            <span
                              className={`proxyStatusDot${
                                remoteStatus?.running ? " isRunning" : ""
                              }`}
                              aria-hidden="true"
                            />
                            {historyStateText}
                          </span>
                        </div>

                        <div className="remoteHistoryItemMeta">
                          <span>
                            SSH {(draft.sshUser.trim() || "root")}:{draft.sshPort.trim() || "--"}
                          </span>
                          <span>
                            {proxyCopy.remoteLastCheckedLabel}{" "}
                            {recentCheckedAt > 0
                              ? formatRemoteHistoryTime(locale, recentCheckedAt)
                              : proxyCopy.remoteNeverChecked}
                          </span>
                        </div>
                      </button>
                    );
                  })}
                </div>
              </aside>

              {selectedRemoteDraft ? (
                <div className="remoteWorkbench">
                  <article className="remoteWorkbenchCard">
                    <div className="remoteWorkbenchHeader">
                      <div className="remoteServerSummary">
                        <div className="remoteServerIdentity">
                          <strong>{selectedRemoteIdentity}</strong>
                          <span>
                            {selectedRemoteStatus?.baseUrl ?? buildRemoteBaseUrl(selectedRemoteDraft)}
                          </span>
                        </div>
                        <div className="remoteServerSummaryMeta">
                          <span className="remoteServerSummaryPill">
                            {proxyCopy.remoteHostLabel} {selectedRemoteDraft.host.trim() || "--"}
                          </span>
                          <span className="remoteServerSummaryPill">
                            SSH {(selectedRemoteDraft.sshUser.trim() || "root")}:
                            {selectedRemoteDraft.sshPort.trim() || "--"}
                          </span>
                          <span className="remoteServerSummaryPill">
                            {proxyCopy.remoteListenPortLabel}{" "}
                            {selectedRemoteDraft.listenPort.trim() || "--"}
                          </span>
                          <span className="remoteServerSummaryPill">
                            {proxyCopy.remoteLastCheckedLabel} {selectedRemoteCheckedLabel}
                          </span>
                        </div>
                      </div>

                      <div className="remoteWorkbenchActions">
                        <button
                          className="ghost"
                          onClick={() => {
                            if (!selectedRemoteDraft) {
                              return;
                            }
                            if (!selectedRemoteConfigured) {
                              setEditingRemoteId(selectedRemoteDraft.id);
                              return;
                            }
                            if (selectedRemoteConfig) {
                              setRemoteHistory((current) => ({
                                ...current,
                                [selectedRemoteDraft.id]: Date.now(),
                              }));
                              onRefreshRemoteStatus(selectedRemoteConfig);
                            }
                          }}
                          disabled={selectedRemoteBusy}
                        >
                          {selectedRefreshing ? proxyCopy.remoteRefreshing : proxyCopy.remoteRefresh}
                        </button>
                        <button
                          className="ghost"
                          onClick={() =>
                            setEditingRemoteId((current) =>
                              current === selectedRemoteDraft.id ? null : selectedRemoteDraft.id,
                            )
                          }
                        >
                          {editingSelectedRemote ? proxyCopy.remoteCollapse : proxyCopy.remoteExpand}
                        </button>
                        {selectedRemoteStatus?.installed ? (
                          selectedRemoteStatus.running ? (
                            <button
                              className="danger"
                              onClick={() => {
                                if (selectedRemoteConfig) {
                                  onStopRemote(selectedRemoteConfig);
                                }
                              }}
                              disabled={!selectedRemoteConfigured || selectedRemoteBusy}
                            >
                              {selectedStopping ? proxyCopy.remoteStopping : proxyCopy.remoteStop}
                            </button>
                          ) : (
                            <button
                              className="primary"
                              onClick={() => {
                                if (selectedRemoteConfig) {
                                  onStartRemote(selectedRemoteConfig);
                                }
                              }}
                              disabled={!selectedRemoteConfigured || selectedRemoteBusy}
                            >
                              {selectedStarting ? proxyCopy.remoteStarting : proxyCopy.remoteStart}
                            </button>
                          )
                        ) : (
                          <button
                            className="primary"
                            onClick={() => {
                              if (selectedRemoteConfig) {
                                onDeployRemote(selectedRemoteConfig);
                              }
                            }}
                            disabled={!selectedRemoteConfigured || selectedRemoteBusy}
                          >
                            {selectedDeploying ? proxyCopy.remoteDeploying : proxyCopy.remoteDeploy}
                          </button>
                        )}
                      </div>
                    </div>

                    <div className="remoteServerStatus">
                      <div className="remoteServerMeta">
                        <span>{proxyCopy.remoteInstalledLabel}</span>
                        <strong>{selectedRemoteInstalledText}</strong>
                      </div>
                      <div className="remoteServerMeta">
                        <span>{proxyCopy.remoteSystemdLabel}</span>
                        <strong>{selectedRemoteSystemdText}</strong>
                      </div>
                      <div className="remoteServerMeta">
                        <span>{proxyCopy.remoteEnabledLabel}</span>
                        <strong>{selectedRemoteEnabledText}</strong>
                      </div>
                      <div className="remoteServerMeta">
                        <span>{proxyCopy.remoteRunningLabel}</span>
                        <strong>{selectedRemoteRunningText}</strong>
                      </div>
                      <div className="remoteServerMeta">
                        <span>{proxyCopy.remotePidLabel}</span>
                        <strong>{selectedRemoteStatus?.pid ?? "--"}</strong>
                      </div>
                    </div>
                  </article>

                  <article className="proxyDetailCard remoteGuideCard">
                    <span className="proxyLabel">{proxyCopy.remoteKicker}</span>
                    <strong>{remoteGuideTitle}</strong>
                    <p>{remoteGuideDescription}</p>
                    <div className="remoteGuideActions">
                      {selectedRemoteConfigured ? (
                        selectedRemoteStatus?.running ? (
                          <>
                            <button
                              className="ghost"
                              onClick={() => copyText(selectedRemoteStatus.baseUrl)}
                              disabled={!selectedRemoteStatus.baseUrl}
                            >
                              {proxyCopy.remoteBaseUrlLabel}
                            </button>
                            <button
                              className="ghost"
                              onClick={() => copyText(selectedRemoteStatus.apiKey ?? null)}
                              disabled={!selectedRemoteStatus.apiKey}
                            >
                              {proxyCopy.remoteApiKeyLabel}
                            </button>
                            <button
                              className="ghost"
                              onClick={toggleSelectedDiagnostics}
                              disabled={selectedReadingLogs}
                            >
                              {diagnosticsOpen
                                ? proxyCopy.remoteCollapse
                                : selectedReadingLogs
                                  ? proxyCopy.remoteReadingLogs
                                  : proxyCopy.remoteReadLogs}
                            </button>
                          </>
                        ) : null
                      ) : (
                        <button
                          className="ghost"
                          onClick={() => setEditingRemoteId(selectedRemoteDraft.id)}
                        >
                          {proxyCopy.remoteExpand}
                        </button>
                      )}
                    </div>

                    <div className="proxyDetailGrid remoteProxyDetailGrid">
                      <article className="proxyDetailCard">
                        <div className="proxyDetailHeader">
                          <span className="proxyLabel">{proxyCopy.remoteBaseUrlLabel}</span>
                          <button
                            className="ghost proxyCopyButton"
                            onClick={() =>
                              copyText(
                                selectedRemoteStatus?.baseUrl ??
                                  buildRemoteBaseUrl(selectedRemoteDraft),
                              )
                            }
                          >
                            {proxyCopy.copy}
                          </button>
                        </div>
                        <code>
                          {selectedRemoteStatus?.baseUrl ?? buildRemoteBaseUrl(selectedRemoteDraft)}
                        </code>
                      </article>

                      <article className="proxyDetailCard">
                        <div className="proxyDetailHeader">
                          <span className="proxyLabel">{proxyCopy.remoteApiKeyLabel}</span>
                          <button
                            className="ghost proxyCopyButton"
                            onClick={() => copyText(selectedRemoteStatus?.apiKey ?? null)}
                            disabled={!selectedRemoteStatus?.apiKey}
                          >
                            {proxyCopy.copy}
                          </button>
                        </div>
                        <code>{selectedRemoteStatus?.apiKey ?? proxyCopy.apiKeyPlaceholder}</code>
                      </article>

                      <article className="proxyDetailCard">
                        <span className="proxyLabel">{proxyCopy.remoteServiceLabel}</span>
                        <code>{selectedRemoteStatus?.serviceName ?? proxyCopy.remoteStatusUnknown}</code>
                      </article>
                    </div>
                  </article>

                  {editingSelectedRemote ? (
                    <div className="remoteWorkbenchSection">
                      <div className="remoteWorkbenchSectionHeader">
                        <div>
                          <span className="proxyLabel">{proxyCopy.remoteConfigTitle}</span>
                          <strong>{selectedRemoteIdentity}</strong>
                        </div>
                        <div className="remoteWorkbenchSectionActions">
                          <button
                            className="ghost"
                            onClick={() => {
                              persistRemoteDrafts(effectiveRemoteDrafts);
                              setEditingRemoteId(null);
                            }}
                            disabled={selectedRemoteBusy}
                          >
                            {proxyCopy.remoteSave}
                          </button>
                          <button
                            className="ghost"
                            onClick={() => removeRemoteDraft(selectedRemoteDraft.id)}
                            disabled={selectedRemoteBusy}
                          >
                            {proxyCopy.remoteRemove}
                          </button>
                        </div>
                      </div>

                      <div className="remoteServerPanel">
                        <div className="remoteServerGrid">
                          <label className="remoteServerField">
                            <span>{proxyCopy.remoteNameLabel}</span>
                            <input
                              value={selectedRemoteDraft.label}
                              onChange={(event) =>
                                updateRemoteDraft(
                                  selectedRemoteDraft.id,
                                  "label",
                                  event.target.value,
                                )
                              }
                              placeholder="tokyo-01"
                            />
                          </label>
                          <label className="remoteServerField">
                            <span>{proxyCopy.remoteHostLabel}</span>
                            <input
                              value={selectedRemoteDraft.host}
                              onChange={(event) =>
                                updateRemoteDraft(selectedRemoteDraft.id, "host", event.target.value)
                              }
                              placeholder="1.2.3.4"
                            />
                          </label>
                          <label className="remoteServerField">
                            <span>{proxyCopy.remoteSshPortLabel}</span>
                            <input
                              inputMode="numeric"
                              value={selectedRemoteDraft.sshPort}
                              onChange={(event) =>
                                updateRemoteDraft(
                                  selectedRemoteDraft.id,
                                  "sshPort",
                                  event.target.value,
                                )
                              }
                              placeholder={DEFAULT_REMOTE_SSH_PORT}
                            />
                          </label>
                          <label className="remoteServerField">
                            <span>{proxyCopy.remoteUserLabel}</span>
                            <input
                              value={selectedRemoteDraft.sshUser}
                              onChange={(event) =>
                                updateRemoteDraft(
                                  selectedRemoteDraft.id,
                                  "sshUser",
                                  event.target.value,
                                )
                              }
                              placeholder="root"
                            />
                          </label>
                          <label className="remoteServerField">
                            <span>{proxyCopy.remoteDirLabel}</span>
                            <input
                              value={selectedRemoteDraft.remoteDir}
                              onChange={(event) =>
                                updateRemoteDraft(
                                  selectedRemoteDraft.id,
                                  "remoteDir",
                                  event.target.value,
                                )
                              }
                              placeholder="/opt/codex-tools"
                            />
                          </label>
                          <label className="remoteServerField">
                            <span>{proxyCopy.remoteListenPortLabel}</span>
                            <input
                              inputMode="numeric"
                              value={selectedRemoteDraft.listenPort}
                              onChange={(event) =>
                                updateRemoteDraft(
                                  selectedRemoteDraft.id,
                                  "listenPort",
                                  event.target.value,
                                )
                              }
                              placeholder={DEFAULT_REMOTE_LISTEN_PORT}
                            />
                          </label>
                        </div>
                      </div>

                      <div className="remoteServerPanel">
                        <div className="remoteAuthRow">
                          <label className="remoteServerField remoteAuthSelectField">
                            <span>{proxyCopy.remoteAuthLabel}</span>
                            <EditorMultiSelect
                              className="remoteAuthPicker"
                              ariaLabel={proxyCopy.remoteAuthLabel}
                              options={remoteAuthOptions}
                              value={selectedRemoteDraft.authMode}
                              onChange={(next) =>
                                updateRemoteDraft(selectedRemoteDraft.id, "authMode", next)
                              }
                            />
                          </label>

                          <div className="remoteAuthInputArea">
                            {selectedRemoteDraft.authMode === "keyContent" ? (
                              <label className="remoteServerField">
                                <span>{proxyCopy.remotePrivateKeyLabel}</span>
                                <textarea
                                  className="remoteServerTextarea"
                                  value={selectedRemoteDraft.privateKey}
                                  onChange={(event) =>
                                    updateRemoteDraft(
                                      selectedRemoteDraft.id,
                                      "privateKey",
                                      event.target.value,
                                    )
                                  }
                                  placeholder={proxyCopy.remotePrivateKeyPlaceholder}
                                />
                              </label>
                            ) : null}

                            {selectedRemoteDraft.authMode === "password" ? (
                              <label className="remoteServerField">
                                <span>{proxyCopy.remotePasswordLabel}</span>
                                <input
                                  type="password"
                                  value={selectedRemoteDraft.password}
                                  onChange={(event) =>
                                    updateRemoteDraft(
                                      selectedRemoteDraft.id,
                                      "password",
                                      event.target.value,
                                    )
                                  }
                                  placeholder={proxyCopy.remotePasswordPlaceholder}
                                />
                              </label>
                            ) : null}

                            {selectedRemoteDraft.authMode === "keyFile" ||
                            selectedRemoteDraft.authMode === "keyPath" ? (
                              <div className="remoteIdentityRow">
                                <label className="remoteServerField">
                                  <span>{proxyCopy.remoteIdentityFileLabel}</span>
                                  <input
                                    value={selectedRemoteDraft.identityFile}
                                    onChange={(event) =>
                                      updateRemoteDraft(
                                        selectedRemoteDraft.id,
                                        "identityFile",
                                        event.target.value,
                                      )
                                    }
                                    placeholder={proxyCopy.remoteIdentityFilePlaceholder}
                                  />
                                </label>
                                {selectedRemoteDraft.authMode === "keyFile" ? (
                                  <button
                                    className="ghost"
                                    type="button"
                                    onClick={() => {
                                      void onPickLocalIdentityFile().then((value) => {
                                        if (value) {
                                          updateRemoteDraft(
                                            selectedRemoteDraft.id,
                                            "identityFile",
                                            value,
                                          );
                                        }
                                      });
                                    }}
                                  >
                                    {proxyCopy.remotePickIdentityFile}
                                  </button>
                                ) : null}
                              </div>
                            ) : null}
                          </div>
                        </div>
                      </div>

                      {selectedInstallingDependency ? (
                        <div
                          className="remoteDependencyInstall"
                          role="status"
                          aria-live="polite"
                          aria-busy="true"
                        >
                          <div className="remoteDependencyInstallHeader">
                            <strong>{copy.notices.installingDependency("sshpass")}</strong>
                          </div>
                          <div className="remoteDependencyInstallTrack" aria-hidden="true">
                            <span className="remoteDependencyInstallFill" />
                          </div>
                        </div>
                      ) : null}
                    </div>
                  ) : null}

                  <div className="remoteWorkbenchSection">
                    <div className="remoteWorkbenchSectionHeader">
                      <div>
                        <span className="proxyLabel">{proxyCopy.remoteLogsLabel}</span>
                        <strong>{selectedRemoteIdentity}</strong>
                      </div>
                      <div className="remoteWorkbenchSectionActions">
                        <button
                          className="ghost"
                          onClick={toggleSelectedDiagnostics}
                          disabled={selectedReadingLogs}
                        >
                          {diagnosticsOpen
                            ? proxyCopy.remoteCollapse
                            : selectedReadingLogs
                              ? proxyCopy.remoteReadingLogs
                              : proxyCopy.remoteReadLogs}
                        </button>
                      </div>
                    </div>

                    {diagnosticsOpen ? (
                      <div className="remoteDiagnosticsGrid">
                        <article className="proxyDetailCard remoteLogCard">
                          <div className="proxyDetailHeader">
                            <span className="proxyLabel">{proxyCopy.remoteLogsLabel}</span>
                            <button
                              className="ghost proxyCopyButton"
                              onClick={() => copyText(selectedRemoteLog ?? null)}
                              disabled={!selectedRemoteLog}
                            >
                              {proxyCopy.copy}
                            </button>
                          </div>
                          <code className="remoteLogCode">
                            {selectedRemoteLog ?? proxyCopy.remoteLogsEmpty}
                          </code>
                        </article>

                        <article className="proxyDetailCard remoteErrorCard">
                          <span className="proxyLabel">{proxyCopy.remoteLastErrorLabel}</span>
                          <p className="proxyErrorText">
                            {selectedRemoteStatus?.lastError ?? proxyCopy.none}
                          </p>
                        </article>
                      </div>
                    ) : null}
                  </div>
                </div>
              ) : null}
            </div>
          ) : (
            <article className="cloudflaredCallout">
              <strong>{proxyCopy.remoteEmptyTitle}</strong>
              <p>{proxyCopy.remoteEmptyDescription}</p>
            </article>
          )}
        </section>

        <section className="proxySectionCard">
          <div className="proxySectionHeader">
            <h3>{proxyCopy.cloudflaredTitle}</h3>
            <div className="proxySectionToggle">
              <span className="proxyInlineLabel">{proxyCopy.cloudflaredToggle}</span>
              <label className="themeSwitch" aria-label={proxyCopy.cloudflaredToggle}>
                <input
                  type="checkbox"
                  checked={publicAccessEnabled}
                  onChange={(event) => setPublicAccessEnabled(event.target.checked)}
                />
                <span className="themeSwitchTrack" aria-hidden="true">
                  <span className="themeSwitchThumb" />
                </span>
                <span className="themeSwitchText">
                  {publicAccessEnabled
                    ? proxyCopy.defaultStartEnabled
                    : proxyCopy.defaultStartDisabled}
                </span>
              </label>
            </div>
          </div>

          {cloudflaredEnabled ? (
            <div className="cloudflaredContent">
              {!status.running ? (
                <article className="cloudflaredCallout">
                  <strong>{proxyCopy.startLocalProxyFirstTitle}</strong>
                  <p>{proxyCopy.startLocalProxyFirstDescription}</p>
                </article>
              ) : null}

              {!cloudflaredStatus.installed ? (
                <article className="cloudflaredInstallCard">
                  <div>
                    <span className="proxyLabel">{proxyCopy.notInstalledLabel}</span>
                    <strong>{proxyCopy.installTitle}</strong>
                    <p>{proxyCopy.installDescription}</p>
                  </div>
                  <button
                    className="primary"
                    onClick={onInstallCloudflared}
                    disabled={installingCloudflared}
                  >
                    {installingCloudflared ? proxyCopy.installing : proxyCopy.installButton}
                  </button>
                </article>
              ) : (
                <>
                  <div className="cloudflaredModeGrid">
                    <button
                      className={`cloudflaredModeCard${tunnelMode === "quick" ? " isActive" : ""}`}
                      onClick={() => setTunnelMode("quick")}
                      disabled={cloudflaredBusy || cloudflaredStatus.running}
                    >
                      <span className="proxyLabel">{proxyCopy.quickModeLabel}</span>
                      <strong>{proxyCopy.quickModeTitle}</strong>
                      <p>{proxyCopy.quickModeDescription}</p>
                    </button>
                    <button
                      className={`cloudflaredModeCard${tunnelMode === "named" ? " isActive" : ""}`}
                      onClick={() => setTunnelMode("named")}
                      disabled={cloudflaredBusy || cloudflaredStatus.running}
                    >
                      <span className="proxyLabel">{proxyCopy.namedModeLabel}</span>
                      <strong>{proxyCopy.namedModeTitle}</strong>
                      <p>{proxyCopy.namedModeDescription}</p>
                    </button>
                  </div>

                  {tunnelMode === "quick" ? (
                    <article className="cloudflaredCallout">
                      <strong>{proxyCopy.quickNoteTitle}</strong>
                      <p>{proxyCopy.quickNoteBody}</p>
                    </article>
                  ) : null}

                  {tunnelMode === "named" ? (
                    <div className="cloudflaredFormGrid">
                      <label className="cloudflaredInputField">
                        <span>{proxyCopy.apiTokenLabel}</span>
                        <input
                          type="password"
                          value={namedInput.apiToken}
                          onChange={(event) =>
                            setNamedInput((current) => ({ ...current, apiToken: event.target.value }))
                          }
                          placeholder={proxyCopy.apiTokenPlaceholder}
                          disabled={cloudflaredBusy || cloudflaredStatus.running}
                        />
                      </label>
                      <label className="cloudflaredInputField">
                        <span>{proxyCopy.accountIdLabel}</span>
                        <input
                          value={namedInput.accountId}
                          onChange={(event) =>
                            setNamedInput((current) => ({ ...current, accountId: event.target.value }))
                          }
                          placeholder={proxyCopy.accountIdPlaceholder}
                          disabled={cloudflaredBusy || cloudflaredStatus.running}
                        />
                      </label>
                      <label className="cloudflaredInputField">
                        <span>{proxyCopy.zoneIdLabel}</span>
                        <input
                          value={namedInput.zoneId}
                          onChange={(event) =>
                            setNamedInput((current) => ({ ...current, zoneId: event.target.value }))
                          }
                          placeholder={proxyCopy.zoneIdPlaceholder}
                          disabled={cloudflaredBusy || cloudflaredStatus.running}
                        />
                      </label>
                      <label className="cloudflaredInputField">
                        <span>{proxyCopy.hostnameLabel}</span>
                        <input
                          value={namedInput.hostname}
                          onChange={(event) =>
                            setNamedInput((current) => ({ ...current, hostname: event.target.value }))
                          }
                          placeholder={proxyCopy.hostnamePlaceholder}
                          disabled={cloudflaredBusy || cloudflaredStatus.running}
                        />
                      </label>
                    </div>
                  ) : null}

                  <div className="cloudflaredToolbar">
                    <div className="cloudflaredToolbarMeta">
                      <span className="proxyInlineLabel">{proxyCopy.useHttp2}</span>
                      <label className="themeSwitch" aria-label={proxyCopy.useHttp2}>
                        <input
                          type="checkbox"
                          checked={useHttp2}
                          onChange={(event) => setUseHttp2(event.target.checked)}
                          disabled={cloudflaredBusy || cloudflaredStatus.running}
                        />
                        <span className="themeSwitchTrack" aria-hidden="true">
                          <span className="themeSwitchThumb" />
                        </span>
                        <span className="themeSwitchText">
                          {useHttp2
                            ? proxyCopy.defaultStartEnabled
                            : proxyCopy.defaultStartDisabled}
                        </span>
                      </label>
                    </div>

                    <div className="cloudflaredToolbarActions">
                      <button
                        className="ghost"
                        onClick={onRefreshCloudflared}
                        disabled={cloudflaredBusy}
                      >
                        {proxyCopy.refreshPublicStatus}
                      </button>
                      {cloudflaredStatus.running ? (
                        <button
                          className="danger"
                          onClick={onStopCloudflared}
                          disabled={cloudflaredBusy}
                        >
                          {stoppingCloudflared ? proxyCopy.stoppingPublic : proxyCopy.stopPublic}
                        </button>
                      ) : (
                        <button
                          className="primary"
                          onClick={() => {
                            if (cloudflaredInput) {
                              onStartCloudflared(cloudflaredInput);
                            }
                          }}
                          disabled={!canStartCloudflared || cloudflaredInput === null}
                        >
                          {startingCloudflared ? proxyCopy.startingPublic : proxyCopy.startPublic}
                        </button>
                      )}
                    </div>
                  </div>

                  <div className="proxyDetailGrid">
                    <article className="proxyDetailCard">
                      <span className="proxyLabel">{proxyCopy.publicStatusLabel}</span>
                      <strong className={`proxyStatus${cloudflaredStatus.running ? " isRunning" : ""}`}>
                        {cloudflaredStatus.running
                          ? proxyCopy.publicStatusRunning
                          : proxyCopy.publicStatusStopped}
                      </strong>
                      <p>
                        {cloudflaredStatus.running
                          ? proxyCopy.publicStatusRunningDescription
                          : proxyCopy.publicStatusStoppedDescription}
                      </p>
                    </article>

                    <article className="proxyDetailCard">
                      <div className="proxyDetailHeader">
                        <span className="proxyLabel">{proxyCopy.publicUrlLabel}</span>
                        <button
                          className="ghost proxyCopyButton"
                          onClick={() => copyText(cloudflaredStatus.publicUrl)}
                          disabled={!cloudflaredStatus.publicUrl}
                        >
                          {proxyCopy.copy}
                        </button>
                      </div>
                      <code>{cloudflaredStatus.publicUrl ?? proxyCopy.baseUrlPlaceholder}</code>
                    </article>

                    <article className="proxyDetailCard">
                      <span className="proxyLabel">{proxyCopy.installPathLabel}</span>
                      <code>{cloudflaredStatus.binaryPath ?? proxyCopy.notDetected}</code>
                    </article>

                    <article className="proxyDetailCard">
                      <span className="proxyLabel">{proxyCopy.lastErrorLabel}</span>
                      <p className="proxyErrorText">{cloudflaredStatus.lastError ?? proxyCopy.none}</p>
                    </article>
                  </div>
                </>
              )}
            </div>
          ) : null}
        </section>
      </div>
    </section>
  );
}
