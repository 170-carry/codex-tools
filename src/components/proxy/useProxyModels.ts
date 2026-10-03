import { useCallback, useEffect, useMemo, useState } from "react";
import type { ApiProxyPanelProps } from "./types";
import { normalizeDisabledProxyModels } from "./usageFormat";
export function useProxyModels({
  apiProxySupportedModels,
  apiProxyDisabledModels,
  onUpdateApiProxyDisabledModels,
}: ApiProxyPanelProps) {
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const [modelMenuSaving, setModelMenuSaving] = useState(false);
  const [modelMenuDraft, setModelMenuDraft] = useState<string[]>(() =>
    normalizeDisabledProxyModels(
      apiProxyDisabledModels,
      apiProxySupportedModels,
    ),
  );
  const [modelSearchQuery, setModelSearchQuery] = useState("");
  const effectiveDisabledModels = useMemo(
    () =>
      normalizeDisabledProxyModels(
        apiProxyDisabledModels,
        apiProxySupportedModels,
      ),
    [apiProxyDisabledModels, apiProxySupportedModels],
  );
  const effectiveModelMenuDraft = useMemo(
    () => normalizeDisabledProxyModels(modelMenuDraft, apiProxySupportedModels),
    [apiProxySupportedModels, modelMenuDraft],
  );
  const enabledModelCount =
    apiProxySupportedModels.length - effectiveDisabledModels.length;
  const hasModelMenuChanges =
    effectiveDisabledModels.length !== effectiveModelMenuDraft.length ||
    effectiveDisabledModels.some(
      (model, index) => model !== effectiveModelMenuDraft[index],
    );
  const normalizedModelSearchQuery = modelSearchQuery
    .trim()
    .toLocaleLowerCase();
  const filteredProxyModels = useMemo(
    () =>
      apiProxySupportedModels.filter((model) =>
        normalizedModelSearchQuery === ""
          ? true
          : model.toLocaleLowerCase().includes(normalizedModelSearchQuery),
      ),
    [apiProxySupportedModels, normalizedModelSearchQuery],
  );
  useEffect(() => {
    if (modelMenuOpen) {
      return;
    }
    // Keep the closed-menu draft synchronized with the persisted selection.
    setModelMenuDraft(effectiveDisabledModels);
  }, [effectiveDisabledModels, modelMenuOpen]);
  useEffect(() => {
    if (!modelMenuOpen) {
      // Closing the menu is the lifecycle boundary for its transient search.
      setModelSearchQuery("");
      return;
    }

    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !modelMenuSaving) {
        setModelMenuOpen(false);
      }
    };

    window.addEventListener("keydown", closeOnEscape);
    return () => {
      window.removeEventListener("keydown", closeOnEscape);
    };
  }, [modelMenuOpen, modelMenuSaving]);
  const handleToggleProxyModel = useCallback(
    (model: string, enabled: boolean) => {
      setModelMenuDraft((current) => {
        const currentDisabled = normalizeDisabledProxyModels(
          current,
          apiProxySupportedModels,
        );
        if (enabled) {
          return currentDisabled.filter((item) => item !== model);
        }
        if (currentDisabled.includes(model)) {
          return currentDisabled;
        }
        return normalizeDisabledProxyModels(
          [...currentDisabled, model],
          apiProxySupportedModels,
        );
      });
    },
    [apiProxySupportedModels],
  );
  const handleSaveProxyModels = useCallback(async () => {
    if (modelMenuSaving) {
      return;
    }
    setModelMenuSaving(true);
    try {
      await onUpdateApiProxyDisabledModels(effectiveModelMenuDraft);
      setModelMenuOpen(false);
    } finally {
      setModelMenuSaving(false);
    }
  }, [
    effectiveModelMenuDraft,
    modelMenuSaving,
    onUpdateApiProxyDisabledModels,
  ]);
  return {
    modelMenuOpen,
    setModelMenuOpen,
    modelMenuSaving,
    setModelMenuSaving,
    modelMenuDraft,
    setModelMenuDraft,
    modelSearchQuery,
    setModelSearchQuery,
    effectiveDisabledModels,
    effectiveModelMenuDraft,
    enabledModelCount,
    hasModelMenuChanges,
    normalizedModelSearchQuery,
    filteredProxyModels,
    handleToggleProxyModel,
    handleSaveProxyModels,
  };
}
