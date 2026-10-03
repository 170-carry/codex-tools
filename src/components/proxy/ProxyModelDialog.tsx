import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
import { UtilityDialog } from "../workspace/UtilityDialog";

export function ProxyModelDialog({
  workspace,
}: {
  workspace: ApiProxyWorkspace;
}) {
  const {
    apiProxySupportedModels,
    copy,
    proxyCopy,
    modelMenuOpen,
    setModelMenuOpen,
    modelMenuSaving,
    setModelMenuDraft,
    modelSearchQuery,
    setModelSearchQuery,
    effectiveModelMenuDraft,
    hasModelMenuChanges,
    filteredProxyModels,
    handleToggleProxyModel,
    handleSaveProxyModels,
  } = workspace;
  if (!modelMenuOpen) return null;
  return (
    <UtilityDialog
      className="proxyModelDialog"
      title={proxyCopy.modelMenuTitle}
      description={proxyCopy.modelMenuDialogDescription}
      closeLabel={copy.common.close}
      onClose={() => setModelMenuOpen(false)}
      dismissible={!modelMenuSaving}
      actions={
        <>
          <button
            type="button"
            className="ghost"
            disabled={modelMenuSaving}
            onClick={() => setModelMenuOpen(false)}
          >
            {proxyCopy.modelMenuCancel}
          </button>
          <button
            type="button"
            className="primary"
            disabled={modelMenuSaving || !hasModelMenuChanges}
            onClick={() => void handleSaveProxyModels()}
          >
            {proxyCopy.modelMenuSave}
          </button>
        </>
      }
    >
      <div className="proxyModelDialogToolbar">
        <label className="proxyModelSearchField">
          <span className="visuallyHidden">
            {proxyCopy.modelMenuSearchLabel}
          </span>
          <input
            className="proxyModelSearchInput"
            type="search"
            value={modelSearchQuery}
            onChange={(event) => setModelSearchQuery(event.currentTarget.value)}
            placeholder={proxyCopy.modelMenuSearchPlaceholder}
            spellCheck={false}
          />
        </label>
        <div className="proxyModelDialogToolbarActions">
          <button
            type="button"
            className="ghost"
            onClick={() => setModelMenuDraft([])}
            disabled={modelMenuSaving || apiProxySupportedModels.length === 0}
          >
            {proxyCopy.modelMenuEnableAll}
          </button>
          <button
            type="button"
            className="ghost"
            onClick={() => setModelMenuDraft(apiProxySupportedModels)}
            disabled={modelMenuSaving || apiProxySupportedModels.length === 0}
          >
            {proxyCopy.modelMenuDisableAll}
          </button>
        </div>
      </div>
      <div className="proxyModelDialogList">
        {filteredProxyModels.length === 0 ? (
          <div className="proxyModelEmptyState">
            {proxyCopy.modelMenuSearchEmpty}
          </div>
        ) : (
          filteredProxyModels.map((model) => (
            <label key={model} className="proxyModelToggleRow">
              <strong className="proxyModelToggleName">{model}</strong>
              <span className="themeSwitch proxyModelToggleControl">
                <input
                  type="checkbox"
                  checked={!effectiveModelMenuDraft.includes(model)}
                  disabled={modelMenuSaving}
                  onChange={(event) =>
                    handleToggleProxyModel(model, event.target.checked)
                  }
                />
                <span className="themeSwitchTrack" aria-hidden="true">
                  <span className="themeSwitchThumb" />
                </span>
              </span>
            </label>
          ))
        )}
      </div>
    </UtilityDialog>
  );
}
