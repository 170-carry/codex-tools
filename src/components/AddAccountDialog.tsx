import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type InputHTMLAttributes,
} from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n/I18nProvider";
import type { AuthJsonImportInput, PreparedOauthLogin } from "../types/app";

type AddAccountRoute = "oauth" | "current" | "upload";

type AddAccountDialogProps = {
  open: boolean;
  importingAccounts: boolean;
  oauthWaitingForCallback: boolean;
  onPrepareOauth: () => Promise<PreparedOauthLogin>;
  onOpenOauthPage: (url: string) => Promise<void>;
  onCompleteOauth: (callbackUrl: string) => Promise<void>;
  onCancelOauth: () => Promise<void>;
  onImportCurrentAuth: () => Promise<void>;
  onImportFiles: (items: AuthJsonImportInput[]) => Promise<void>;
  onClose: () => void;
};

const folderPickerAttributes = {
  webkitdirectory: "",
  directory: "",
} as unknown as InputHTMLAttributes<HTMLInputElement>;

function AddAccountRouteIcon({ route }: { route: AddAccountRoute }) {
  if (route === "oauth") {
    return (
      <svg className="iconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M12 3a9 9 0 1 0 9 9" />
        <path d="M12 3v6l4 2" />
        <path d="M21 5v4h-4" />
      </svg>
    );
  }

  if (route === "current") {
    return (
      <svg className="iconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M12 4v16" />
        <path d="m7 9 5-5 5 5" />
        <path d="M5 19h14" />
      </svg>
    );
  }

  return (
    <svg className="iconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 16V4" />
      <path d="m7 11 5 5 5-5" />
      <path d="M5 20h14" />
    </svg>
  );
}

