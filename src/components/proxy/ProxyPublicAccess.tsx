import { copyText } from "./clipboard";
import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function ProxyPublicAccess({
  workspace,
}: {
  workspace: ApiProxyWorkspace;
}) {
  const {
    status,
    cloudflaredStatus,
    installingCloudflared,
    startingCloudflared,
    stoppingCloudflared,
    onRefreshCloudflared,
    onInstallCloudflared,
    onStartCloudflared,
    onStopCloudflared,
    proxyCopy,
    cloudflaredBusy,
    publicAccessEnabled,
    setPublicAccessEnabled,
    tunnelMode,
    setTunnelMode,
    useHttp2,
    setUseHttp2,
    namedInput,
    setNamedInput,
    cloudflaredEnabled,
    canStartCloudflared,
    cloudflaredInput,
  } = workspace;

  return (
    <div className="proxySectionContent">
      <div className="proxySectionHeader">
        <h3>{proxyCopy.cloudflaredTitle}</h3>
        <div className="proxySwitchRow proxySectionToggle">
          <div className="settingMeta">
            <strong>{proxyCopy.cloudflaredToggle}</strong>
          </div>
          <label
            className="themeSwitch"
            aria-label={proxyCopy.cloudflaredToggle}
          >
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
                <span className="proxyLabel">
                  {proxyCopy.notInstalledLabel}
                </span>
                <strong>{proxyCopy.installTitle}</strong>
                <p>{proxyCopy.installDescription}</p>
              </div>
              <button
                className="primary"
                onClick={onInstallCloudflared}
                disabled={installingCloudflared}
              >
                {installingCloudflared
                  ? proxyCopy.installing
                  : proxyCopy.installButton}
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
                        setNamedInput((current) => ({
                          ...current,
                          apiToken: event.target.value,
                        }))
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
                        setNamedInput((current) => ({
                          ...current,
                          accountId: event.target.value,
                        }))
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
                        setNamedInput((current) => ({
                          ...current,
                          zoneId: event.target.value,
                        }))
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
                        setNamedInput((current) => ({
                          ...current,
                          hostname: event.target.value,
                        }))
                      }
                      placeholder={proxyCopy.hostnamePlaceholder}
                      disabled={cloudflaredBusy || cloudflaredStatus.running}
                    />
                  </label>
                </div>
              ) : null}

              <div className="cloudflaredToolbar">
                <div className="proxySwitchRow cloudflaredToolbarMeta">
                  <div className="settingMeta">
                    <strong>{proxyCopy.useHttp2}</strong>
                  </div>
                  <label
                    className="themeSwitch"
                    aria-label={proxyCopy.useHttp2}
                  >
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
                      {stoppingCloudflared
                        ? proxyCopy.stoppingPublic
                        : proxyCopy.stopPublic}
                    </button>
                  ) : (
                    <button
                      className="primary"
                      onClick={() => {
                        if (cloudflaredInput) {
                          onStartCloudflared(cloudflaredInput);
                        }
                      }}
                      disabled={
                        !canStartCloudflared || cloudflaredInput === null
                      }
                    >
                      {startingCloudflared
                        ? proxyCopy.startingPublic
                        : proxyCopy.startPublic}
                    </button>
                  )}
                </div>
              </div>

              <div className="proxyDetailGrid">
                <article className="proxyDetailCard">
                  <span className="proxyLabel">
                    {proxyCopy.publicStatusLabel}
                  </span>
                  <strong
                    className={`proxyStatus${cloudflaredStatus.running ? " isRunning" : ""}`}
                  >
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
                    <span className="proxyLabel">
                      {proxyCopy.publicUrlLabel}
                    </span>
                    <button
                      className="ghost proxyCopyButton"
                      onClick={() => copyText(cloudflaredStatus.publicUrl)}
                      disabled={!cloudflaredStatus.publicUrl}
                    >
                      {proxyCopy.copy}
                    </button>
                  </div>
                  <code>
                    {cloudflaredStatus.publicUrl ??
                      proxyCopy.baseUrlPlaceholder}
                  </code>
                </article>

                <article className="proxyDetailCard">
                  <span className="proxyLabel">
                    {proxyCopy.installPathLabel}
                  </span>
                  <code>
                    {cloudflaredStatus.binaryPath ?? proxyCopy.notDetected}
                  </code>
                </article>

                <article className="proxyDetailCard">
                  <span className="proxyLabel">{proxyCopy.lastErrorLabel}</span>
                  <p className="proxyErrorText">
                    {cloudflaredStatus.lastError ?? proxyCopy.none}
                  </p>
                </article>
              </div>
            </>
          )}
        </div>
      ) : null}
    </div>
  );
}
