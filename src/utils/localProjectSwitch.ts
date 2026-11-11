import type {
  AccountSourceKind, LocalProjectPreservation, LocalProjectSwitchPreview,
  Notice, SwitchAccountResult,
} from "../types/app.ts";

const errorKeys = {
  PROJECT_SHARING_AUTH_REFRESH_FAILED: "authRefreshFailed",
  PROJECT_SHARING_BUSY: "busy",
  PROJECT_SHARING_CONFIRMATION_REQUIRED: "confirmationRequired",
  PROJECT_SHARING_DATABASE_BUSY: "databaseBusy",
  PROJECT_SHARING_DATABASE_MISMATCH: "databaseMismatch",
  PROJECT_SHARING_DESKTOP_RUNNING: "desktopRunning",
  PROJECT_SHARING_DESKTOP_UNAVAILABLE: "desktopUnavailable",
  PROJECT_SHARING_DISABLED: "disabled",
  PROJECT_SHARING_EXTERNAL_CHANGE: "externalChange",
  PROJECT_SHARING_IDENTITY_CHANGED: "identityChanged",
  PROJECT_SHARING_INVALID_STATE: "invalidState",
  PROJECT_SHARING_IO: "io",
  PROJECT_SHARING_LAUNCH_FAILED: "launchFailed",
  PROJECT_SHARING_PREVIEW_CHANGED: "previewChanged",
  PROJECT_SHARING_RECOVERY_REQUIRED: "recoveryRequired",
  PROJECT_SHARING_RESTORE_PENDING: "restorePending",
  PROJECT_SHARING_STALE_STORE: "staleStore",
  PROJECT_SHARING_SYNC_FAILED: "syncFailed",
  PROJECT_SHARING_UNSUPPORTED_HOME: "unsupportedHome",
  PROJECT_SHARING_UNSUPPORTED_IDENTITY: "unsupportedIdentity",
  PROJECT_SHARING_UNSUPPORTED_PLATFORM: "unsupportedPlatform",
  PROJECT_SHARING_UNSUPPORTED_ROUTE: "unsupportedRoute",
} as const;
export const PROJECT_SHARING_ERROR_CODES = Object.keys(errorKeys);

export type LocalProjectCopy = {
  title: string; description: string; mergeDetails: string;
  projectCount: string; threadLinkCount: string; unassignedThreadCount: string;
  retainedTargetProjectCount: string; cancel: string; confirm: string;
  recoveryTitle: string; recoveryDescription: string; recoveryButton: string;
  recoveryConfirm: string; recovering: string; recovered: string; noRecovery: string;
  statuses: Record<LocalProjectPreservation["status"], string>;
  errors: Record<(typeof errorKeys)[keyof typeof errorKeys] | "generic", string>;
};

export function projectSharingErrorCode(raw: string): string | null {
  return raw.match(/PROJECT_SHARING_[A-Z_]+/)?.[0] ?? null;
}

export function localizeProjectSharingError(raw: string, copy: LocalProjectCopy): string | null {
  const code = projectSharingErrorCode(raw);
  if (!code) return null;
  const key = errorKeys[code as keyof typeof errorKeys];
  return key ? copy.errors[key] : copy.errors.generic;
}

export function formatLocalProjectText(template: string, values: Record<string, string | number>): string {
  return template.replace(/\{\{\s*(\w+)\s*\}\}/g, (_, key: string) => String(values[key] ?? ""));
}

export function appendLocalProjectNotice(
  base: Notice,
  preservation: LocalProjectPreservation | null | undefined,
  copy: LocalProjectCopy,
): Notice {
  if (!preservation) return base;
  const status = formatLocalProjectText(copy.statuses[preservation.status], {
    projectCount: preservation.projectCount,
    threadLinkCount: preservation.threadLinkCount,
    unassignedThreadCount: preservation.unassignedThreadCount,
  });
  const repeatsPending = preservation.status === "pending" &&
    projectSharingErrorCode(preservation.warning ?? "") === "PROJECT_SHARING_RESTORE_PENDING";
  const warning = preservation.warning && !repeatsPending
    ? localizeProjectSharingError(preservation.warning, copy) ?? copy.errors.generic
    : null;
  return {
    type: base.type === "error" ? "error"
      : preservation.status === "verified" && !warning ? base.type : "info",
    message: [preservation.status === "pending" ? null : base.message, status, warning].filter(Boolean).join(" "),
  };
}

