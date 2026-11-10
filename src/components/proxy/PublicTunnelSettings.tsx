import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function PublicTunnelSettings({workspace}: {workspace: ApiProxyWorkspace}) {
 const {tunnelMode,setTunnelMode,cloudflaredBusy,cloudflaredStatus,proxyCopy,namedInput,setNamedInput}=workspace;
 return <>              <div className="cloudflaredModeGrid">
                <button
                  className={`cloudflaredModeCard${tunnelMode === "quick" ? " isActive" : ""}`}
                  onClick={() => setTunnelMode("quick")}
                  disabled={cloudflaredBusy || cloudflaredStatus.running}
                >
                  <strong>{proxyCopy.quickModeLabel}</strong>
                  <p>{proxyCopy.quickModeDescription}</p>
                </button>
                <button
                  className={`cloudflaredModeCard${tunnelMode === "named" ? " isActive" : ""}`}
                  onClick={() => setTunnelMode("named")}
                  disabled={cloudflaredBusy || cloudflaredStatus.running}
                >
                  <strong>{proxyCopy.namedModeLabel}</strong>
                  <p>{proxyCopy.namedModeDescription}</p>
                </button>
              </div>

              {tunnelMode === "quick" ? (
                <article className="cloudflaredCallout">
                  <p>{proxyCopy.quickNoteBody}</p>
                </article>
              ) : null}

              {tunnelMode === "named" ? (
                <div className="cloudflaredFormGrid">
                  <label className="cloudflaredInputField">
                    <span>{proxyCopy.apiTokenLabel}</span>
                    <input
                      type="password"
                      value={namedInput.apiToken}
                      onChange={(event) =>
                        setNamedInput((current) => ({
                          ...current,
                          apiToken: event.target.value,
                        }))
                      }
                      placeholder={proxyCopy.apiTokenPlaceholder}
                      disabled={cloudflaredBusy || cloudflaredStatus.running}
                    />
                  </label>
                  <label className="cloudflaredInputField">
                    <span>{proxyCopy.accountIdLabel}</span>
                    <input
                      value={namedInput.accountId}
                      onChange={(event) =>
                        setNamedInput((current) => ({
                          ...current,
                          accountId: event.target.value,
                        }))
                      }
                      placeholder={proxyCopy.accountIdPlaceholder}
                      disabled={cloudflaredBusy || cloudflaredStatus.running}
                    />
                  </label>
                  <label className="cloudflaredInputField">
                    <span>{proxyCopy.zoneIdLabel}</span>
                    <input
                      value={namedInput.zoneId}
                      onChange={(event) =>
                        setNamedInput((current) => ({
                          ...current,
                          zoneId: event.target.value,
                        }))
                      }
                      placeholder={proxyCopy.zoneIdPlaceholder}
                      disabled={cloudflaredBusy || cloudflaredStatus.running}
                    />
                  </label>
                  <label className="cloudflaredInputField">
                    <span>{proxyCopy.hostnameLabel}</span>
                    <input
                      value={namedInput.hostname}
                      onChange={(event) =>
                        setNamedInput((current) => ({
                          ...current,
                          hostname: event.target.value,
                        }))
                      }
                      placeholder={proxyCopy.hostnamePlaceholder}
                      disabled={cloudflaredBusy || cloudflaredStatus.running}
                    />
                  </label>
                </div>
              ) : null}

</>;
}
