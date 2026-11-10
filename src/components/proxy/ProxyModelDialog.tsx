import { Button } from "../ui/Button";
import { SearchField } from "../ui/SearchField";
import { Switch } from "../ui/Switch";
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
      description={proxyCopy.modelMenuDescription}
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
          <Button busy={modelMenuSaving} variant="primary"
            type="button"
            className="primary"
            disabled={modelMenuSaving || !hasModelMenuChanges}
            onClick={() => void handleSaveProxyModels()}
          >
            {proxyCopy.modelMenuSave}
          </Button>
        </>
      }
    >
      <div className="proxyModelDialogToolbar">
        <div className="proxyModelSearchField">
          <span className="visuallyHidden">
            {proxyCopy.modelMenuSearchLabel}
          </span>
          <SearchField
            className="proxyModelSearchInput"
            type="search"
            aria-label={proxyCopy.modelMenuSearchLabel}
            value={modelSearchQuery}
            onValueChange={setModelSearchQuery}
            placeholder={proxyCopy.modelMenuSearchPlaceholder}
            spellCheck={false}
          />
        </div>
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
            <div key={model} className="proxyModelToggleRow">
              <strong className="proxyModelToggleName">{model}</strong>
              <Switch label={model} checked={!effectiveModelMenuDraft.includes(model)}
                disabled={modelMenuSaving} onChange={checked => handleToggleProxyModel(model, checked)} />
            </div>
          ))
        )}
      </div>
    </UtilityDialog>
  );
}
