import { EditorMultiSelect } from "../EditorMultiSelect";
import {
  DEFAULT_REMOTE_SSH_PORT,
  DEFAULT_REMOTE_LISTEN_PORT,
} from "./constants";
import type { ApiProxyWorkspace } from "./useApiProxyWorkspace";
export function RemoteEditor({ workspace }: { workspace: ApiProxyWorkspace }) {
  const {
    onPickLocalIdentityFile,
    copy,
    proxyCopy,
    remoteAuthOptions,
    setEditingRemoteId,
    effectiveRemoteDrafts,
    selectedRemoteDraft,
    selectedRemoteIdentity,
    selectedInstallingDependency,
    selectedRemoteBusy,
    editingSelectedRemote,
    persistRemoteDrafts,
    updateRemoteDraft,
    removeRemoteDraft,
  } = workspace;
  if (!selectedRemoteDraft) return null;
  return (
    <>
      {editingSelectedRemote ? (
        <div className="remoteWorkbenchSection">
          <div className="remoteWorkbenchSectionHeader">
            <div>
              <span className="proxyLabel">{proxyCopy.remoteConfigTitle}</span>
              <strong>{selectedRemoteIdentity}</strong>
            </div>
            <div className="remoteWorkbenchSectionActions">
              <button
                className="ghost"
                onClick={() => {
                  persistRemoteDrafts(effectiveRemoteDrafts);
                  setEditingRemoteId(null);
                }}
                disabled={selectedRemoteBusy}
              >
                {proxyCopy.remoteSave}
              </button>
              <button
                className="ghost"
                onClick={() => removeRemoteDraft(selectedRemoteDraft.id)}
                disabled={selectedRemoteBusy}
              >
                {proxyCopy.remoteRemove}
              </button>
            </div>
          </div>

          <div className="remoteServerPanel">
            <div className="remoteServerGrid">
              <label className="remoteServerField">
                <span>{proxyCopy.remoteNameLabel}</span>
                <input
                  value={selectedRemoteDraft.label}
                  onChange={(event) =>
                    updateRemoteDraft(
                      selectedRemoteDraft.id,
                      "label",
                      event.target.value,
                    )
                  }
                  placeholder="tokyo-01"
                />
              </label>
              <label className="remoteServerField">
                <span>{proxyCopy.remoteHostLabel}</span>
                <input
                  value={selectedRemoteDraft.host}
                  onChange={(event) =>
                    updateRemoteDraft(
                      selectedRemoteDraft.id,
                      "host",
                      event.target.value,
                    )
                  }
                  placeholder="1.2.3.4"
                />
              </label>
              <label className="remoteServerField">
                <span>{proxyCopy.remoteSshPortLabel}</span>
                <input
                  inputMode="numeric"
                  value={selectedRemoteDraft.sshPort}
                  onChange={(event) =>
                    updateRemoteDraft(
                      selectedRemoteDraft.id,
                      "sshPort",
                      event.target.value,
                    )
                  }
                  placeholder={DEFAULT_REMOTE_SSH_PORT}
                />
              </label>
              <label className="remoteServerField">
                <span>{proxyCopy.remoteUserLabel}</span>
                <input
                  value={selectedRemoteDraft.sshUser}
                  onChange={(event) =>
                    updateRemoteDraft(
                      selectedRemoteDraft.id,
                      "sshUser",
                      event.target.value,
                    )
                  }
                  placeholder="root"
                />
              </label>
              <label className="remoteServerField">
                <span>{proxyCopy.remoteDirLabel}</span>
                <input
                  value={selectedRemoteDraft.remoteDir}
                  onChange={(event) =>
                    updateRemoteDraft(
                      selectedRemoteDraft.id,
                      "remoteDir",
                      event.target.value,
                    )
                  }
                  placeholder="/opt/codex-tools"
                />
              </label>
              <label className="remoteServerField">
                <span>{proxyCopy.remoteListenPortLabel}</span>
                <input
                  inputMode="numeric"
                  value={selectedRemoteDraft.listenPort}
                  onChange={(event) =>
                    updateRemoteDraft(
                      selectedRemoteDraft.id,
                      "listenPort",
                      event.target.value,
                    )
                  }
                  placeholder={DEFAULT_REMOTE_LISTEN_PORT}
                />
              </label>
            </div>
          </div>

          <div className="remoteServerPanel">
            <div className="remoteAuthRow">
              <label className="remoteServerField remoteAuthSelectField">
                <span>{proxyCopy.remoteAuthLabel}</span>
                <EditorMultiSelect
                  className="remoteAuthPicker"
                  ariaLabel={proxyCopy.remoteAuthLabel}
                  options={remoteAuthOptions}
                  value={selectedRemoteDraft.authMode}
                  onChange={(next) =>
                    updateRemoteDraft(selectedRemoteDraft.id, "authMode", next)
                  }
                />
              </label>

              <div className="remoteAuthInputArea">
                {selectedRemoteDraft.authMode === "keyContent" ? (
                  <label className="remoteServerField">
                    <span>{proxyCopy.remotePrivateKeyLabel}</span>
                    <textarea
                      className="remoteServerTextarea"
                      value={selectedRemoteDraft.privateKey}
                      onChange={(event) =>
                        updateRemoteDraft(
                          selectedRemoteDraft.id,
                          "privateKey",
                          event.target.value,
                        )
                      }
                      placeholder={proxyCopy.remotePrivateKeyPlaceholder}
                    />
                  </label>
                ) : null}

                {selectedRemoteDraft.authMode === "password" ? (
                  <label className="remoteServerField">
                    <span>{proxyCopy.remotePasswordLabel}</span>
                    <input
                      type="password"
                      value={selectedRemoteDraft.password}
                      onChange={(event) =>
                        updateRemoteDraft(
                          selectedRemoteDraft.id,
                          "password",
                          event.target.value,
                        )
                      }
                      placeholder={proxyCopy.remotePasswordPlaceholder}
                    />
                  </label>
                ) : null}

                {selectedRemoteDraft.authMode === "keyFile" ||
                selectedRemoteDraft.authMode === "keyPath" ? (
                  <div className="remoteIdentityRow">
                    <label className="remoteServerField">
                      <span>{proxyCopy.remoteIdentityFileLabel}</span>
                      <input
                        value={selectedRemoteDraft.identityFile}
                        onChange={(event) =>
                          updateRemoteDraft(
                            selectedRemoteDraft.id,
                            "identityFile",
                            event.target.value,
                          )
                        }
                        placeholder={proxyCopy.remoteIdentityFilePlaceholder}
                      />
                    </label>
                    {selectedRemoteDraft.authMode === "keyFile" ? (
                      <button
                        className="ghost"
                        type="button"
                        onClick={() => {
                          void onPickLocalIdentityFile().then((value) => {
                            if (value) {
                              updateRemoteDraft(
                                selectedRemoteDraft.id,
                                "identityFile",
                                value,
                              );
                            }
                          });
                        }}
                      >
                        {proxyCopy.remotePickIdentityFile}
                      </button>
                    ) : null}
                  </div>
                ) : null}
              </div>
            </div>
          </div>

          {selectedInstallingDependency ? (
            <div
              className="remoteDependencyInstall"
              role="status"
              aria-live="polite"
              aria-busy="true"
            >
              <div className="remoteDependencyInstallHeader">
                <strong>{copy.notices.installingDependency("sshpass")}</strong>
              </div>
              <div className="remoteDependencyInstallTrack" aria-hidden="true">
                <span className="remoteDependencyInstallFill" />
              </div>
            </div>
          ) : null}
        </div>
      ) : null}
    </>
  );
}
