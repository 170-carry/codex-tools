import { useI18n } from "../i18n/I18nProvider";

type AddAccountSectionProps = {
  onOpenAddDialog: () => void;
  onSmartSwitch: () => void;
  smartSwitching: boolean;
};

export function AddAccountSection({
  onOpenAddDialog,
  onSmartSwitch,
  smartSwitching,
}: AddAccountSectionProps) {
  const { copy } = useI18n();

  return (
    <section className="importBar">
      <button
        className="ghost smartSwitchButton importSmartSwitch"
        onClick={onSmartSwitch}
        disabled={smartSwitching}
        title={copy.addAccount.smartSwitch}
        aria-label={copy.addAccount.smartSwitch}
      >
        {copy.addAccount.smartSwitch}
      </button>
      <button
        className="primary importPrimary"
        onClick={onOpenAddDialog}
      >
        {copy.addAccount.startButton}
      </button>
    </section>
  );
}
