import { useI18n } from "../i18n/I18nProvider";
import { WorkspaceIcon } from "./workspace/WorkspaceIcon";

type AddAccountSectionProps = {
  onOpenAddDialog: () => void;
  onSmartSwitch: () => void;
  smartSwitching: boolean;
  compact?: boolean;
};

export function AddAccountSection({
  onOpenAddDialog,
  onSmartSwitch,
  smartSwitching,
  compact = false,
}: AddAccountSectionProps) {
  const { copy } = useI18n();
  return (
    <section className="importBar">
      <button
        type="button"
        className={
          compact
            ? "toolbarIconButton smartSwitchButton"
            : "ghost importSmartSwitch"
        }
        onClick={onSmartSwitch}
        disabled={smartSwitching}
        title={copy.addAccount.smartSwitch}
        aria-label={copy.addAccount.smartSwitch}
      >
        <WorkspaceIcon name="switch" spinning={smartSwitching} />
        <span className={compact ? "visuallyHidden" : undefined}>
          {copy.addAccount.smartSwitch}
        </span>
      </button>
      <button
        type="button"
        className={
          compact
            ? "toolbarIconButton addAccountButton"
            : "primary importPrimary"
        }
        onClick={onOpenAddDialog}
        title={`${copy.addAccount.startButton} · ⌘N`}
        aria-label={copy.addAccount.startButton}
      >
        <WorkspaceIcon name="plus" />
        <span className={compact ? "visuallyHidden" : undefined}>
          {copy.addAccount.startButton}
        </span>
      </button>
    </section>
  );
}
