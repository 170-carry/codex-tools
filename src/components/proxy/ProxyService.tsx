import { PageHeader } from "../ui/PageFrame";
import { uiCopy } from "../../i18n/uiCopy";
import { Switch } from "../ui/Switch";
import { DEFAULT_PROXY_PORT } from "./constants";
import { copyText } from "./clipboard";
import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function ProxyService({ workspace }: { workspace: ApiProxyWorkspace }) {
  const {
    status,
    accountCount,
    autoStartEnabled,
    savingSettings,
    starting,
    stopping,
    bindingCodexProxy,
    restoringCodexProxy,
    onStop,
    onBindCodexProxy,
    onRestoreCodexProxy,
    onRefresh,
    onToggleAutoStart,
    proxyCopy,
    busy,
    setPortDraft,
    portInput,
    codexBindTitle,
    canBindCodexProxy,
    canRestoreCodexProxy,
    effectivePort,
    persistPortIfNeeded,
    handleStart,
  } = workspace;

  return (
    <section className="proxySectionCard proxySectionCardPrimary proxyLocalControlCard">
      <PageHeader title={uiCopy(workspace.locale).localService}
        context={
          <div className="proxyHeaderStats">
            <span className="proxyHeaderStat">
              <span
                className={`proxyStatusDot${status.running ? " isRunning" : ""}`}
                aria-hidden="true"
              />
              <span>{proxyCopy.statusLabel}</span>
              <strong>
                {status.running
                  ? proxyCopy.statusRunning
                  : proxyCopy.statusStopped}
              </strong>
            </span>
            <span className="proxyHeaderStat">
              <span>{proxyCopy.portLabel}</span>
              <strong>{status.port ?? "--"}</strong>
            </span>
            <span className="proxyHeaderStat">
              <span>{proxyCopy.accountCountLabel}</span>
              <strong>{accountCount}</strong>
            </span>
          </div>
        }
        actions={
        <div className="proxyControlActions">
          <button className="ghost" onClick={onRefresh} disabled={busy}>
            {proxyCopy.refreshStatus}
          </button>
          {status.running ? (
            <button className="danger" onClick={onStop} disabled={busy}>
              {stopping ? proxyCopy.stopping : proxyCopy.stop}
            </button>
          ) : (
            <button
              className="primary"
              onClick={() => {
                void handleStart();
              }}
              disabled={busy || accountCount === 0 || effectivePort === null}
            >
              {starting ? proxyCopy.starting : proxyCopy.start}
            </button>
          )}
        </div>
        }
      />

      <div className="proxyServiceFields">
        <div className="proxyControlRow">
          <label className="proxyCompactField">
            <span>{proxyCopy.portLabel}</span>
            <input
              className="proxyPortInput"
              inputMode="numeric"
              aria-label={proxyCopy.portInputAriaLabel}
              placeholder={DEFAULT_PROXY_PORT}
              value={portInput}
              onChange={(event) => setPortDraft(event.target.value)}
              onBlur={() => {
                void (async () => {
                  await persistPortIfNeeded();
                  if (effectivePort !== null) {
                    setPortDraft(null);
                  }
                })();
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.currentTarget.blur();
                }
              }}
              disabled={busy || status.running}
            />
          </label>

          <div className="proxySwitchRow proxyInlineSetting">
            <div className="settingMeta">
              <strong>{proxyCopy.defaultStartLabel}</strong>
            </div>
            <Switch label={proxyCopy.defaultStartLabel}
                checked={autoStartEnabled}
                disabled={savingSettings}
                onChange={(checked) => onToggleAutoStart(checked)} />
          </div>
        </div>

        <article className="proxyDetailCard proxyEndpointCard">
          <span className="proxyLabel">{proxyCopy.baseUrlLabel}</span>
          <div className="proxyEndpointList">
            <div className="proxyEndpointRow">
              <div className="proxyEndpointMeta">
                <span>{proxyCopy.localBaseUrlLabel}</span>
                <code>{status.baseUrl ?? proxyCopy.baseUrlPlaceholder}</code>
              </div>
              <button
                className="ghost proxyCopyButton"
                onClick={() => copyText(status.baseUrl)}
                disabled={!status.baseUrl}
              >
                {proxyCopy.copy}
              </button>
            </div>

            {status.lanBaseUrl ? (
              <div className="proxyEndpointRow">
                <div className="proxyEndpointMeta">
                  <span>{proxyCopy.lanBaseUrlLabel}</span>
                  <code>{status.lanBaseUrl}</code>
                </div>
                <button
                  className="ghost proxyCopyButton"
                  onClick={() => copyText(status.lanBaseUrl)}
                  disabled={!status.lanBaseUrl}
                >
                  {proxyCopy.copy}
                </button>
              </div>
            ) : null}
          </div>
        </article>
      </div>

      {status.lastError ? (
        <p className="proxyServiceError" role="alert">
          {status.lastError}
        </p>
      ) : null}
      <details className="proxyClientDisclosure">
        <summary>
          {proxyCopy.codexBindLabel}
          <span>{codexBindTitle}</span>
        </summary>
        <article
          className={`proxyDetailCard proxyCodexBindCard${status.codexProxyBound ? " isBound" : ""}`}
        >
          <div className="proxyDetailHeader">
            <div className="proxyCodexBindMeta">
              <span className="proxyLabel">{proxyCopy.codexBindLabel}</span>
              <strong>{codexBindTitle}</strong>
            </div>
            <div className="proxyDetailActions">
              <button
                type="button"
                className="ghost proxyCopyButton"
                disabled={!canRestoreCodexProxy}
                onClick={onRestoreCodexProxy}
              >
                {restoringCodexProxy
                  ? proxyCopy.codexRestoreActionBusy
                  : proxyCopy.codexRestoreAction}
              </button>
              <button
                type="button"
                className="primary"
                disabled={!canBindCodexProxy}
                onClick={onBindCodexProxy}
              >
                {bindingCodexProxy
                  ? proxyCopy.codexBindActionBusy
                  : status.codexProxyBound
                    ? proxyCopy.codexRefreshModelsAction
                    : proxyCopy.codexBindAction}
              </button>
            </div>
          </div>
          <p className="proxyCatalogHint">{proxyCopy.codexModelsRestartHint}</p>
          <div className="proxyEndpointList">
            <div className="proxyEndpointRow">
              <div className="proxyEndpointMeta">
                <span>{proxyCopy.codexBindCurrentBaseUrlLabel}</span>
                <code>{status.codexProxyBaseUrl ?? proxyCopy.none}</code>
              </div>
              <button
                className="ghost proxyCopyButton"
                onClick={() => copyText(status.codexProxyBaseUrl)}
                disabled={!status.codexProxyBaseUrl}
              >
                {proxyCopy.copy}
              </button>
            </div>
          </div>
        </article>
      </details>
    </section>
  );
}
