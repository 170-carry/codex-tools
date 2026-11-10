import { Presence } from "./ui/Presence";
import type { ReactNode } from "react";
import { useI18n } from "../i18n/I18nProvider";
import type { LocalProjectSwitchPreview, Notice } from "../types/app";
import { formatLocalProjectText } from "../utils/localProjectSwitch";
import { UtilityDialog } from "./workspace/UtilityDialog";

export type LocalProjectMergeRequest = { label: string; preview: LocalProjectSwitchPreview };

export function LocalProjectMergeDialog({ request, onAnswer }: {
  request: LocalProjectMergeRequest | null;
  onAnswer: (accepted: boolean) => void;
}) {
  const { copy } = useI18n();
  const text = copy.localProjects;
  return <Presence present={!!request}>{request && <UtilityDialog
    title={text.title}
    description={formatLocalProjectText(text.description, { label: request.label })}
    closeLabel={copy.common.close}
    onClose={() => onAnswer(false)}
    actions={<>
      <button type="button" className="ghost" onClick={() => onAnswer(false)}>{text.cancel}</button>
      <button type="button" className="primary" onClick={() => onAnswer(true)}>{text.confirm}</button>
    </>}
  >
    <p>{text.mergeDetails}</p>
    {(["projectCount", "threadLinkCount", "unassignedThreadCount", "retainedTargetProjectCount"] as const).map((key) =>
      <div className="settingRow" key={key}><span>{text[key]}</span><strong>{request.preview[key]}</strong></div>,
    )}
  </UtilityDialog>}</Presence>;
}

export function LocalProjectRecoveryDialog({ open, onAnswer }: {
  open: boolean;
  onAnswer: (accepted: boolean) => void;
}) {
  const { copy } = useI18n();
  const text = copy.localProjects;
  return <Presence present={open}><UtilityDialog
    title={text.recoveryTitle}
    description={text.recoveryDescription}
    closeLabel={copy.common.close}
    onClose={() => onAnswer(false)}
    actions={<>
      <button type="button" className="ghost" onClick={() => onAnswer(false)}>{text.cancel}</button>
      <button type="button" className="primary" onClick={() => onAnswer(true)}>{text.recoveryConfirm}</button>
    </>}
  /></Presence>;
}

export function LocalProjectRecoveryGate({ blocked, confirmationOpen, onAnswer, children }: {
  blocked: boolean;
  confirmationOpen: boolean;
  onAnswer: (accepted: boolean) => void;
  children: ReactNode;
}) {
  return blocked
    ? <LocalProjectRecoveryDialog open={confirmationOpen} onAnswer={onAnswer} />
    : children;
}

export function LocalProjectRecoveryView({ recovering, onRecover, notice }: {
  recovering: boolean;
  onRecover: () => void;
  notice?: Notice | null;
}) {
  const { copy } = useI18n();
  const text = copy.localProjects;
  return <section className="settingsGroup" aria-labelledby="project-recovery-title">
    <h2 id="project-recovery-title">{text.recoveryTitle}</h2>
    <p>{text.recoveryDescription}</p>
    {notice ? <p role="status">{notice.message}</p> : null}
    <button type="button" className="primary" disabled={recovering} onClick={onRecover}>
      {recovering ? text.recovering : text.recoveryButton}
    </button>
  </section>;
}
