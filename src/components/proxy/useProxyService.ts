import { useCallback, useMemo, useRef, useState } from "react";
import { useI18n } from "../../i18n/I18nProvider";
import { DEFAULT_PROXY_PORT } from "./constants";
import type { ApiProxyPanelProps } from "./types";
export function useProxyService({
  status,
  savedPort,
  sequentialFiveHourLimitPercent,
  starting,
  stopping,
  bindingCodexProxy,
  restoringCodexProxy,
  onStart,
  onPersistPort,
  onUpdateSequentialFiveHourLimitPercent,
}: ApiProxyPanelProps) {
  const { copy } = useI18n();
  const proxyCopy = copy.apiProxy;
  const busy = starting || stopping;
  const codexProxyBindingBusy = bindingCodexProxy || restoringCodexProxy;
  const [portDraft, setPortDraft] = useState<string | null>(null);
  const [sequentialLimitDraft, setSequentialLimitDraft] = useState<
    number | null
  >(null);
  const commitSequentialLimitRef = useRef<number | null>(null);
  const loadBalanceOptions = useMemo(
    () => [
      { id: "average" as const, label: proxyCopy.loadBalanceAverage },
      { id: "sequential" as const, label: proxyCopy.loadBalanceSequential },
      { id: "priority" as const, label: proxyCopy.loadBalancePriority },
    ],
    [proxyCopy.loadBalanceAverage, proxyCopy.loadBalanceSequential, proxyCopy.loadBalancePriority],
  );
  const portInput =
    portDraft ?? String(status.port ?? savedPort ?? DEFAULT_PROXY_PORT);
  const codexBindTitle = status.codexProxyBound
    ? proxyCopy.codexBindBoundTitle
    : proxyCopy.codexBindNormalTitle;
  const canBindCodexProxy =
    status.running &&
    Boolean(status.baseUrl) &&
    Boolean(status.apiKey) &&
    !busy &&
    !codexProxyBindingBusy;
  const canRestoreCodexProxy =
    status.codexProxyRestoreAvailable && !busy && !codexProxyBindingBusy;
  const effectiveSequentialLimit =
    sequentialLimitDraft ?? sequentialFiveHourLimitPercent;
  const rawPort = portInput.trim();
  const effectivePort = !rawPort
    ? 8787
    : Number.isInteger(Number(rawPort)) &&
        Number(rawPort) >= 1 &&
        Number(rawPort) <= 65535
      ? Number(rawPort)
      : null;
  const persistPortIfNeeded = async (explicitPort?: number | null) => {
    const nextPort = explicitPort ?? effectivePort;
    if (nextPort === null || nextPort === savedPort) {
      return;
    }
    await onPersistPort(nextPort);
  };
  const commitSequentialLimit = useCallback(
    (value: number) => {
      const nextValue = Math.min(100, Math.max(0, Math.round(value)));
      if (
        nextValue === sequentialFiveHourLimitPercent ||
        commitSequentialLimitRef.current === nextValue
      ) {
        setSequentialLimitDraft(null);
        return;
      }

      commitSequentialLimitRef.current = nextValue;
      void Promise.resolve(
        onUpdateSequentialFiveHourLimitPercent(nextValue),
      ).finally(() => {
        commitSequentialLimitRef.current = null;
        setSequentialLimitDraft(null);
      });
    },
    [onUpdateSequentialFiveHourLimitPercent, sequentialFiveHourLimitPercent],
  );
  const handleStart = async () => {
    await persistPortIfNeeded(effectivePort);
    await onStart(effectivePort);
    setPortDraft(null);
  };
  return {
    busy,
    codexProxyBindingBusy,
    portDraft,
    setPortDraft,
    sequentialLimitDraft,
    setSequentialLimitDraft,
    commitSequentialLimitRef,
    loadBalanceOptions,
    portInput,
    codexBindTitle,
    canBindCodexProxy,
    canRestoreCodexProxy,
    effectiveSequentialLimit,
    rawPort,
    effectivePort,
    persistPortIfNeeded,
    commitSequentialLimit,
    handleStart,
  };
}
