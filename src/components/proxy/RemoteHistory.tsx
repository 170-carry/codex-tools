import {
  isRemoteDraftConfigured,
  formatRemoteHistoryTime,
} from "./remoteDrafts";
import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function RemoteHistory({ workspace }: { workspace: ApiProxyWorkspace }) {
  const {
    remoteStatuses,
    refreshingRemoteId,
    locale,
    proxyCopy,
    remoteHistory,
    resolvedSelectedRemoteId,
    orderedRemoteDrafts,
    selectRemoteDraft,
  } = workspace;

  return (
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
              aria-pressed={resolvedSelectedRemoteId === draft.id}
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
                  SSH {draft.sshUser.trim() || "root"}:
                  {draft.sshPort.trim() || "--"}
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
  );
}
