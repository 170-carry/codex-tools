import { useCallback, useMemo, useState } from "react";
import { useI18n } from "../../i18n/I18nProvider";
import type { ApiProxyKey } from "../../types/app";
import {
  API_PROXY_REASONING_OPTION_IDS,
  API_PROXY_SERVICE_TIER_OPTION_IDS,
} from "./constants";
import type { ApiProxyPanelProps } from "./types";
import { toggleStringValue } from "./keyPolicy";
export function useProxyKeys({
  apiProxySupportedModels,
  onCreateApiProxyKey,
  onUpdateApiProxyKey,
}: ApiProxyPanelProps) {
  const { copy } = useI18n();
  const proxyCopy = copy.apiProxy;
  const apiProxyReasoningOptions = useMemo(
    () => [
      { id: "none", label: proxyCopy.reasoningNone },
      { id: "minimal", label: proxyCopy.reasoningMinimal },
      { id: "low", label: proxyCopy.reasoningLow },
      { id: "medium", label: proxyCopy.reasoningMedium },
      { id: "high", label: proxyCopy.reasoningHigh },
      { id: "xhigh", label: proxyCopy.reasoningXHigh },
      { id: "max", label: proxyCopy.reasoningMax },
    ],
    [
      proxyCopy.reasoningHigh,
      proxyCopy.reasoningLow,
      proxyCopy.reasoningMax,
      proxyCopy.reasoningMedium,
      proxyCopy.reasoningMinimal,
      proxyCopy.reasoningNone,
      proxyCopy.reasoningXHigh,
    ],
  );
  const apiProxyServiceTierOptions = useMemo(
    () => [
      { id: "auto", label: proxyCopy.serviceTierAuto },
      { id: "default", label: proxyCopy.serviceTierDefault },
      { id: "fast", label: proxyCopy.serviceTierFast },
      { id: "flex", label: proxyCopy.serviceTierFlex },
    ],
    [
      proxyCopy.serviceTierAuto,
      proxyCopy.serviceTierDefault,
      proxyCopy.serviceTierFast,
      proxyCopy.serviceTierFlex,
    ],
  );
  const [newApiProxyKeyLabel, setNewApiProxyKeyLabel] = useState("");
  const [newApiProxyKeyValue, setNewApiProxyKeyValue] = useState("");
  const [apiProxyKeyLabelDrafts, setApiProxyKeyLabelDrafts] = useState<
    Record<string, string>
  >({});
  const handleCreateApiProxyKey = useCallback(async () => {
    await onCreateApiProxyKey({
      label: newApiProxyKeyLabel.trim() || proxyCopy.keyCreateDefaultLabel,
      key: newApiProxyKeyValue.trim() || null,
      allowedModels: [],
      allowedReasoningEfforts: [],
      allowedServiceTiers: [],
    });
    setNewApiProxyKeyLabel("");
    setNewApiProxyKeyValue("");
  }, [
    newApiProxyKeyLabel,
    newApiProxyKeyValue,
    onCreateApiProxyKey,
    proxyCopy.keyCreateDefaultLabel,
  ]);
  const updateApiProxyKeyModels = useCallback(
    (key: ApiProxyKey, model: string, enabled: boolean) => {
      const currentModels =
        key.allowedModels.length === 0
          ? apiProxySupportedModels
          : key.allowedModels;
      const nextModels = toggleStringValue(currentModels, model, enabled);
      void onUpdateApiProxyKey({ id: key.id, allowedModels: nextModels });
    },
    [apiProxySupportedModels, onUpdateApiProxyKey],
  );
  const updateApiProxyKeyReasoning = useCallback(
    (key: ApiProxyKey, effort: string, enabled: boolean) => {
      const current =
        key.allowedReasoningEfforts.length === 0
          ? [...API_PROXY_REASONING_OPTION_IDS]
          : key.allowedReasoningEfforts;
      const next = toggleStringValue(current, effort, enabled);
      void onUpdateApiProxyKey({ id: key.id, allowedReasoningEfforts: next });
    },
    [onUpdateApiProxyKey],
  );
  const updateApiProxyKeyServiceTier = useCallback(
    (key: ApiProxyKey, tier: string, enabled: boolean) => {
      const current =
        key.allowedServiceTiers.length === 0
          ? [...API_PROXY_SERVICE_TIER_OPTION_IDS]
          : key.allowedServiceTiers;
      const next = toggleStringValue(current, tier, enabled);
      void onUpdateApiProxyKey({ id: key.id, allowedServiceTiers: next });
    },
    [onUpdateApiProxyKey],
  );
  const commitApiProxyKeyLabel = useCallback(
    (key: ApiProxyKey) => {
      const nextLabel = (apiProxyKeyLabelDrafts[key.id] ?? key.label).trim();
      if (!nextLabel) {
        setApiProxyKeyLabelDrafts((drafts) => ({
          ...drafts,
          [key.id]: key.label,
        }));
        return;
      }

      setApiProxyKeyLabelDrafts((drafts) => ({
        ...drafts,
        [key.id]: nextLabel,
      }));
      if (nextLabel !== key.label) {
        void onUpdateApiProxyKey({ id: key.id, label: nextLabel });
      }
    },
    [apiProxyKeyLabelDrafts, onUpdateApiProxyKey],
  );
  return {
    apiProxyReasoningOptions,
    apiProxyServiceTierOptions,
    newApiProxyKeyLabel,
    setNewApiProxyKeyLabel,
    newApiProxyKeyValue,
    setNewApiProxyKeyValue,
    apiProxyKeyLabelDrafts,
    setApiProxyKeyLabelDrafts,
    handleCreateApiProxyKey,
    updateApiProxyKeyModels,
    updateApiProxyKeyReasoning,
    updateApiProxyKeyServiceTier,
    commitApiProxyKeyLabel,
  };
}
