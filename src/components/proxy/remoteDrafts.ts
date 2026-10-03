import type { RemoteServerConfig } from "../../types/app";
import {
  DEFAULT_REMOTE_SSH_PORT,
  DEFAULT_REMOTE_LISTEN_PORT,
  REMOTE_DRAFTS_CACHE_KEY,
  REMOTE_EXPANDED_CACHE_KEY,
  REMOTE_SELECTED_CACHE_KEY,
  REMOTE_HISTORY_CACHE_KEY,
} from "./constants";
import type { RemoteServerDraft } from "./types";
export function createRemoteServerId() {
  if (
    typeof crypto !== "undefined" &&
    typeof crypto.randomUUID === "function"
  ) {
    return crypto.randomUUID();
  }
  return `remote-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export function createRemoteDraft(): RemoteServerDraft {
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

export function configToDraft(server: RemoteServerConfig): RemoteServerDraft {
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

export function draftToConfig(draft: RemoteServerDraft): RemoteServerConfig {
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

export function buildRemoteBaseUrl(draft: RemoteServerDraft) {
  const host = draft.host.trim();
  const port = draft.listenPort.trim();
  if (!host || !port) {
    return "--";
  }
  return `http://${host}:${port}/v1`;
}

export function readStorageValue(
  key: string,
  scope: "session" | "local" = "session",
) {
  if (typeof window === "undefined") {
    return null;
  }
  try {
    return (
      scope === "local" ? window.localStorage : window.sessionStorage
    ).getItem(key);
  } catch {
    return null;
  }
}

export function writeStorageValue(
  key: string,
  value: string | null,
  scope: "session" | "local" = "session",
) {
  if (typeof window === "undefined") {
    return;
  }
  try {
    const storage =
      scope === "local" ? window.localStorage : window.sessionStorage;
    if (value === null) {
      storage.removeItem(key);
    } else {
      storage.setItem(key, value);
    }
  } catch {
    // Ignore storage failures in constrained environments.
  }
}

export function readCachedRemoteDrafts(remoteServers: RemoteServerConfig[]) {
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
          id:
            typeof raw.id === "string" && raw.id
              ? raw.id
              : createRemoteServerId(),
          label: typeof raw.label === "string" ? raw.label : "",
          host: typeof raw.host === "string" ? raw.host : "",
          sshPort:
            typeof raw.sshPort === "string"
              ? raw.sshPort
              : DEFAULT_REMOTE_SSH_PORT,
          sshUser: typeof raw.sshUser === "string" ? raw.sshUser : "root",
          authMode,
          identityFile:
            typeof raw.identityFile === "string" ? raw.identityFile : "",
          privateKey: typeof raw.privateKey === "string" ? raw.privateKey : "",
          password: typeof raw.password === "string" ? raw.password : "",
          remoteDir:
            typeof raw.remoteDir === "string"
              ? raw.remoteDir
              : "/opt/codex-tools",
          listenPort:
            typeof raw.listenPort === "string"
              ? raw.listenPort
              : DEFAULT_REMOTE_LISTEN_PORT,
        } satisfies RemoteServerDraft;
      })
      .filter((item): item is RemoteServerDraft => item !== null);
  } catch {
    return remoteServers.map(configToDraft);
  }
}

export function readCachedEditingRemoteId(remoteServers: RemoteServerConfig[]) {
  const drafts = readCachedRemoteDrafts(remoteServers);
  const cached = readStorageValue(REMOTE_EXPANDED_CACHE_KEY);
  if (cached && drafts.some((draft) => draft.id === cached)) {
    return cached;
  }
  return null;
}

export function readCachedSelectedRemoteId(
  remoteServers: RemoteServerConfig[],
) {
  const drafts = readCachedRemoteDrafts(remoteServers);
  const cached = readStorageValue(REMOTE_SELECTED_CACHE_KEY, "local");
  if (cached && drafts.some((draft) => draft.id === cached)) {
    return cached;
  }
  return drafts[0]?.id ?? null;
}

export function readCachedRemoteHistory(remoteServers: RemoteServerConfig[]) {
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
      if (
        activeIds.has(id) &&
        typeof value === "number" &&
        Number.isFinite(value) &&
        value > 0
      ) {
        next[id] = value;
      }
    }
    return next;
  } catch {
    return {} as Record<string, number>;
  }
}

export function isRemoteDraftConfigured(draft: RemoteServerDraft) {
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

export function formatRemoteHistoryTime(locale: string, timestamp: number) {
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
