import { useSettingsWorkspace } from "./settings/useSettingsWorkspace";
import type { SettingsPanelProps } from "./settings/types";
import { GeneralSettings } from "./settings/GeneralSettings";
import { QuotaSettings } from "./settings/QuotaSettings";
import { SwitchingSettings } from "./settings/SwitchingSettings";
import { WarmupSettings } from "./settings/WarmupSettings";
import { AboutSettings } from "./settings/AboutSettings";
import { PageToolbar } from "./workspace/PageToolbar";
import { PageSections } from "./workspace/PageSections";
import { getPageLayoutCopy } from "../i18n/pageLayoutCopy";
import { useAppLayout } from "../hooks/useAppLayout";
import { ClassicSettings } from "./classic/ClassicSettings";

export function SettingsPanel(props: SettingsPanelProps) {
  const workspace = useSettingsWorkspace(props);
  const { layout } = useAppLayout();
  const text = getPageLayoutCopy(workspace.locale);
  if (layout === "classic") return <ClassicSettings workspace={workspace} />;
  return (
    <section
      className="settingsPage workspacePage"
      aria-label={workspace.copy.settings.title}
    >
      <PageToolbar
        title={workspace.copy.settings.title}
        detail={workspace.versionValue}
      />
      <PageSections
        label={workspace.copy.settings.title}
        sections={[
          {
            id: "general",
            label: text.general,
            content: <GeneralSettings workspace={workspace} />,
          },
          {
            id: "quota",
            label: text.quota,
            content: <QuotaSettings workspace={workspace} />,
          },
          {
            id: "switching",
            label: text.switching,
            content: <SwitchingSettings workspace={workspace} />,
          },
          {
            id: "warmup",
            label: text.warmup,
            content: <WarmupSettings workspace={workspace} />,
          },
          {
            id: "about",
            label: text.about,
            content: <AboutSettings workspace={workspace} />,
          },
        ]}
      />
    </section>
  );
}
