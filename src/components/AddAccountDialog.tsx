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
import type {
  AccountSummary,
  AuthJsonImportInput,
  CreateApiAccountInput,
  PreparedOauthLogin,
  TestApiAccountConnectionInput,
  TestApiAccountConnectionResult,
} from "../types/app";

type AddAccountRoute = "oauth" | "current" | "session" | "upload" | "api";

type AddAccountDialogProps = {
  open: boolean;
  reauthorizeAccount: AccountSummary | null;
  importingAccounts: boolean;
  oauthWaitingForCallback: boolean;
  onPrepareOauth: () => Promise<PreparedOauthLogin>;
  onOpenOauthPage: (url: string) => Promise<void>;
  onCompleteOauth: (callbackUrl: string) => Promise<void>;
  onCancelOauth: () => Promise<void>;
  onImportCurrentAuth: () => Promise<void>;
  onCreateApiAccount: (input: CreateApiAccountInput) => Promise<void>;
  onTestApiConnection: (
    input: TestApiAccountConnectionInput,
  ) => Promise<TestApiAccountConnectionResult>;
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
      <svg
        className="iconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M12 3a9 9 0 1 0 9 9" />
        <path d="M12 3v6l4 2" />
        <path d="M21 5v4h-4" />
      </svg>
    );
  }

  if (route === "current") {
    return (
      <svg
        className="iconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M12 4v16" />
        <path d="m7 9 5-5 5 5" />
        <path d="M5 19h14" />
      </svg>
    );
  }

  if (route === "api") {
    return (
      <svg
        className="iconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M4 8.5h16" />
        <path d="M4 15.5h16" />
        <path d="M7 4.5v15" />
        <path d="M17 4.5v15" />
      </svg>
    );
  }

  if (route === "session") {
    return (
      <svg
        className="iconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M9 3h6" />
        <path d="M10 3v4.5L5.8 17a3 3 0 0 0 2.7 4.2h7a3 3 0 0 0 2.7-4.2L14 7.5V3" />
        <path d="M8 14h8" />
      </svg>
    );
  }

  return (
    <svg
      className="iconGlyph"
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M12 16V4" />
      <path d="m7 11 5 5 5-5" />
      <path d="M5 20h14" />
    </svg>
  );
}

