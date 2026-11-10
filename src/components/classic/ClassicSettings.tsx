import { PageFrame, SectionGroup } from "../ui/PageFrame";
import type { SettingsWorkspace } from "../settings/useSettingsWorkspace";
import { GeneralSettings } from "../settings/GeneralSettings";
import { QuotaSettings } from "../settings/QuotaSettings";
import { SwitchingSettings } from "../settings/SwitchingSettings";
import { WarmupSettings } from "../settings/WarmupSettings";
import { AboutSettings } from "../settings/AboutSettings";
import { getPageLayoutCopy } from "../../i18n/pageLayoutCopy";

export function ClassicSettings({
  workspace,
}: {
  workspace: SettingsWorkspace;
}) {
  const text = getPageLayoutCopy(workspace.locale);
  const groups = [
    { label: text.general, content: <GeneralSettings workspace={workspace} /> },
    { label: `Codex · ${text.quota}`, content: <QuotaSettings workspace={workspace} /> },
    {
      label: `Codex · ${text.switching}`,
      content: <SwitchingSettings workspace={workspace} />,
    },
    { label: `Codex · ${text.warmup}`, content: <WarmupSettings workspace={workspace} /> },
    { label: text.about, content: <AboutSettings workspace={workspace} /> },
  ];
  return (
    <PageFrame kind="settings" className="settingsPage classicSettingsPage">
      <div className="settingsShell">
        {groups.map((group) => (
          <SectionGroup title={group.label} key={group.label} className="classicSettingsGroup">
            {group.content}
          </SectionGroup>
        ))}
      </div>
    </PageFrame>
  );
}
