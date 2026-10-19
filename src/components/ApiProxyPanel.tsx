import type { ApiProxyPanelProps } from "./proxy/types";
import { useApiProxyWorkspace } from "./proxy/useApiProxyWorkspace";
import { ClassicProxySections } from "./classic/ClassicProxySections";

export function ApiProxyPanel(props: ApiProxyPanelProps) {
  const workspace = useApiProxyWorkspace(props);
  return <ClassicProxySections workspace={workspace} />;
}
