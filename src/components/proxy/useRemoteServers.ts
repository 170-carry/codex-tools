import { useEffect, useState } from "react";
import { useI18n } from "../../i18n/I18nProvider";
import type { RemoteAuthMode } from "../../types/app";
import {
  REMOTE_DRAFTS_CACHE_KEY,
  REMOTE_EXPANDED_CACHE_KEY,
  REMOTE_SELECTED_CACHE_KEY,
  REMOTE_HISTORY_CACHE_KEY,
} from "./constants";
import type { RemoteServerDraft, ApiProxyPanelProps } from "./types";
import {
  createRemoteDraft,
  configToDraft,
  draftToConfig,
  writeStorageValue,
  readCachedRemoteDrafts,
  readCachedEditingRemoteId,
  readCachedSelectedRemoteId,
  readCachedRemoteHistory,
  isRemoteDraftConfigured,
  formatRemoteHistoryTime,
} from "./remoteDrafts";
import { REMOTE_AUTH_OPTIONS } from "./keyPolicy";
export function useRemoteServers({
  remoteServers,
  remoteStatuses,
  remoteLogs,
  refreshingRemoteId,
  deployingRemoteId,
  startingRemoteId,
  stoppingRemoteId,
  readingRemoteLogsId,
  installingDependencyName,
  installingDependencyTargetId,
  onUpdateRemoteServers,
  onRefreshRemoteStatus,
  onReadRemoteLogs,
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
  const [remoteDrafts, setRemoteDrafts] = useState<RemoteServerDraft[]>(() =>
    readCachedRemoteDrafts(remoteServers),
  );
  const [selectedRemoteId, setSelectedRemoteId] = useState<string | null>(() =>
    readCachedSelectedRemoteId(remoteServers),
  );
  const [editingRemoteId, setEditingRemoteId] = useState<string | null>(() =>
    readCachedEditingRemoteId(remoteServers),
  );
  const [diagnosticsRemoteId, setDiagnosticsRemoteId] = useState<string | null>(
    null,
  );
  const [remoteHistory, setRemoteHistory] = useState<Record<string, number>>(
    () => readCachedRemoteHistory(remoteServers),
  );
  const effectiveRemoteDrafts =
    remoteDrafts.length === 0 && remoteServers.length > 0
      ? remoteServers.map(configToDraft)
      : remoteDrafts;
  useEffect(() => {
    writeStorageValue(
      REMOTE_DRAFTS_CACHE_KEY,
      JSON.stringify(effectiveRemoteDrafts),
    );
  }, [effectiveRemoteDrafts]);
  useEffect(() => {
    const resolvedEditingRemoteId =
      editingRemoteId &&
      effectiveRemoteDrafts.some((draft) => draft.id === editingRemoteId)
        ? editingRemoteId
        : null;
    writeStorageValue(REMOTE_EXPANDED_CACHE_KEY, resolvedEditingRemoteId);
  }, [effectiveRemoteDrafts, editingRemoteId]);
  useEffect(() => {
    const resolvedSelectedRemoteId =
      selectedRemoteId &&
      effectiveRemoteDrafts.some((draft) => draft.id === selectedRemoteId)
        ? selectedRemoteId
        : (effectiveRemoteDrafts[0]?.id ?? null);
    writeStorageValue(
      REMOTE_SELECTED_CACHE_KEY,
      resolvedSelectedRemoteId,
      "local",
    );
  }, [effectiveRemoteDrafts, selectedRemoteId]);
  useEffect(() => {
    writeStorageValue(
      REMOTE_HISTORY_CACHE_KEY,
      JSON.stringify(remoteHistory),
      "local",
    );
  }, [remoteHistory]);
  const hasRemoteServers = effectiveRemoteDrafts.length > 0;
  const resolvedSelectedRemoteId =
    selectedRemoteId &&
    effectiveRemoteDrafts.some((draft) => draft.id === selectedRemoteId)
      ? selectedRemoteId
      : (effectiveRemoteDrafts[0]?.id ?? null);
  const selectedRemoteDraft =
    resolvedSelectedRemoteId === null
      ? null
      : (effectiveRemoteDrafts.find(
          (draft) => draft.id === resolvedSelectedRemoteId,
        ) ?? null);
  const selectedRemoteConfig = selectedRemoteDraft
    ? draftToConfig(selectedRemoteDraft)
    : null;
  const selectedRemoteStatus = selectedRemoteDraft
    ? remoteStatuses[selectedRemoteDraft.id]
    : null;
  const selectedRemoteLog = selectedRemoteDraft
    ? remoteLogs[selectedRemoteDraft.id]
    : undefined;
  const selectedRemoteConfigured = selectedRemoteDraft
    ? isRemoteDraftConfigured(selectedRemoteDraft)
    : false;
  const selectedRemoteIdentity = selectedRemoteDraft
    ? selectedRemoteDraft.label.trim() ||
      selectedRemoteDraft.host.trim() ||
      proxyCopy.remoteTitle
    : proxyCopy.remoteTitle;
  const selectedRefreshing =
    selectedRemoteDraft !== null &&
    refreshingRemoteId === selectedRemoteDraft.id;
  const selectedDeploying =
    selectedRemoteDraft !== null &&
    deployingRemoteId === selectedRemoteDraft.id;
  const selectedStarting =
    selectedRemoteDraft !== null && startingRemoteId === selectedRemoteDraft.id;
  const selectedStopping =
    selectedRemoteDraft !== null && stoppingRemoteId === selectedRemoteDraft.id;
  const selectedReadingLogs =
    selectedRemoteDraft !== null &&
    readingRemoteLogsId === selectedRemoteDraft.id;
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
    selectedRemoteDraft !== null
      ? (remoteHistory[selectedRemoteDraft.id] ?? 0)
      : 0;
  const selectedRemoteCheckedLabel =
    selectedRemoteLastChecked > 0
      ? formatRemoteHistoryTime(locale, selectedRemoteLastChecked)
      : proxyCopy.remoteNeverChecked;
  const editingSelectedRemote =
    selectedRemoteDraft !== null && editingRemoteId === selectedRemoteDraft.id;
  const diagnosticsOpen =
    selectedRemoteDraft !== null &&
    diagnosticsRemoteId === selectedRemoteDraft.id;
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
    const historyDelta =
      (remoteHistory[right.id] ?? 0) - (remoteHistory[left.id] ?? 0);
    if (historyDelta !== 0) {
      return historyDelta;
    }
    return (remoteOrder[left.id] ?? 0) - (remoteOrder[right.id] ?? 0);
  });
  const persistRemoteDrafts = (drafts: RemoteServerDraft[]) => {
    onUpdateRemoteServers(drafts.map(draftToConfig));
  };
  const updateRemoteDraft = (
    id: string,
    key: keyof Omit<RemoteServerDraft, "id">,
    value: string | RemoteAuthMode,
  ) => {
    setRemoteDrafts((current) =>
      (current.length === 0 && remoteServers.length > 0
        ? remoteServers.map(configToDraft)
        : current
      ).map((draft) => {
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
      ...(current.length === 0 && remoteServers.length > 0
        ? remoteServers.map(configToDraft)
        : current),
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
    setSelectedRemoteId((current) =>
      current === id ? (next[0]?.id ?? null) : current,
    );
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
  return {
    remoteAuthOptions,
    remoteDrafts,
    setRemoteDrafts,
    selectedRemoteId,
    setSelectedRemoteId,
    editingRemoteId,
    setEditingRemoteId,
    diagnosticsRemoteId,
    setDiagnosticsRemoteId,
    remoteHistory,
    setRemoteHistory,
    effectiveRemoteDrafts,
    hasRemoteServers,
    resolvedSelectedRemoteId,
    selectedRemoteDraft,
    selectedRemoteConfig,
    selectedRemoteStatus,
    selectedRemoteLog,
    selectedRemoteConfigured,
    selectedRemoteIdentity,
    selectedRefreshing,
    selectedDeploying,
    selectedStarting,
    selectedStopping,
    selectedReadingLogs,
    selectedInstallingDependency,
    selectedRemoteBusy,
    selectedRemoteLastChecked,
    selectedRemoteCheckedLabel,
    editingSelectedRemote,
    diagnosticsOpen,
    selectedRemoteRunningText,
    selectedRemoteInstalledText,
    selectedRemoteSystemdText,
    selectedRemoteEnabledText,
    remoteOrder,
    orderedRemoteDrafts,
    persistRemoteDrafts,
    updateRemoteDraft,
    addRemoteDraft,
    removeRemoteDraft,
    selectRemoteDraft,
    toggleSelectedDiagnostics,
    remoteGuideTitle,
    remoteGuideDescription,
  };
}
