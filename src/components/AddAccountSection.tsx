import { useI18n } from "../i18n/I18nProvider";

type AddAccountSectionProps = {
  onOpenAddDialog: () => void;
  onSmartSwitch: () => void;
  smartSwitching: boolean;
};

function SparkIcon() {
  return (
    <svg className="buttonIcon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 3v4" />
      <path d="M12 17v4" />
      <path d="M3 12h4" />
      <path d="M17 12h4" />
      <path d="m5.6 5.6 2.8 2.8" />
      <path d="m15.6 15.6 2.8 2.8" />
      <path d="m18.4 5.6-2.8 2.8" />
      <path d="m8.4 15.6-2.8 2.8" />
    </svg>
  );
}

function PlusIcon() {
  return (
    <svg className="buttonIcon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 5v14" />
      <path d="M5 12h14" />
    </svg>
  );
}

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
        <SparkIcon />
        {copy.addAccount.smartSwitch}
      </button>
      <button
        className="primary importPrimary"
        onClick={onOpenAddDialog}
      >
        <PlusIcon />
        {copy.addAccount.startButton}
      </button>
    </section>
  );
}
