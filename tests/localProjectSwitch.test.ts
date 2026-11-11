import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import * as sharing from "../src/utils/localProjectSwitch.ts";
import type { LocalProjectSwitchPreview, SwitchAccountResult } from "../src/types/app.ts";

const preview: LocalProjectSwitchPreview = {
  projectCount: 24, threadLinkCount: 81, unassignedThreadCount: 4,
  retainedTargetProjectCount: 3, requiresMergeConfirmation: true,
  previewFingerprint: "synthetic-fingerprint", noOp: false,
};
const result = { accountId: "synthetic", localProjectPreservation: null } as SwitchAccountResult;
function setup(preserve = true) {
  const calls: unknown[] = [];
  return { calls, options: {
    preserve, sourceKind: "chatgpt" as const,
    preview: async () => { calls.push("preview"); return preview; },
    confirm: async (value: LocalProjectSwitchPreview) => { calls.push(value); return true; },
    switchAccount: async (args: Record<string, unknown>) => { calls.push(args); return result; },
  } };
}

test("default-off switches without preview, confirmation, or preservation parameters", async () => {
  const { calls, options } = setup(false);
  assert.equal(await sharing.createLocalProjectSwitchFlow()(options), result);
  assert.deepEqual(calls, [{}]);
});

test("default-off still allows relay switching", async () => {
  const { calls, options } = setup(false);
  assert.equal(await sharing.createLocalProjectSwitchFlow()({ ...options, sourceKind: "relay" }), result);
  assert.deepEqual(calls, [{}]);
});

test("merge cancellation does not call switch", async () => {
  const { calls, options } = setup();
  options.confirm = async () => false;
  assert.equal(await sharing.createLocalProjectSwitchFlow()(options), null);
  assert.deepEqual(calls, ["preview"]);
});

test("merge approval is bound to the exact reviewed fingerprint and counts", async () => {
  const { calls, options } = setup();
  await sharing.createLocalProjectSwitchFlow()(options);
  assert.deepEqual(calls, ["preview", preview, {
    acceptLocalProjectMerge: true, localProjectPreviewFingerprint: preview.previewFingerprint,
  }]);
});

test("nonempty confirmation is not required for an empty target but its fingerprint is still sent", async () => {
  const { calls, options } = setup();
  options.preview = async () => ({ ...preview, requiresMergeConfirmation: false });
  await sharing.createLocalProjectSwitchFlow()(options);
  assert.deepEqual(calls, [{ acceptLocalProjectMerge: false, localProjectPreviewFingerprint: preview.previewFingerprint }]);
});

test("the next attempt previews again instead of reusing a previous merge approval", async () => {
  const { calls, options } = setup();
  const flow = sharing.createLocalProjectSwitchFlow();
  await flow(options);
  const next = { ...preview, previewFingerprint: "synthetic-next-preview" };
  await flow({ ...options, preview: async () => next });
  assert.deepEqual(calls.slice(-2), [next, { acceptLocalProjectMerge: true, localProjectPreviewFingerprint: next.previewFingerprint }]);
});

test("a repeated click cannot start a second preview or switch while approval is pending", async () => {
  const { calls, options } = setup();
  let answer!: (value: boolean) => void;
  options.confirm = () => new Promise<boolean>((resolve) => { answer = resolve; });
  const flow = sharing.createLocalProjectSwitchFlow();
  const first = flow(options);
  await Promise.resolve();
  assert.equal(await flow(options), null);
  answer(true);
  await first;
  assert.equal(calls.length, 2);
});

test("unsupported session routes still switch without preview", async () => {
  const { calls, options } = setup();
  const flow = sharing.createLocalProjectSwitchFlow();
  assert.equal(await flow({ ...options, sourceKind: "relay" }), result);
  assert.deepEqual(calls, [{}]);
});

test("session preview failures still reach the account switch command", async () => {
  for (const code of ["PROJECT_SHARING_SYNC_FAILED", "PROJECT_SHARING_UNSUPPORTED_ROUTE", "PROJECT_SHARING_UNSUPPORTED_IDENTITY"]) {
    const { calls, options } = setup();
    assert.equal(await sharing.createLocalProjectSwitchFlow()({
      ...options, preview: async () => { throw new Error(code); },
    }), result);
    assert.deepEqual(calls, [{}]);
  }
});

test("credential, recovery and changed-preview errors still abort; failed flows release their lock", async () => {
  const { calls, options } = setup();
  const flow = sharing.createLocalProjectSwitchFlow();
  for (const code of ["PROJECT_SHARING_AUTH_REFRESH_FAILED", "PROJECT_SHARING_RECOVERY_REQUIRED", "PROJECT_SHARING_INVALID_STATE"]) {
    await assert.rejects(flow({ ...options, preview: async () => { throw new Error(code); } }), new RegExp(code));
  }
  await assert.rejects(flow({ ...options, preview: async () => { throw new Error("PROJECT_SHARING_PREVIEW_CHANGED"); } }), /PREVIEW_CHANGED/);
  assert.deepEqual(calls, []);
  assert.equal(await flow({ ...options, preserve: false }), result);
});

