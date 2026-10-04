import { useAppLayout } from "../../hooks/useAppLayout";
import { getPageLayoutCopy } from "../../i18n/pageLayoutCopy";
import { copyText } from "./clipboard";
import { ProxyHelpTip } from "./ProxyHelpTip";
import {
  formatApiProxyKeyLogTime,
  summarizeApiProxyKeyLogs,
} from "./keyPolicy";
import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function ProxyKeys({ workspace }: { workspace: ApiProxyWorkspace }) {
  const { layout } = useAppLayout();
  const {
    apiProxyKeys,
    apiProxyKeyLogs,
    apiProxyKeysLoading,
    apiProxySupportedModels,
    savingApiProxyKey,
    onUpdateApiProxyKey,
    onDeleteApiProxyKey,
    onRegenerateApiProxyKey,
    locale,
    proxyCopy,
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
  } = workspace;

  return (
    <article className="proxyDetailCard proxyKeyManagerCard">
      <div className="proxyKeyManagerHeader">
        <div className="proxyKeyManagerIntro">
          <div className="proxyKeyTitleRow">
            <span className="proxyLabel">{proxyCopy.keyManagerTitle}</span>
            <ProxyHelpTip label={proxyCopy.keyManagerHelpLabel}>
              {proxyCopy.keyManagerHelp}
            </ProxyHelpTip>
          </div>
          <strong>{apiProxyKeys.length}</strong>
          <p>{proxyCopy.keyManagerDescription}</p>
        </div>
        <div className="proxyKeyCreateRow">
          <input
            className="proxyKeyInput"
            value={newApiProxyKeyLabel}
            onChange={(event) =>
              setNewApiProxyKeyLabel(event.currentTarget.value)
            }
            placeholder={proxyCopy.keyCreateNamePlaceholder}
            disabled={savingApiProxyKey}
          />
          <input
            className="proxyKeyInput proxyKeySecretInput"
            value={newApiProxyKeyValue}
            onChange={(event) =>
              setNewApiProxyKeyValue(event.currentTarget.value)
            }
            placeholder={proxyCopy.keyCreateSecretPlaceholder}
            disabled={savingApiProxyKey}
          />
          <button
            type="button"
            className="primary"
            disabled={savingApiProxyKey}
            onClick={() => void handleCreateApiProxyKey()}
          >
            {proxyCopy.keyCreateAction}
          </button>
        </div>
      </div>

      {apiProxyKeysLoading ? (
        <div className="proxyModelEmptyState">{proxyCopy.keyLoading}</div>
      ) : apiProxyKeys.length === 0 ? (
        <div className="proxyModelEmptyState">{proxyCopy.keyEmpty}</div>
      ) : (
        <div className="proxyKeyList">
          {apiProxyKeys.map((key) => {
            const summary = summarizeApiProxyKeyLogs(apiProxyKeyLogs, key.id);
            const recentLogs = apiProxyKeyLogs
              .filter((log) => log.keyId === key.id)
              .slice(0, 4);
            const boundModels =
              key.allowedModels.length === 0
                ? apiProxySupportedModels
                : key.allowedModels;
            return (
              <section key={key.id} className="proxyKeyItem">
                <div className="proxyKeyItemHeader">
                  <label className="proxyKeyNameField">
                    <span>{proxyCopy.keyNameLabel}</span>
                    <input
                      className="proxyKeyInput"
                      value={apiProxyKeyLabelDrafts[key.id] ?? key.label}
                      disabled={savingApiProxyKey}
                      onChange={(event) => {
                        const value = event.currentTarget.value;
                        setApiProxyKeyLabelDrafts((drafts) => ({
                          ...drafts,
                          [key.id]: value,
                        }));
                      }}
                      onBlur={() => commitApiProxyKeyLabel(key)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") {
                          event.currentTarget.blur();
                        }
                        if (event.key === "Escape") {
                          setApiProxyKeyLabelDrafts((drafts) => ({
                            ...drafts,
                            [key.id]: key.label,
                          }));
                          event.currentTarget.blur();
                        }
                      }}
                    />
                  </label>
                  <label
                    className="themeSwitch"
                    aria-label={proxyCopy.keyToggleAria}
                  >
                    <input
                      type="checkbox"
                      checked={key.enabled}
                      disabled={savingApiProxyKey}
                      onChange={(event) =>
                        void onUpdateApiProxyKey({
                          id: key.id,
                          enabled: event.currentTarget.checked,
                        })
                      }
                    />
                    <span className="themeSwitchTrack" aria-hidden="true">
                      <span className="themeSwitchThumb" />
                    </span>
                    <span className="themeSwitchText">
                      {key.enabled
                        ? proxyCopy.keyEnabled
                        : proxyCopy.keyDisabled}
                    </span>
                  </label>
                  <button
                    type="button"
                    className="ghost proxyCopyButton"
                    disabled={savingApiProxyKey}
                    onClick={() => void onRegenerateApiProxyKey(key.id)}
                  >
                    {proxyCopy.keyRegenerate}
                  </button>
                  <button
                    type="button"
                    className="danger"
                    disabled={savingApiProxyKey || apiProxyKeys.length <= 1}
                    onClick={() => void onDeleteApiProxyKey(key.id)}
                  >
                    {proxyCopy.keyDelete}
                  </button>
                </div>

                <div className="proxyKeySecretRow">
                  <span className="proxyInlineLabel">
                    {proxyCopy.keySecretLabel}
                  </span>
                  <code>{key.key}</code>
                  <button
                    type="button"
                    className="ghost proxyCopyButton"
                    onClick={() => copyText(key.key)}
                  >
                    {proxyCopy.copy}
                  </button>
                </div>

                <div className="proxyKeySummaryGrid">
                  <span>
                    <strong>{summary.calls}</strong>
                    {proxyCopy.keyCallsLabel}
                  </span>
                  <span>
                    <strong>{summary.tokens}</strong>
                    {proxyCopy.keyTokensLabel}
                  </span>
                  <span>
                    <strong>
                      {formatApiProxyKeyLogTime(locale, summary.lastUsedAt)}
                    </strong>
                    {proxyCopy.keyLastUsedLabel}
                  </span>
                </div>

                <details
                  className="proxyKeyDetails"
                  open={layout === "classic" ? true : undefined}
                >
                  <summary>{getPageLayoutCopy(locale).keyDetails}</summary>
                  <div>
                    <div className="proxyKeyBindingBlock">
                      <div className="proxySectionTitleWithHelp">
                        <span className="proxyInlineLabel">
                          {proxyCopy.keyModelsLabel}
                        </span>
                        <ProxyHelpTip label={proxyCopy.keyModelsHelpLabel}>
                          {proxyCopy.keyModelsHelp}
                        </ProxyHelpTip>
                      </div>
                      <div className="proxyKeyChipList">
                        {apiProxySupportedModels.map((model) => (
                          <label key={model} className="proxyKeyChip">
                            <input
                              type="checkbox"
                              checked={boundModels.includes(model)}
                              disabled={savingApiProxyKey}
                              onChange={(event) =>
                                updateApiProxyKeyModels(
                                  key,
                                  model,
                                  event.currentTarget.checked,
                                )
                              }
                            />
                            <span>{model}</span>
                          </label>
                        ))}
                      </div>
                    </div>

                    <div className="proxyKeyBindingColumns">
                      <div className="proxyKeyBindingBlock">
                        <div className="proxySectionTitleWithHelp">
                          <span className="proxyInlineLabel">
                            {proxyCopy.keyReasoningLabel}
                          </span>
                          <ProxyHelpTip label={proxyCopy.keyReasoningHelpLabel}>
                            {proxyCopy.keyReasoningHelp}
                          </ProxyHelpTip>
                        </div>
                        <div className="proxyKeyChipList">
                          {apiProxyReasoningOptions.map((option) => (
                            <label key={option.id} className="proxyKeyChip">
                              <input
                                type="checkbox"
                                checked={
                                  key.allowedReasoningEfforts.length === 0 ||
                                  key.allowedReasoningEfforts.includes(
                                    option.id,
                                  )
                                }
                                disabled={savingApiProxyKey}
                                onChange={(event) =>
                                  updateApiProxyKeyReasoning(
                                    key,
                                    option.id,
                                    event.currentTarget.checked,
                                  )
                                }
                              />
                              <span>{option.label}</span>
                            </label>
                          ))}
                        </div>
                      </div>
                      <div className="proxyKeyBindingBlock">
                        <div className="proxySectionTitleWithHelp">
                          <span className="proxyInlineLabel">
                            {proxyCopy.keyServiceTierLabel}
                          </span>
                          <ProxyHelpTip
                            label={proxyCopy.keyServiceTierHelpLabel}
                          >
                            {proxyCopy.keyServiceTierHelp}
                          </ProxyHelpTip>
                        </div>
                        <div className="proxyKeyChipList">
                          {apiProxyServiceTierOptions.map((option) => (
                            <label key={option.id} className="proxyKeyChip">
                              <input
                                type="checkbox"
                                checked={
                                  key.allowedServiceTiers.length === 0 ||
                                  key.allowedServiceTiers.includes(option.id)
                                }
                                disabled={savingApiProxyKey}
                                onChange={(event) =>
                                  updateApiProxyKeyServiceTier(
                                    key,
                                    option.id,
                                    event.currentTarget.checked,
                                  )
                                }
                              />
                              <span>{option.label}</span>
                            </label>
                          ))}
                        </div>
                      </div>
                    </div>

                    <div className="proxyKeyLogs">
                      <span className="proxyInlineLabel">
                        {proxyCopy.keyLogsLabel}
                      </span>
                      {recentLogs.length === 0 ? (
                        <p>{proxyCopy.keyNoLogs}</p>
                      ) : (
                        recentLogs.map((log) => (
                          <div
                            key={`${log.timestamp}-${log.route}-${log.model}-${log.calls}-${log.tokens}`}
                            className="proxyKeyLogRow"
                          >
                            <span>
                              {formatApiProxyKeyLogTime(locale, log.timestamp)}
                            </span>
                            <strong>{log.model}</strong>
                            <span>{log.route ?? "--"}</span>
                            <span>
                              {log.reasoningEffort ?? "--"} /{" "}
                              {log.serviceTier ?? "--"}
                            </span>
                            <span>
                              {log.calls} {proxyCopy.keyCallsLabel},{" "}
                              {log.tokens} {proxyCopy.keyTokensLabel}
                            </span>
                          </div>
                        ))
                      )}
                    </div>
                  </div>
                </details>
              </section>
            );
          })}
        </div>
      )}
    </article>
  );
}
