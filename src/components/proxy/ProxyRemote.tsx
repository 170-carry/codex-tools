import { RemoteHistory } from "./RemoteHistory";
import { RemoteEditor } from "./RemoteEditor";
import { copyText } from "./clipboard";
import { buildRemoteBaseUrl } from "./remoteDrafts";
import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function ProxyRemote({ workspace }: { workspace: ApiProxyWorkspace }) {
  const {
    onRefreshRemoteStatus,
    onDeployRemote,
    onStartRemote,
    onStopRemote,
    proxyCopy,
    setEditingRemoteId,
    setRemoteHistory,
    hasRemoteServers,
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
    selectedRemoteBusy,
    selectedRemoteCheckedLabel,
    editingSelectedRemote,
    diagnosticsOpen,
    selectedRemoteRunningText,
    selectedRemoteInstalledText,
    selectedRemoteSystemdText,
    selectedRemoteEnabledText,
    addRemoteDraft,
    toggleSelectedDiagnostics,
    remoteGuideTitle,
    remoteGuideDescription,
  } = workspace;

  return (
    <div className="proxySectionContent">
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
          <RemoteHistory workspace={workspace} />

          {selectedRemoteDraft ? (
            <div className="remoteWorkbench">
              <article className="remoteWorkbenchCard">
                <div className="remoteWorkbenchHeader">
                  <div className="remoteServerSummary">
                    <div className="remoteServerIdentity">
                      <strong>{selectedRemoteIdentity}</strong>
                      <span>
                        {selectedRemoteStatus?.baseUrl ??
                          buildRemoteBaseUrl(selectedRemoteDraft)}
                      </span>
                    </div>
                    <div className="remoteServerSummaryMeta">
                      <span className="remoteServerSummaryPill">
                        {proxyCopy.remoteHostLabel}{" "}
                        {selectedRemoteDraft.host.trim() || "--"}
                      </span>
                      <span className="remoteServerSummaryPill">
                        SSH {selectedRemoteDraft.sshUser.trim() || "root"}:
                        {selectedRemoteDraft.sshPort.trim() || "--"}
                      </span>
                      <span className="remoteServerSummaryPill">
                        {proxyCopy.remoteListenPortLabel}{" "}
                        {selectedRemoteDraft.listenPort.trim() || "--"}
                      </span>
                      <span className="remoteServerSummaryPill">
                        {proxyCopy.remoteLastCheckedLabel}{" "}
                        {selectedRemoteCheckedLabel}
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
                      {selectedRefreshing
                        ? proxyCopy.remoteRefreshing
                        : proxyCopy.remoteRefresh}
                    </button>
                    <button
                      className="ghost"
                      onClick={() =>
                        setEditingRemoteId((current) =>
                          current === selectedRemoteDraft.id
                            ? null
                            : selectedRemoteDraft.id,
                        )
                      }
                    >
                      {editingSelectedRemote
                        ? proxyCopy.remoteCollapse
                        : proxyCopy.remoteExpand}
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
                          disabled={
                            !selectedRemoteConfigured || selectedRemoteBusy
                          }
                        >
                          {selectedStopping
                            ? proxyCopy.remoteStopping
                            : proxyCopy.remoteStop}
                        </button>
                      ) : (
                        <button
                          className="primary"
                          onClick={() => {
                            if (selectedRemoteConfig) {
                              onStartRemote(selectedRemoteConfig);
                            }
                          }}
                          disabled={
                            !selectedRemoteConfigured || selectedRemoteBusy
                          }
                        >
                          {selectedStarting
                            ? proxyCopy.remoteStarting
                            : proxyCopy.remoteStart}
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
                        disabled={
                          !selectedRemoteConfigured || selectedRemoteBusy
                        }
                      >
                        {selectedDeploying
                          ? proxyCopy.remoteDeploying
                          : proxyCopy.remoteDeploy}
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
                          onClick={() =>
                            copyText(selectedRemoteStatus.apiKey ?? null)
                          }
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
                      <span className="proxyLabel">
                        {proxyCopy.remoteBaseUrlLabel}
                      </span>
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
                      {selectedRemoteStatus?.baseUrl ??
                        buildRemoteBaseUrl(selectedRemoteDraft)}
                    </code>
                  </article>

                  <article className="proxyDetailCard">
                    <div className="proxyDetailHeader">
                      <span className="proxyLabel">
                        {proxyCopy.remoteApiKeyLabel}
                      </span>
                      <button
                        className="ghost proxyCopyButton"
                        onClick={() =>
                          copyText(selectedRemoteStatus?.apiKey ?? null)
                        }
                        disabled={!selectedRemoteStatus?.apiKey}
                      >
                        {proxyCopy.copy}
                      </button>
                    </div>
                    <code>
                      {selectedRemoteStatus?.apiKey ??
                        proxyCopy.apiKeyPlaceholder}
                    </code>
                  </article>

                  <article className="proxyDetailCard">
                    <span className="proxyLabel">
                      {proxyCopy.remoteServiceLabel}
                    </span>
                    <code>
                      {selectedRemoteStatus?.serviceName ??
                        proxyCopy.remoteStatusUnknown}
                    </code>
                  </article>
                </div>
              </article>

              <RemoteEditor workspace={workspace} />

              <div className="remoteWorkbenchSection">
                <div className="remoteWorkbenchSectionHeader">
                  <div>
                    <span className="proxyLabel">
                      {proxyCopy.remoteLogsLabel}
                    </span>
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
                        <span className="proxyLabel">
                          {proxyCopy.remoteLogsLabel}
                        </span>
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
                      <span className="proxyLabel">
                        {proxyCopy.remoteLastErrorLabel}
                      </span>
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
    </div>
  );
}
