import { ProxyKeys } from "./ProxyKeys";
import { ProxyModelDialog } from "./ProxyModelDialog";
import { EditorMultiSelect } from "../EditorMultiSelect";
import { copyText } from "./clipboard";
import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function ProxyConfiguration({
  workspace,
}: {
  workspace: ApiProxyWorkspace;
}) {
  const {
    status,
    loadBalanceMode,
    apiProxySupportedModels,
    savingSettings,
    refreshingApiKey,
    onRefreshApiKey,
    onUpdateLoadBalanceMode,
    proxyCopy,
    setSequentialLimitDraft,
    setModelMenuOpen,
    loadBalanceOptions,
    effectiveSequentialLimit,
    enabledModelCount,
    commitSequentialLimit,
  } = workspace;

  return (
    <div className="proxySectionContent">
      <article className="proxyDetailCard proxyBalanceCard">
        <div className="proxyBalanceHeader">
          <span className="proxyLabel">{proxyCopy.loadBalanceLabel}</span>
          <EditorMultiSelect
            className="proxyModePicker"
            options={loadBalanceOptions}
            value={loadBalanceMode}
            ariaLabel={proxyCopy.loadBalanceLabel}
            placeholder={proxyCopy.loadBalanceLabel}
            disabled={savingSettings}
            onChange={(mode) => {
              void onUpdateLoadBalanceMode(mode);
            }}
          />
        </div>

        {loadBalanceMode === "priority" ? <p>{proxyCopy.priorityDescription}</p> : null}
        {loadBalanceMode === "sequential" ? (
          <div className="proxySequentialLimit">
            <div className="proxySequentialLimitHeader">
              <span className="proxyInlineLabel">
                {proxyCopy.sequentialFiveHourLimitLabel}
              </span>
              <strong>{effectiveSequentialLimit}%</strong>
            </div>
            <input
              className="proxyRangeInput"
              type="range"
              min={0}
              max={100}
              step={1}
              value={effectiveSequentialLimit}
              disabled={savingSettings}
              aria-label={proxyCopy.sequentialFiveHourLimitLabel}
              aria-valuetext={`${effectiveSequentialLimit}%`}
              onChange={(event) => {
                setSequentialLimitDraft(Number(event.currentTarget.value));
              }}
              onPointerUp={(event) => {
                commitSequentialLimit(Number(event.currentTarget.value));
              }}
              onBlur={(event) => {
                commitSequentialLimit(Number(event.currentTarget.value));
              }}
            />
            <p>{proxyCopy.sequentialFiveHourLimitDescription}</p>
          </div>
        ) : null}
      </article>

      <article className="proxyDetailCard proxyModelCard">
        <div className="proxyModelCardHeader">
          <div className="proxyModelCardCopy">
            <span className="proxyLabel">{proxyCopy.modelMenuLabel}</span>
            <strong>
              {enabledModelCount}/{apiProxySupportedModels.length || 0}
            </strong>
            <p>{proxyCopy.modelMenuDescription}</p>
          </div>
          <button
            type="button"
            className="ghost proxySubmenuTrigger"
            disabled={savingSettings || apiProxySupportedModels.length === 0}
            onClick={() => setModelMenuOpen(true)}
          >
            <span>{proxyCopy.modelMenuOpen}</span>
          </button>
        </div>
      </article>

      <div className="proxyDetailGrid">
        <article className="proxyDetailCard">
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

        <article className="proxyDetailCard">
          <span className="proxyLabel">{proxyCopy.activeAccountLabel}</span>
          <strong>
            {status.activeAccountLabel ?? proxyCopy.activeAccountEmptyTitle}
          </strong>
          <p>
            {status.activeAccountId ?? proxyCopy.activeAccountEmptyDescription}
          </p>
        </article>

        <article className="proxyDetailCard">
          <span className="proxyLabel">{proxyCopy.lastErrorLabel}</span>
          <p className="proxyErrorText">{status.lastError ?? proxyCopy.none}</p>
        </article>
      </div>

      <ProxyKeys workspace={workspace} />

      <ProxyModelDialog workspace={workspace} />
    </div>
  );
}
