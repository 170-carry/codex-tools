import { copyText } from "./clipboard";
import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function PublicAccessStatus({workspace}: {workspace: ApiProxyWorkspace}) {
 const {cloudflaredStatus,proxyCopy}=workspace;
 return               <div className="proxyDetailGrid">
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
;
}
