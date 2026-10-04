import type { ApiProxyPanelProps } from "./proxy/types";
import { useApiProxyWorkspace } from "./proxy/useApiProxyWorkspace";
import { ProxyService } from "./proxy/ProxyService";
import { ProxyConfiguration } from "./proxy/ProxyConfiguration";
import { ProxyUsage } from "./proxy/ProxyUsage";
import { ProxyRemote } from "./proxy/ProxyRemote";
import { ProxyPublicAccess } from "./proxy/ProxyPublicAccess";
import { PageSections } from "./workspace/PageSections";
import { PageToolbar } from "./workspace/PageToolbar";
import { getPageLayoutCopy } from "../i18n/pageLayoutCopy";
import { useAppLayout } from "../hooks/useAppLayout";
import { ClassicProxySections } from "./classic/ClassicProxySections";
export function ApiProxyPanel(props: ApiProxyPanelProps) {
  const workspace = useApiProxyWorkspace(props);
  const { layout } = useAppLayout();
  const text = getPageLayoutCopy(workspace.locale);
  if (layout === "classic")
    return <ClassicProxySections workspace={workspace} />;
  return (
    <section className="proxyPage workspacePage">
      <PageToolbar
        title={workspace.proxyCopy.title}
        detail={workspace.proxyCopy.hint}
      />
      <PageSections
        label={workspace.proxyCopy.title}
        sections={[
          {
            id: "service",
            label: text.service,
            content: <ProxyService workspace={workspace} />,
          },
          {
            id: "configuration",
            label: text.configuration,
            content: <ProxyConfiguration workspace={workspace} />,
          },
          {
            id: "usage",
            label: text.usage,
            content: <ProxyUsage workspace={workspace} />,
          },
          {
            id: "remote",
            label: text.remote,
            content: <ProxyRemote workspace={workspace} />,
          },
          {
            id: "public",
            label: text.publicAccess,
            content: <ProxyPublicAccess workspace={workspace} />,
          },
        ]}
      />
    </section>
  );
}