type PreservationArguments = {
  acceptLocalProjectMerge?: boolean;
  localProjectPreviewFingerprint?: string;
};

export function composeLocalProjectSwitchNotice(
  base: Notice,
  preservation: LocalProjectPreservation | null | undefined,
  copy: LocalProjectCopy,
  appendCompanions: (base: Notice) => Notice,
): Notice {
  // Pending replaces only the ordinary switch/launch claim. Independent
  // companion results are appended afterwards so their failures remain visible.
  if (preservation?.status === "pending") {
    return appendCompanions(appendLocalProjectNotice(base, preservation, copy));
  }
  return appendLocalProjectNotice(appendCompanions(base), preservation, copy);
}

type SwitchFlowOptions = {
  preserve: boolean;
  sourceKind: AccountSourceKind;
  preview: () => Promise<LocalProjectSwitchPreview>;
  confirm: (preview: LocalProjectSwitchPreview) => Promise<boolean>;
  switchAccount: (args: PreservationArguments) => Promise<SwitchAccountResult>;
};

export function createLocalProjectSwitchFlow() {
  let busy = false;
  return async (options: SwitchFlowOptions): Promise<SwitchAccountResult | null> => {
    if (busy) return null;
    busy = true;
    try {
      const args: PreservationArguments = {};
      if (options.preserve && options.sourceKind === "chatgpt") {
        let preview: LocalProjectSwitchPreview | undefined;
        try {
          preview = await options.preview();
        } catch (error) {
          const code = projectSharingErrorCode(String(error));
          if (code !== "PROJECT_SHARING_SYNC_FAILED" &&
              code !== "PROJECT_SHARING_UNSUPPORTED_IDENTITY" &&
              code !== "PROJECT_SHARING_UNSUPPORTED_ROUTE") throw error;
        }
        if (preview) {
          if (preview.requiresMergeConfirmation && !await options.confirm(preview)) return null;
          args.acceptLocalProjectMerge = preview.requiresMergeConfirmation;
          args.localProjectPreviewFingerprint = preview.previewFingerprint;
        }
      }
      return await options.switchAccount(args);
    } finally {
      busy = false;
    }
  };
}

export function createLocalProjectRecoveryFlow() {
  let busy = false;
  return async (options: {
    confirm: () => Promise<boolean>;
    recover: () => Promise<boolean>;
    reload: () => Promise<void>;
  }): Promise<boolean | null> => {
    if (busy) return null;
    busy = true;
    try {
      if (!await options.confirm()) return null;
      const recovered = await options.recover();
      await options.reload();
      return recovered;
    } finally {
      busy = false;
    }
  };
}

export type ProjectCommandInvoker = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export function createProjectCommandGate(
  invoke: ProjectCommandInvoker,
  onRecoveryRequired: () => void,
) {
  let required = false;
  let reloading = false;
  const requireRecovery = () => {
    reloading = false;
    if (!required) { required = true; onRecoveryRequired(); }
  };
  return {
    requireRecovery,
    allowRecoveryReload: () => { reloading = true; },
    finishRecovery: () => { required = false; reloading = false; },
    invoke: async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
      const recoveryRead = reloading && (command === "get_app_settings" || command === "list_accounts");
      if (required && command !== "recover_local_project_switch" && !recoveryRead)
        throw new Error("PROJECT_SHARING_RECOVERY_REQUIRED");
      try {
        return await invoke<T>(command, args);
      } catch (error) {
        if (projectSharingErrorCode(String(error)) === "PROJECT_SHARING_RECOVERY_REQUIRED") requireRecovery();
        throw error;
      }
    },
  };
}
