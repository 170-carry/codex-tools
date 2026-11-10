import { Switch } from "../ui/Switch";
import { PublicTunnelSettings } from "./PublicTunnelSettings";
import { PublicAccessStatus } from "./PublicAccessStatus";
import { SetupProgress } from "../ui/SetupProgress";
import { uiCopy } from "../../i18n/uiCopy";
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
    useHttp2,
    setUseHttp2,
    cloudflaredEnabled,
    canStartCloudflared,
    cloudflaredInput,
  } = workspace;

  const text = uiCopy(workspace.locale);
  return (
    <div className="proxySectionContent">
      <div className="proxySectionHeader">
        <h3>{proxyCopy.cloudflaredTitle}</h3>
        <div className="proxySwitchRow proxySectionToggle">
          <Switch label={proxyCopy.cloudflaredToggle}
              checked={publicAccessEnabled}
              onChange={(checked) => setPublicAccessEnabled(checked)} />
        </div>
      </div>

      {cloudflaredEnabled ? (
        <div className="cloudflaredContent">
          <SetupProgress steps={[text.installation, text.configuration, text.service]} current={!cloudflaredStatus.installed ? 0 : cloudflaredStatus.running ? 2 : 1} />
          {!status.running ? (
            <article className="cloudflaredCallout">
              <strong>{proxyCopy.startLocalProxyFirstTitle}</strong>
            </article>
          ) : null}

          {!cloudflaredStatus.installed ? (
            <article className="cloudflaredInstallCard">
              <div>
                <strong>{proxyCopy.installTitle}</strong>
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
              <PublicTunnelSettings workspace={workspace} />

              <div className="cloudflaredToolbar">
                <div className="proxySwitchRow cloudflaredToolbarMeta">
                  <div className="settingMeta">
                    <strong>{proxyCopy.useHttp2}</strong>
                  </div>
                  <Switch label={proxyCopy.useHttp2}
                      checked={useHttp2}
                      onChange={(checked) => setUseHttp2(checked)}
                      disabled={cloudflaredBusy || cloudflaredStatus.running} />
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

              <PublicAccessStatus workspace={workspace} />
            </>
          )}
        </div>
      ) : null}
    </div>
  );
}
