import { EditorMultiSelect } from "../EditorMultiSelect";
import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function ProxyRouting({ workspace }: { workspace: ApiProxyWorkspace }) {
const { loadBalanceMode, apiProxySupportedModels, savingSettings, onUpdateLoadBalanceMode, proxyCopy, setSequentialLimitDraft, setModelMenuOpen, loadBalanceOptions, effectiveSequentialLimit, enabledModelCount, commitSequentialLimit } = workspace;
return <>
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

</>;
}
