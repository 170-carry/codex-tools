type DebugFloatingToolProps = {
  onOpenUpdateDialog: () => void;
};

export function DebugFloatingTool({ onOpenUpdateDialog }: DebugFloatingToolProps) {
  if (!import.meta.env.DEV) {
    return null;
  }

  return (
    <aside className="debugFloatingTool" aria-label="Debug tools">
      <div className="debugFloatingHeader">
        <span>DEBUG</span>
        <strong>本地调试</strong>
      </div>
      <button type="button" className="ghost" onClick={onOpenUpdateDialog}>
        打开更新弹窗
      </button>
    </aside>
  );
}