export function AddAccountDialog({
  open,
  reauthorizeAccount,
  importingAccounts,
  oauthWaitingForCallback,
  onPrepareOauth,
  onOpenOauthPage,
  onCompleteOauth,
  onCancelOauth,
  onImportCurrentAuth,
  onCreateApiAccount,
  onTestApiConnection,
  onImportFiles,
  onClose,
}: AddAccountDialogProps) {
  const { copy } = useI18n();
  const [activeRoute, setActiveRoute] = useState<AddAccountRoute>("oauth");
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
  const [sessionJsonText, setSessionJsonText] = useState("");
  const [readingFiles, setReadingFiles] = useState(false);
  const [pendingRoute, setPendingRoute] = useState<AddAccountRoute | null>(
    null,
  );
  const [preparingOauth, setPreparingOauth] = useState(false);
  const [oauthLogin, setOauthLogin] = useState<PreparedOauthLogin | null>(null);
  const [oauthCallbackUrl, setOauthCallbackUrl] = useState("");
  const [apiForm, setApiForm] = useState<CreateApiAccountInput>({
    label: "",
    baseUrl: "",
    apiKey: "",
    modelName: "",
    forceSave: false,
  });
  const [apiInlineError, setApiInlineError] = useState<string | null>(null);
  const [apiInlineSuccess, setApiInlineSuccess] = useState<string | null>(null);
  const [apiCanForceSave, setApiCanForceSave] = useState(false);
  const [testingApiConnection, setTestingApiConnection] = useState(false);
  const oauthAutoPrepareAttemptedRef = useRef(false);
  const oauthPrepareRequestRef = useRef(0);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);

  const busy = importingAccounts || readingFiles;
  const actionLocked = busy || preparingOauth;
  const closeBlocked = busy;
  const routeSwitchBlocked = busy || oauthWaitingForCallback;

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
      setSessionJsonText("");
      setReadingFiles(false);
      setPendingRoute(null);
      setApiForm({
        label: "",
        baseUrl: "",
        apiKey: "",
        modelName: "",
        forceSave: false,
      });
      setApiInlineError(null);
      setApiInlineSuccess(null);
      setApiCanForceSave(false);
      resetOauthState(!oauthWaitingForCallback);
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
  }, [closeBlocked, oauthWaitingForCallback, onClose, open, resetOauthState]);

  const routeOptions = useMemo(() => {
    const oauthRoute = {
      id: "oauth" as const,
      label: copy.addAccount.oauthTab,
      description: reauthorizeAccount
        ? copy.addAccount.reauthorizeOauthDescription
        : copy.addAccount.oauthDescription,
    };
    if (reauthorizeAccount) {
      return [oauthRoute];
    }
    return [
      oauthRoute,
      {
        id: "current" as const,
        label: copy.addAccount.currentTab,
        description: copy.addAccount.currentDescription,
      },
      {
        id: "session" as const,
        label: copy.addAccount.sessionTab,
        description: copy.addAccount.sessionDescription,
      },
      {
        id: "upload" as const,
        label: copy.addAccount.uploadTab,
        description: copy.addAccount.uploadDescription,
      },
      {
        id: "api" as const,
        label: copy.addAccount.apiTab,
        description: copy.addAccount.apiDescription,
      },
    ];
  }, [copy.addAccount, reauthorizeAccount]);

  const activeRouteMeta =
    routeOptions.find((item) => item.id === activeRoute) ?? routeOptions[0];
  const dialogTitle = reauthorizeAccount
    ? copy.addAccount.reauthorizeDialogTitle
    : copy.addAccount.dialogTitle;
  const dialogSubtitle = reauthorizeAccount
    ? copy.addAccount.reauthorizeDialogSubtitle(reauthorizeAccount.label)
    : copy.addAccount.dialogSubtitle;

  const selectedSummary = useMemo(() => {
    if (selectedFiles.length === 0) {
      return copy.addAccount.uploadNoJsonFiles;
    }

    const firstPath =
      selectedFiles[0]?.webkitRelativePath || selectedFiles[0]?.name || "";
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
  const apiRequiredMissing =
    apiForm.label.trim() === "" ||
    apiForm.baseUrl.trim() === "" ||
    apiForm.apiKey.trim() === "" ||
    apiForm.modelName.trim() === "";
  const apiSubmitDisabled =
    actionLocked || testingApiConnection || apiRequiredMissing;

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

    if (
      !oauthLogin &&
      !oauthWaitingForCallback &&
      oauthCallbackUrl.trim() === "" &&
      !preparingOauth
    ) {
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

    if (
      busy ||
      preparingOauth ||
      oauthLogin ||
      oauthAutoPrepareAttemptedRef.current
    ) {
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

  const handleImportSessionJson = async () => {
    const content = sessionJsonText.trim();
    if (actionLocked || content === "") {
      return;
    }

    setPendingRoute("session");
    try {
      await onImportFiles([
        {
          source: "pasted-session.json",
          content,
          label: null,
        },
      ]);
    } finally {
      setPendingRoute(null);
    }
  };

  const handleApiFieldChange =
    (field: keyof CreateApiAccountInput) =>
    (event: ChangeEvent<HTMLInputElement>) => {
      const value = event.target.value;
      setApiForm((current) => ({
        ...current,
        [field]: value,
        forceSave: false,
      }));
      setApiInlineError(null);
      setApiInlineSuccess(null);
      setApiCanForceSave(false);
    };

  const handleCreateApiAccount = async (forceSave: boolean) => {
    if (actionLocked) {
      return;
    }

    setPendingRoute("api");
    setApiInlineError(null);
    setApiInlineSuccess(null);
    try {
      await onCreateApiAccount({
        ...apiForm,
        forceSave,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setApiInlineError(message);
      setApiCanForceSave(!forceSave);
    } finally {
      setPendingRoute(null);
    }
  };

  const handleTestApiConnection = async () => {
    if (actionLocked || testingApiConnection || apiRequiredMissing) {
      return;
    }

    setTestingApiConnection(true);
    setApiInlineError(null);
    setApiInlineSuccess(null);
    setApiCanForceSave(false);
    try {
      const result = await onTestApiConnection({
        label: apiForm.label,
        baseUrl: apiForm.baseUrl,
        apiKey: apiForm.apiKey,
        modelName: apiForm.modelName,
      });
      const balance = result.balanceText ? ` · ${result.balanceText}` : "";
      setApiInlineSuccess(`${result.message}${balance}`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setApiInlineError(message);
    } finally {
      setTestingApiConnection(false);
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
            <h2>{dialogTitle}</h2>
            <p className="addAccountDialogSubtitle">{dialogSubtitle}</p>
          </div>
          <button
            type="button"
            className="iconButton ghost"
            onClick={onClose}
            title={copy.common.close}
            disabled={closeBlocked}
            aria-label={copy.common.close}
          >
            <svg
              className="iconGlyph"
              viewBox="0 0 24 24"
              aria-hidden="true"
              focusable="false"
            >
              <path d="m6 6 12 12" />
              <path d="M18 6 6 18" />
            </svg>
          </button>
        </div>

        <div className="addAccountWorkspace">
          <div
            className="addAccountTabs"
            aria-label={copy.addAccount.tabsAriaLabel}
          >
            {routeOptions.map((route) => {
              const active = route.id === activeRoute;
              return (
                <button
                  key={route.id}
                  type="button"
                  aria-pressed={active}
                  className={`addAccountTab${active ? " isActive" : ""}`}
                  onClick={() => setActiveRoute(route.id)}
                  disabled={routeSwitchBlocked}
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
                    <span className="addOauthListening">
                      {copy.addAccount.oauthListening}
                    </span>
                  ) : null}
                </div>

                <label className="addOauthField">
                  <span className="addOauthFieldLabel">
                    {copy.addAccount.oauthLinkLabel}
                  </span>
                  <input
                    className="addOauthInput addOauthReadonlyInput"
                    value={oauthLogin?.authUrl ?? ""}
                    readOnly
                  />
                </label>

                <label className="addOauthField">
                  <span className="addOauthFieldLabel">
                    {copy.addAccount.oauthCallbackLabel}
                  </span>
                  <textarea
                    className="addOauthTextarea"
                    value={oauthCallbackUrl}
                    onChange={(event) =>
                      setOauthCallbackUrl(event.target.value)
                    }
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
                    : reauthorizeAccount
                      ? copy.addAccount.reauthorizeParseCallback
                      : copy.addAccount.oauthParseCallback}
                </button>

                {!oauthLogin ? (
                  <div className="addOauthStatus">
                    <strong>{copy.addAccount.oauthPreparing}</strong>
                    <p>{activeRouteMeta.description}</p>
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
                        ? copy.addAccount.uploadSelectedCount(
                            selectedFiles.length,
                          )
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
                          <span className="addUploadFileIndex">
                            {index + 1}
                          </span>
                          <span className="addUploadFilePath">
                            {file.label}
                          </span>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <div className="addUploadEmptyState">
                      {copy.addAccount.uploadQueueEmpty}
                    </div>
                  )}
                </div>

                <button
                  type="button"
                  className="primary addAccountPrimaryAction"
                  onClick={() => void handleImportFiles()}
                  disabled={actionLocked || selectedFiles.length === 0}
                >
                  {pendingRoute === "upload" ||
                  importingAccounts ||
                  readingFiles
                    ? copy.addAccount.uploadImporting
                    : copy.addAccount.uploadStartImport}
                </button>
              </div>
            ) : null}

            {activeRoute === "session" ? (
              <div className="addAccountPanelBody addSessionSection">
                <label className="addOauthField">
                  <span className="addOauthFieldLabel">
                    {copy.addAccount.sessionJsonLabel}
                  </span>
                  <textarea
                    className="addOauthTextarea addSessionTextarea"
                    value={sessionJsonText}
                    onChange={(event) => setSessionJsonText(event.target.value)}
                    placeholder={copy.addAccount.sessionJsonPlaceholder}
                    rows={10}
                    spellCheck={false}
                  />
                </label>

                <button
                  type="button"
                  className="primary addAccountPrimaryAction"
                  onClick={() => void handleImportSessionJson()}
                  disabled={actionLocked || sessionJsonText.trim() === ""}
                >
                  {pendingRoute === "session" || importingAccounts
                    ? copy.addAccount.sessionImporting
                    : copy.addAccount.sessionStartImport}
                </button>
              </div>
            ) : null}

            {activeRoute === "api" ? (
              <div className="addAccountPanelBody addApiSection">
                <div className="addApiFieldGrid">
                  <label className="addOauthField">
                    <span className="addOauthFieldLabel">
                      {copy.addAccount.apiNameLabel}
                    </span>
                    <input
                      className="addOauthInput"
                      value={apiForm.label}
                      onChange={handleApiFieldChange("label")}
                      placeholder={copy.addAccount.apiNamePlaceholder}
                      spellCheck={false}
                    />
                  </label>

                  <label className="addOauthField">
                    <span className="addOauthFieldLabel">
                      {copy.addAccount.apiBaseUrlLabel}
                    </span>
                    <input
                      className="addOauthInput"
                      value={apiForm.baseUrl}
                      onChange={handleApiFieldChange("baseUrl")}
                      placeholder={copy.addAccount.apiBaseUrlPlaceholder}
                      spellCheck={false}
                    />
                    <span className="addFieldHint">
                      {copy.addAccount.apiBaseUrlHint}
                    </span>
                  </label>

                  <label className="addOauthField">
                    <span className="addOauthFieldLabel">
                      {copy.addAccount.apiKeyLabel}
                    </span>
                    <input
                      className="addOauthInput"
                      value={apiForm.apiKey}
                      onChange={handleApiFieldChange("apiKey")}
                      placeholder={copy.addAccount.apiKeyPlaceholder}
                      spellCheck={false}
                    />
                  </label>

                  <label className="addOauthField">
                    <span className="addOauthFieldLabel">
                      {copy.addAccount.apiModelLabel}
                    </span>
                    <input
                      className="addOauthInput"
                      value={apiForm.modelName}
                      onChange={handleApiFieldChange("modelName")}
                      placeholder={copy.addAccount.apiModelPlaceholder}
                      spellCheck={false}
                    />
                  </label>
                </div>

                {apiInlineError ? (
                  <div className="addApiErrorBox">
                    <strong>{copy.addAccount.apiValidationFailed}</strong>
                    <p>{apiInlineError}</p>
                  </div>
                ) : apiInlineSuccess ? (
                  <div className="addOauthStatus addApiStatus addApiSuccessBox">
                    <strong>{copy.addAccount.apiTestSucceeded}</strong>
                    <p>{apiInlineSuccess}</p>
                  </div>
                ) : (
                  <div className="addOauthStatus addApiStatus">
                    <strong>{copy.addAccount.apiValidationTitle}</strong>
                    <p>{copy.addAccount.apiValidationDescription}</p>
                  </div>
                )}

                <div className="addApiActionRow">
                  <button
                    type="button"
                    className="ghost addAccountSecondaryAction"
                    onClick={() => void handleTestApiConnection()}
                    disabled={
                      actionLocked || testingApiConnection || apiRequiredMissing
                    }
                  >
                    {testingApiConnection
                      ? copy.addAccount.apiTestingConnection
                      : copy.addAccount.apiTestConnection}
                  </button>
                  <button
                    type="button"
                    className="primary addAccountPrimaryAction"
                    onClick={() => void handleCreateApiAccount(false)}
                    disabled={apiSubmitDisabled}
                  >
                    {pendingRoute === "api"
                      ? copy.addAccount.apiSaving
                      : copy.addAccount.apiValidateAndSave}
                  </button>
                  {apiCanForceSave ? (
                    <button
                      type="button"
                      className="ghost addAccountSecondaryAction"
                      onClick={() => void handleCreateApiAccount(true)}
                      disabled={actionLocked}
                    >
                      {copy.addAccount.apiForceSave}
                    </button>
                  ) : null}
                </div>
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