export function AddAccountDialog({
  open,
  importingAccounts,
  oauthWaitingForCallback,
  onPrepareOauth,
  onOpenOauthPage,
  onCompleteOauth,
  onCancelOauth,
  onImportCurrentAuth,
  onImportFiles,
  onClose,
}: AddAccountDialogProps) {
  const { copy } = useI18n();
  const [activeRoute, setActiveRoute] = useState<AddAccountRoute>("oauth");
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
  const [readingFiles, setReadingFiles] = useState(false);
  const [pendingRoute, setPendingRoute] = useState<AddAccountRoute | null>(null);
  const [preparingOauth, setPreparingOauth] = useState(false);
  const [oauthLogin, setOauthLogin] = useState<PreparedOauthLogin | null>(null);
  const [oauthCallbackUrl, setOauthCallbackUrl] = useState("");
  const oauthAutoPrepareAttemptedRef = useRef(false);
  const oauthPrepareRequestRef = useRef(0);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);

  const busy = importingAccounts || readingFiles;
  const actionLocked = busy || preparingOauth;
  const closeBlocked = busy;

  const resetOauthState = useCallback(
    (cancelRemote: boolean) => {
      oauthAutoPrepareAttemptedRef.current = false;
      oauthPrepareRequestRef.current += 1;
      setPreparingOauth(false);
      setOauthLogin(null);
      setOauthCallbackUrl("");
      if (cancelRemote) {
        void onCancelOauth();
      }
    },
    [onCancelOauth],
  );

  useEffect(() => {
    if (!open) {
      setActiveRoute("oauth");
      setSelectedFiles([]);
      setReadingFiles(false);
      setPendingRoute(null);
      resetOauthState(true);
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !closeBlocked) {
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [closeBlocked, onClose, open, resetOauthState]);

  const routeOptions = useMemo(
    () => [
      {
        id: "oauth" as const,
        label: copy.addAccount.oauthTab,
        description: copy.addAccount.oauthDescription,
      },
      {
        id: "current" as const,
        label: copy.addAccount.currentTab,
        description: copy.addAccount.currentDescription,
      },
      {
        id: "upload" as const,
        label: copy.addAccount.uploadTab,
        description: copy.addAccount.uploadDescription,
      },
    ],
    [copy.addAccount],
  );

  const activeRouteMeta = routeOptions.find((item) => item.id === activeRoute) ?? routeOptions[0];

  const selectedSummary = useMemo(() => {
    if (selectedFiles.length === 0) {
      return copy.addAccount.uploadNoJsonFiles;
    }

    const firstPath = selectedFiles[0]?.webkitRelativePath || selectedFiles[0]?.name || "";
    if (selectedFiles.length === 1) {
      return firstPath;
    }

    return copy.addAccount.uploadFileSummary(firstPath, selectedFiles.length);
  }, [copy.addAccount, selectedFiles]);

  const selectedPreview = useMemo(
    () =>
      selectedFiles.slice(0, 4).map((file) => ({
        key: file.webkitRelativePath || file.name,
        label: file.webkitRelativePath || file.name,
      })),
    [selectedFiles],
  );

  const handlePrepareOauth = useCallback(async () => {
    if (busy || preparingOauth) {
      return;
    }

    const requestId = oauthPrepareRequestRef.current + 1;
    oauthPrepareRequestRef.current = requestId;
    setPreparingOauth(true);
    try {
      const prepared = await onPrepareOauth();
      if (oauthPrepareRequestRef.current !== requestId) {
        return;
      }
      setOauthLogin(prepared);
      setOauthCallbackUrl("");
    } finally {
      if (oauthPrepareRequestRef.current === requestId) {
        setPreparingOauth(false);
      }
    }
  }, [busy, onPrepareOauth, preparingOauth]);

  useEffect(() => {
    if (!open || activeRoute === "oauth") {
      return;
    }

    if (!oauthLogin && !oauthWaitingForCallback && oauthCallbackUrl.trim() === "" && !preparingOauth) {
      return;
    }

    resetOauthState(true);
  }, [
    activeRoute,
    oauthCallbackUrl,
    oauthLogin,
    oauthWaitingForCallback,
    open,
    preparingOauth,
    resetOauthState,
  ]);

  useEffect(() => {
    if (!open) {
      oauthAutoPrepareAttemptedRef.current = false;
      return;
    }

    if (activeRoute !== "oauth") {
      oauthAutoPrepareAttemptedRef.current = false;
      return;
    }

    if (busy || preparingOauth || oauthLogin || oauthAutoPrepareAttemptedRef.current) {
      return;
    }

    oauthAutoPrepareAttemptedRef.current = true;
    void handlePrepareOauth().catch(() => {});
  }, [activeRoute, busy, handlePrepareOauth, oauthLogin, open, preparingOauth]);

  if (!open) {
    return null;
  }

  const mergeSelectedFiles = (incomingFiles: File[]) => {
    setSelectedFiles((current) => {
      const nextMap = new Map<string, File>();
      for (const file of current) {
        const key = file.webkitRelativePath || file.name;
        nextMap.set(key, file);
      }
      for (const file of incomingFiles) {
        const key = file.webkitRelativePath || file.name;
        nextMap.set(key, file);
      }
      return Array.from(nextMap.entries())
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([, file]) => file);
    });
  };

  const handleFilesPicked = (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.currentTarget.files ?? []).filter((file) =>
      file.name.toLowerCase().endsWith(".json"),
    );
    if (files.length > 0) {
      mergeSelectedFiles(files);
    }
    event.currentTarget.value = "";
  };

  const handleCompleteOauth = async () => {
    if (actionLocked || oauthCallbackUrl.trim() === "") {
      return;
    }

    setPendingRoute("oauth");
    try {
      await onCompleteOauth(oauthCallbackUrl.trim());
    } finally {
      setPendingRoute(null);
    }
  };

  const handleOpenOauthPage = async () => {
    if (!oauthLogin || actionLocked) {
      return;
    }
    await onOpenOauthPage(oauthLogin.authUrl);
  };

  const handleImportCurrentAuth = async () => {
    if (actionLocked) {
      return;
    }

    setPendingRoute("current");
    try {
      await onImportCurrentAuth();
    } finally {
      setPendingRoute(null);
    }
  };

  const handleImportFiles = async () => {
    if (actionLocked || selectedFiles.length === 0) {
      return;
    }

    setPendingRoute("upload");
    setReadingFiles(true);
    try {
      const items = await Promise.all(
        selectedFiles.map(async (file) => ({
          source: file.webkitRelativePath || file.name,
          content: await file.text(),
          label: null,
        })),
      );
      await onImportFiles(items);
    } finally {
      setReadingFiles(false);
      setPendingRoute(null);
    }
  };

  return createPortal(
    <div
      className="settingsOverlay"
      onClick={() => {
        if (!closeBlocked) {
          onClose();
        }
      }}
    >
      <section
        className="settingsDialog addAuthDialog"
        role="dialog"
        aria-modal="true"
        aria-label={copy.addAccount.dialogAriaLabel}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="settingsHeader">
          <div>
            <h2>{copy.addAccount.dialogTitle}</h2>
            <p className="addAccountDialogSubtitle">{copy.addAccount.dialogSubtitle}</p>
          </div>
          <button
            type="button"
            className="iconButton ghost"
            onClick={onClose}
            title={copy.common.close}
            disabled={closeBlocked}
            aria-label={copy.common.close}
          >
            <svg className="iconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <path d="m6 6 12 12" />
              <path d="M18 6 6 18" />
            </svg>
          </button>
        </div>

        <div className="addAccountWorkspace">
          <div className="addAccountTabs" aria-label={copy.addAccount.tabsAriaLabel}>
            {routeOptions.map((route) => {
              const active = route.id === activeRoute;
              return (
                <button
                  key={route.id}
                  type="button"
                  aria-pressed={active}
                  className={`addAccountTab${active ? " isActive" : ""}`}
                  onClick={() => setActiveRoute(route.id)}
                  disabled={busy}
                >
                  <span className="addAccountTabIcon">
                    <AddAccountRouteIcon route={route.id} />
                  </span>
                  <span className="addAccountTabContent">
                    <strong>{route.label}</strong>
                    <span>{route.description}</span>
                  </span>
                </button>
              );
            })}
          </div>

          <div className="addAccountPanel">
            <div className="addAccountPanelHead">
              <span className="addAccountPanelIcon">
                <AddAccountRouteIcon route={activeRoute} />
              </span>
              <div className="addAccountPanelCopy">
                <h3>{activeRouteMeta.label}</h3>
                <p>{activeRouteMeta.description}</p>
              </div>
            </div>

            {activeRoute === "oauth" ? (
              <div className="addAccountPanelBody addOauthSection">
                <div className="addOauthActionRow">
                  <button
                    type="button"
                    className="primary addAccountPrimaryAction"
                    onClick={() => void handleOpenOauthPage()}
                    disabled={actionLocked || !oauthLogin}
                  >
                    {copy.addAccount.oauthOpenBrowser}
                  </button>
                  {oauthWaitingForCallback ? (
                    <span className="addOauthListening">{copy.addAccount.oauthListening}</span>
                  ) : null}
                </div>

                <label className="addOauthField">
                  <span className="addOauthFieldLabel">{copy.addAccount.oauthLinkLabel}</span>
                  <input
                    className="addOauthInput addOauthReadonlyInput"
                    value={oauthLogin?.authUrl ?? ""}
                    readOnly
                  />
                </label>

                <label className="addOauthField">
                  <span className="addOauthFieldLabel">{copy.addAccount.oauthCallbackLabel}</span>
                  <textarea
                    className="addOauthTextarea"
                    value={oauthCallbackUrl}
                    onChange={(event) => setOauthCallbackUrl(event.target.value)}
                    placeholder={copy.addAccount.oauthCallbackPlaceholder}
                    rows={4}
                    spellCheck={false}
                  />
                </label>

                <button
                  type="button"
                  className="primary addAccountPrimaryAction"
                  onClick={() => void handleCompleteOauth()}
                  disabled={actionLocked || oauthCallbackUrl.trim() === ""}
                >
                  {pendingRoute === "oauth" || importingAccounts
                    ? copy.addAccount.oauthCallbackSubmitting
                    : copy.addAccount.oauthParseCallback}
                </button>

                {!oauthLogin ? (
                  <div className="addOauthStatus">
                    <strong>{copy.addAccount.oauthPreparing}</strong>
                    <p>{copy.addAccount.oauthDescription}</p>
                  </div>
                ) : null}
              </div>
            ) : null}

            {activeRoute === "current" ? (
              <div className="addAccountPanelBody addCurrentSection">
                <div className="addCurrentSummary">
                  <span className="addInlineBadge">AUTH.JSON</span>
                  <p>{copy.addAccount.currentDescription}</p>
                </div>
                <button
                  type="button"
                  className="primary addAccountPrimaryAction"
                  onClick={() => void handleImportCurrentAuth()}
                  disabled={actionLocked}
                >
                  {pendingRoute === "current"
                    ? copy.addAccount.currentImporting
                    : copy.addAccount.currentStart}
                </button>
              </div>
            ) : null}

            {activeRoute === "upload" ? (
              <div className="addAccountPanelBody addUploadSection">
                <div className="addUploadPickerGrid">
                  <button
                    type="button"
                    className="ghost"
                    onClick={() => fileInputRef.current?.click()}
                    disabled={actionLocked}
                  >
                    {copy.addAccount.uploadChooseFiles}
                  </button>
                  <button
                    type="button"
                    className="ghost"
                    onClick={() => folderInputRef.current?.click()}
                    disabled={actionLocked}
                  >
                    {copy.addAccount.uploadChooseFolder}
                  </button>
                </div>

                <div className="addUploadQueue">
                  <div className="addUploadQueueHeader">
                    <strong>
                      {selectedFiles.length > 0
                        ? copy.addAccount.uploadSelectedCount(selectedFiles.length)
                        : copy.addAccount.uploadQueueTitle}
                    </strong>
                    <p>
                      {selectedFiles.length > 0
                        ? selectedSummary
                        : copy.addAccount.uploadQueueEmpty}
                    </p>
                  </div>

                  {selectedPreview.length > 0 ? (
                    <ul className="addUploadFileList">
                      {selectedPreview.map((file, index) => (
                        <li key={file.key} className="addUploadFileItem">
                          <span className="addUploadFileIndex">{index + 1}</span>
                          <span className="addUploadFilePath">{file.label}</span>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <div className="addUploadEmptyState">{copy.addAccount.uploadQueueEmpty}</div>
                  )}
                </div>

                <button
                  type="button"
                  className="primary addAccountPrimaryAction"
                  onClick={() => void handleImportFiles()}
                  disabled={actionLocked || selectedFiles.length === 0}
                >
                  {pendingRoute === "upload" || importingAccounts || readingFiles
                    ? copy.addAccount.uploadImporting
                    : copy.addAccount.uploadStartImport}
                </button>
              </div>
            ) : null}

            <input
              ref={fileInputRef}
              className="visuallyHidden"
              type="file"
              multiple
              accept=".json,application/json"
              onChange={handleFilesPicked}
            />
            <input
              ref={folderInputRef}
              className="visuallyHidden"
              type="file"
              multiple
              accept=".json,application/json"
              onChange={handleFilesPicked}
              {...folderPickerAttributes}
            />
          </div>
        </div>
      </section>
    </div>,
    document.body,
  );
}
