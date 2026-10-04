import type { ApiProxyWorkspace } from "../proxy/useApiProxyWorkspace";
import { ProxyService } from "../proxy/ProxyService";
import { ProxyUsage } from "../proxy/ProxyUsage";
import { ProxyConfiguration } from "../proxy/ProxyConfiguration";
import { ProxyRemote } from "../proxy/ProxyRemote";
import { ProxyPublicAccess } from "../proxy/ProxyPublicAccess";
import { getPageLayoutCopy } from "../../i18n/pageLayoutCopy";

export function ClassicProxySections({
  workspace,
}: {
  workspace: ApiProxyWorkspace;
}) {
  const text = getPageLayoutCopy(workspace.locale);
  return (
    <section className="proxyPage classicProxyPage">
      <div className="proxyShell">
        <ProxyService workspace={workspace} />
        <ProxyUsage workspace={workspace} />
        <section className="proxySectionCard" aria-label={text.configuration}>
          <ProxyConfiguration workspace={workspace} />
        </section>
        <section className="proxySectionCard" aria-label={text.remote}>
          <ProxyRemote workspace={workspace} />
        </section>
        <section className="proxySectionCard" aria-label={text.publicAccess}>
          <ProxyPublicAccess workspace={workspace} />
        </section>
      </div>
    </section>
  );
}