test("recovery cancellation performs no IPC or reload", async () => {
  const calls: string[] = [];
  assert.equal(await sharing.createLocalProjectRecoveryFlow()({
    confirm: async () => false,
    recover: async () => { calls.push("recover"); return true; },
    reload: async () => { calls.push("reload"); },
  }), null);
  assert.deepEqual(calls, []);
});

test("explicit recovery reloads only after recovery finishes, including when no journal remains", async () => {
  for (const recovered of [true, false]) {
    const calls: string[] = [];
    assert.equal(await sharing.createLocalProjectRecoveryFlow()({
      confirm: async () => { calls.push("confirm"); return true; },
      recover: async () => { calls.push("recover"); return recovered; },
      reload: async () => { calls.push("reload settings and accounts"); },
    }), recovered);
    assert.deepEqual(calls, ["confirm", "recover", "reload settings and accounts"]);
  }
});

test("recovery conflict never reloads or reports success", async () => {
  let reloaded = false;
  await assert.rejects(sharing.createLocalProjectRecoveryFlow()({
    confirm: async () => true,
    recover: async () => { throw new Error("PROJECT_SHARING_EXTERNAL_CHANGE"); },
    reload: async () => { reloaded = true; },
  }), /EXTERNAL_CHANGE/);
  assert.equal(reloaded, false);
});

test("a recovery reload failure cannot report completion", async () => {
  await assert.rejects(sharing.createLocalProjectRecoveryFlow()({
    confirm: async () => true,
    recover: async () => true,
    reload: async () => { throw new Error("PROJECT_SHARING_EXTERNAL_CHANGE"); },
  }), /EXTERNAL_CHANGE/);
});

test("a startup recovery error blocks ordinary IPC and permits only explicit recovery and reload", async () => {
  const calls: string[] = [];
  let required = 0;
  const gate = sharing.createProjectCommandGate(async <T>(command: string) => {
    calls.push(command);
    if (calls.length === 1) throw new Error("wrapped PROJECT_SHARING_RECOVERY_REQUIRED private-path");
    return true as T;
  }, () => { required++; });
  await assert.rejects(gate.invoke("list_accounts"), /RECOVERY_REQUIRED/);
  for (const command of ["switch_account_and_launch", "import_current_auth_account", "update_app_settings", "refresh_all_usage"])
    await assert.rejects(gate.invoke(command), /RECOVERY_REQUIRED/);
  assert.deepEqual(calls, ["list_accounts"]);
  assert.equal(required, 1);
  await gate.invoke("recover_local_project_switch");
  gate.allowRecoveryReload();
  await gate.invoke("get_app_settings");
  await gate.invoke("list_accounts");
  await assert.rejects(gate.invoke("update_app_settings"), /RECOVERY_REQUIRED/);
  gate.finishRecovery();
  await gate.invoke("update_app_settings");
  assert.deepEqual(calls, ["list_accounts", "recover_local_project_switch", "get_app_settings", "list_accounts", "update_app_settings"]);
});

test("all five locales format accurate statuses and hide raw preservation warnings and errors", async () => {
  for (const locale of ["zh-CN", "en-US", "ja-JP", "ko-KR", "ru-RU"]) {
    const raw = JSON.parse(await readFile(new URL(`../src/i18n/locales/${locale}.json`, import.meta.url), "utf8"));
    const copy = raw.localProjects;
    assert.ok(copy, `${locale} local project messages`);
    const base = { type: "ok" as const, message: "base" };
    assert.deepEqual(sharing.appendLocalProjectNotice(base, null, copy), base);
    assert.deepEqual(sharing.appendLocalProjectNotice(base, undefined, copy), base);
    for (const status of ["prepared", "verified", "pending", "noOp"] as const) {
      const notice = sharing.appendLocalProjectNotice(base, { ...preview, status, warning: null }, copy);
      assert.ok(!notice.message.includes("{{"));
      if (status !== "noOp") for (const n of [24, 81, 4]) assert.ok(notice.message.includes(String(n)));
      if (status !== "verified") assert.equal(notice.type, "info");
      if (status === "pending") assert.ok(!notice.message.includes(base.message), "pending must not retain a generic switch success/launch claim");
    }
    for (const code of sharing.PROJECT_SHARING_ERROR_CODES) {
      const safe = sharing.localizeProjectSharingError(`outer: ${code}: /private/secret.db auth=token`, copy);
      assert.ok(safe && !safe.includes("PROJECT_SHARING_") && !safe.includes("secret") && !safe.includes("token"));
    }
    assert.equal(sharing.localizeProjectSharingError("PROJECT_SHARING_FUTURE_ERROR /secret", copy), copy.errors.generic);
    assert.equal(sharing.localizeProjectSharingError("ordinary error", copy), null);
    const warning = sharing.appendLocalProjectNotice(base, { ...preview, status: "pending", warning: "/private/secret token" }, copy);
    assert.ok(!warning.message.includes("secret") && !warning.message.includes("token"));
    assert.equal(sharing.projectSharingErrorCode("wrapped PROJECT_SHARING_RECOVERY_REQUIRED: secret"), "PROJECT_SHARING_RECOVERY_REQUIRED");
  }
});

