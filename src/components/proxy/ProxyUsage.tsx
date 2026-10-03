import { ApiProxyUsageChart } from "./ApiProxyUsageChart";
import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function ProxyUsage({ workspace }: { workspace: ApiProxyWorkspace }) {
  const {
    status,
    apiProxyKeys,
    apiProxyUsageStats,
    apiProxyUsageRange,
    apiProxyUsageMetric,
    apiProxyUsageLoading,
    apiProxyUsageClearing,
    apiProxyUsageExporting,
    onSelectApiProxyUsageRange,
    onSelectApiProxyUsageMetric,
    onExportApiProxyUsage,
    onClearApiProxyUsageStats,
    locale,
    proxyCopy,
  } = workspace;

  return (
    <ApiProxyUsageChart
      copy={proxyCopy}
      locale={locale}
      stats={apiProxyUsageStats}
      range={apiProxyUsageRange}
      metric={apiProxyUsageMetric}
      loading={apiProxyUsageLoading}
      clearing={apiProxyUsageClearing}
      exporting={apiProxyUsageExporting}
      proxyRunning={status.running}
      apiProxyKeys={apiProxyKeys}
      onSelectRange={onSelectApiProxyUsageRange}
      onSelectMetric={onSelectApiProxyUsageMetric}
      onExport={onExportApiProxyUsage}
      onClear={onClearApiProxyUsageStats}
    />
  );
}
