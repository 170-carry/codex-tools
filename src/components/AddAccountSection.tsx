type AddAccountSectionProps = {
  startingAdd: boolean;
  addFlowActive: boolean;
  onStartAddAccount: () => void;
  onSmartSwitch: () => void;
  smartSwitching: boolean;
};

export function AddAccountSection({
  startingAdd,
  addFlowActive,
  onStartAddAccount,
  onSmartSwitch,
  smartSwitching,
}: AddAccountSectionProps) {
  return (
    <section className="importBar">
      <div className="importInfo">
        <button
          className="smartSwitchButton importSmartSwitch"
          onClick={onSmartSwitch}
          disabled={smartSwitching}
          title="智能切换"
          aria-label="智能切换"
        >
          智能切换
        </button>
      </div>
      <div className="importRow">
        <button
          className="primary"
          onClick={onStartAddAccount}
          disabled={startingAdd || addFlowActive}
        >
          {startingAdd ? "启动中..." : addFlowActive ? "等待授权中..." : "添加账号"}
        </button>
      </div>
    </section>
  );
}
