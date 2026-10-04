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
    { label: text.quota, content: <QuotaSettings workspace={workspace} /> },
    {
      label: text.switching,
      content: <SwitchingSettings workspace={workspace} />,
    },
    { label: text.warmup, content: <WarmupSettings workspace={workspace} /> },
    { label: text.about, content: <AboutSettings workspace={workspace} /> },
  ];
  return (
    <section
      className="settingsPage classicSettingsPage"
      aria-label={workspace.copy.settings.title}
    >
      <header className="classicPageHeader">
        <h2>{workspace.copy.settings.title}</h2>
      </header>
      <div className="settingsShell">
        {groups.map((group) => (
          <section
            className="classicSettingsGroup"
            key={group.label}
            aria-label={group.label}
          >
            <h3 className="classicSectionTitle">{group.label}</h3>
            {group.content}
          </section>
        ))}
      </div>
    </section>
  );
}