test("skipped sync keeps account-switch success and reports a warning in all locales", async () => {
  for (const locale of ["zh-CN", "en-US", "ja-JP", "ko-KR", "ru-RU"]) {
    const raw = JSON.parse(await readFile(new URL(`../src/i18n/locales/${locale}.json`, import.meta.url), "utf8"));
    const base = { type: "ok" as const, message: raw.notices.switchedOnly };
    const notice = sharing.appendLocalProjectNotice(base, {
      status: "skipped", projectCount: 0, threadLinkCount: 0, unassignedThreadCount: 0,
      warning: "PROJECT_SHARING_SYNC_FAILED /private/synthetic auth=token",
    }, raw.localProjects);
    assert.equal(notice.type, "info");
    assert.ok(notice.message.includes(base.message));
    assert.ok(notice.message.includes(raw.localProjects.statuses.skipped));
    assert.ok(notice.message.includes(raw.localProjects.errors.syncFailed));
    assert.doesNotMatch(notice.message, /PROJECT_SHARING_|auth=token|\/private/);
  }
});

test("all five pending notices describe unverified loading without recovery instructions or duplicate warnings", async () => {
  const unverified = {
    "zh-CN": "尚未确认 Codex 已加载",
    "en-US": "Codex loading has not yet been confirmed",
    "ja-JP": "Codex への読み込みはまだ確認できていません",
    "ko-KR": "Codex에 로드되었는지는 아직 확인되지 않았습니다",
    "ru-RU": "Загрузка в Codex ещё не подтверждена",
  };
  for (const [locale, expected] of Object.entries(unverified)) {
    const raw = JSON.parse(await readFile(new URL(`../src/i18n/locales/${locale}.json`, import.meta.url), "utf8"));
    const copy = raw.localProjects;
    const base = { type: "ok" as const, message: raw.notices.switchedAndLaunching };
    const pending = { ...preview, status: "pending" as const, warning: null };
    const plain = sharing.appendLocalProjectNotice(base, pending, copy);
    assert.equal(plain.type, "info");
    assert.ok(plain.message.includes(expected), locale);
    assert.ok(!plain.message.includes(base.message));
    assert.doesNotMatch(plain.message, /恢复入口|recovery|復旧|복구|восстановлени/i);
    const sameWarning = sharing.appendLocalProjectNotice(base, { ...pending, warning: "wrapped PROJECT_SHARING_RESTORE_PENDING /synthetic/private" }, copy);
    assert.deepEqual(sameWarning, plain, `${locale} does not repeat the same pending warning`);
    const independent = sharing.appendLocalProjectNotice(base, { ...pending, warning: "PROJECT_SHARING_LAUNCH_FAILED" }, copy);
    assert.equal(independent.message, `${plain.message} ${copy.errors.launchFailed}`);
    const pendingError = sharing.localizeProjectSharingError("PROJECT_SHARING_RESTORE_PENDING", copy)!;
    assert.doesNotMatch(pendingError, /恢复入口|recovery|復旧|복구|восстановлени/i);
  }
});

test("pending preserves OpenCode and editor failures in all locales without a generic switch success prefix", async () => {
  for (const locale of ["zh-CN", "en-US", "ja-JP", "ko-KR", "ru-RU"]) {
    const raw = JSON.parse(await readFile(new URL(`../src/i18n/locales/${locale}.json`, import.meta.url), "utf8"));
    const copy = raw.localProjects;
    const pending = { ...preview, status: "pending" as const, warning: "PROJECT_SHARING_RESTORE_PENDING" };
    for (const baseMessage of [raw.notices.switchedOnly, raw.notices.switchedAndLaunching, raw.notices.switchedAndLaunchByCli]) {
      const notice = sharing.composeLocalProjectSwitchNotice({ type: "ok", message: baseMessage }, pending, copy, (base) => {
        let message = base.message;
        for (const [template, error] of [
          [raw.notices.opencodeSyncFailed, "synthetic-opencode-sync-failure"],
          [raw.notices.opencodeDesktopRestartFailed, "synthetic-opencode-restart-failure"],
          [raw.notices.editorRestartFailed, "synthetic-editor-restart-failure"],
        ]) message = sharing.formatLocalProjectText(template, { base: message, error });
        return { type: "error", message };
      });
      assert.equal(notice.type, "error");
      for (const failure of ["synthetic-opencode-sync-failure", "synthetic-opencode-restart-failure", "synthetic-editor-restart-failure"])
        assert.ok(notice.message.includes(failure), `${locale} retains ${failure}`);
      for (const ordinary of [raw.notices.switchedOnly, raw.notices.switchedAndLaunching, raw.notices.switchedAndLaunchByCli])
        assert.ok(!notice.message.includes(ordinary), `${locale} suppresses ordinary switching success`);
      const pendingText = sharing.formatLocalProjectText(copy.statuses.pending, {
        projectCount: preview.projectCount, threadLinkCount: preview.threadLinkCount, unassignedThreadCount: preview.unassignedThreadCount,
      });
      assert.equal(notice.message.split(pendingText).length - 1, 1);
    }
  }
});
