import { useI18n } from "../../i18n/I18nProvider";
import type { ApiProxyPanelProps } from "./types";
import { useProxyService } from "./useProxyService";
import { useProxyModels } from "./useProxyModels";
import { useProxyKeys } from "./useProxyKeys";
import { useRemoteServers } from "./useRemoteServers";
import { usePublicAccess } from "./usePublicAccess";
export function useApiProxyWorkspace(props: ApiProxyPanelProps) {
  const { copy, locale } = useI18n();
  return {
    ...props,
    copy,
    locale,
    proxyCopy: copy.apiProxy,
    ...useProxyService(props),
    ...useProxyModels(props),
    ...useProxyKeys(props),
    ...useRemoteServers(props),
    ...usePublicAccess(props),
  };
}
export type ApiProxyWorkspace = ReturnType<typeof useApiProxyWorkspace>;
