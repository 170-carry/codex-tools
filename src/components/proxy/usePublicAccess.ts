import { useState } from "react";
import type {
  CloudflaredTunnelMode,
  StartCloudflaredTunnelInput,
} from "../../types/app";
import type { ApiProxyPanelProps } from "./types";
export function usePublicAccess({
  status,
  cloudflaredStatus,
  installingCloudflared,
  startingCloudflared,
  stoppingCloudflared,
}: ApiProxyPanelProps) {
  const cloudflaredBusy =
    installingCloudflared || startingCloudflared || stoppingCloudflared;
  const [publicAccessEnabled, setPublicAccessEnabled] = useState(
    cloudflaredStatus.running,
  );
  const [tunnelMode, setTunnelMode] = useState<CloudflaredTunnelMode>(
    cloudflaredStatus.tunnelMode ?? "quick",
  );
  const [useHttp2, setUseHttp2] = useState(cloudflaredStatus.useHttp2);
  const [namedInput, setNamedInput] = useState({
    apiToken: "",
    accountId: "",
    zoneId: "",
    hostname: cloudflaredStatus.customHostname ?? "",
  });
  const cloudflaredEnabled = publicAccessEnabled || cloudflaredStatus.running;
  const namedReady =
    namedInput.apiToken.trim() !== "" &&
    namedInput.accountId.trim() !== "" &&
    namedInput.zoneId.trim() !== "" &&
    namedInput.hostname.trim() !== "";
  const canStartCloudflared =
    status.running &&
    status.port !== null &&
    cloudflaredStatus.installed &&
    !cloudflaredBusy &&
    (tunnelMode === "quick" || namedReady);
  const cloudflaredInput: StartCloudflaredTunnelInput | null =
    status.port === null
      ? null
      : {
          apiProxyPort: status.port,
          useHttp2,
          mode: tunnelMode,
          named:
            tunnelMode === "named"
              ? {
                  apiToken: namedInput.apiToken.trim(),
                  accountId: namedInput.accountId.trim(),
                  zoneId: namedInput.zoneId.trim(),
                  hostname: namedInput.hostname.trim(),
                }
              : null,
        };
  return {
    cloudflaredBusy,
    publicAccessEnabled,
    setPublicAccessEnabled,
    tunnelMode,
    setTunnelMode,
    useHttp2,
    setUseHttp2,
    namedInput,
    setNamedInput,
    cloudflaredEnabled,
    namedReady,
    canStartCloudflared,
    cloudflaredInput,
  };
}
