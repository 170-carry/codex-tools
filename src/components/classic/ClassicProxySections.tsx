import { useState } from "react";
import { PageFrame } from "../ui/PageFrame";
import type { ApiProxyWorkspace } from "../proxy/useApiProxyWorkspace";
import { ProxyService } from "../proxy/ProxyService";
import { ProxyUsage } from "../proxy/ProxyUsage";
import { ProxyConfiguration } from "../proxy/ProxyConfiguration";
import { ProxyRemote } from "../proxy/ProxyRemote";
import { ProxyPublicAccess } from "../proxy/ProxyPublicAccess";
import { SectionTabs } from "../ui/SectionTabs";
import { getPageLayoutCopy } from "../../i18n/pageLayoutCopy";

export function ClassicProxySections({
  workspace,
}: {
  workspace: ApiProxyWorkspace;
}) {
  const [active, setActive] = useState("usage");
  const text = getPageLayoutCopy(workspace.locale);
  return (
    <PageFrame kind="service" className="proxyPage classicProxyPage">
      <div className="proxyShell">
        <div hidden={active === "remote"}><ProxyService workspace={workspace} /></div>
        <SectionTabs
          label="API Proxy"
          initial="usage"
          value={active}
          onValueChange={setActive}
          sections={[
            {
              id: "usage",
              label: text.usage,
              content: <ProxyUsage workspace={workspace} />,
            },
            {
              id: "configuration",
              label: text.configuration,
              content: (
                <section
                  className="proxyTabContent"
                  aria-label={text.configuration}
                >
                  <ProxyConfiguration workspace={workspace} />
                </section>
              ),
            },
            {
              id: "remote",
              label: text.remote,
              content: (
                <section className="proxyTabContent" aria-label={text.remote}>
                  <ProxyRemote workspace={workspace} />
                </section>
              ),
            },
            {
              id: "public",
              label: text.publicAccess,
              content: (
                <section
                  className="proxyTabContent"
                  aria-label={text.publicAccess}
                >
                  <ProxyPublicAccess workspace={workspace} />
                </section>
              ),
            },
          ]}
        />
      </div>
    </PageFrame>
  );
}
