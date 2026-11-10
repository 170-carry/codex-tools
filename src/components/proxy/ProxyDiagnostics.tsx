import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function ProxyDiagnostics({ workspace }: { workspace: ApiProxyWorkspace }) {
const {status, proxyCopy}=workspace;
return <div className="proxyDetailGrid proxyDiagnosticRows">
        <article className="proxyDetailCard">
          <span className="proxyLabel">{proxyCopy.activeAccountLabel}</span>
          <strong>
            {status.activeAccountLabel ?? proxyCopy.activeAccountEmptyTitle}
          </strong>
          {status.activeAccountId ? <p>{status.activeAccountId}</p> : null}
        </article>

        <article className="proxyDetailCard">
          <span className="proxyLabel">{proxyCopy.lastErrorLabel}</span>
          <p className="proxyErrorText">{status.lastError ?? proxyCopy.none}</p>
        </article>
</div>;
}
