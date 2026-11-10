import { Presence } from "../ui/Presence";
import { ProxyKeys } from "./ProxyKeys";
import { ProxyModelDialog } from "./ProxyModelDialog";
import { ProxyRouting } from "./ProxyRouting";
import { ProxyDiagnostics } from "./ProxyDiagnostics";
import { SectionGroup } from "../ui/PageFrame";
import { uiCopy } from "../../i18n/uiCopy";
import { useI18n } from "../../i18n/I18nProvider";
import { copyText } from "./clipboard";
import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function ProxyConfiguration({workspace}: {workspace: ApiProxyWorkspace}) {
 const {locale}=useI18n();
 const text=uiCopy(locale);
 const {status,proxyCopy,refreshingApiKey,onRefreshApiKey}=workspace;
 return <div className="proxySectionContent proxyConfigurationContent">
  <SectionGroup title={text.routing}><ProxyRouting workspace={workspace}/></SectionGroup>
  <SectionGroup title={proxyCopy.apiKeyLabel}><div className="proxyConnectionDetails">        <article className="proxyDetailCard">
          <div className="proxyDetailHeader">
            <span className="proxyLabel">{proxyCopy.apiKeyLabel}</span>
            <div className="proxyDetailActions">
              <button
                className="ghost proxyCopyButton"
                onClick={onRefreshApiKey}
                disabled={refreshingApiKey}
              >
                {refreshingApiKey
                  ? proxyCopy.refreshingKey
                  : proxyCopy.refreshKey}
              </button>
              <button
                className="ghost proxyCopyButton"
                onClick={() => copyText(status.apiKey)}
                disabled={!status.apiKey}
              >
                {proxyCopy.copy}
              </button>
            </div>
          </div>
          <code>{status.apiKey ?? proxyCopy.apiKeyPlaceholder}</code>
        </article>

</div><ProxyKeys workspace={workspace}/></SectionGroup>
  <SectionGroup title={text.diagnostics}><ProxyDiagnostics workspace={workspace}/></SectionGroup>
  <Presence present={workspace.modelMenuOpen}><ProxyModelDialog workspace={workspace}/></Presence>
 </div>;
}
