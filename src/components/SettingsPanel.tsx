import type { SettingsPanelProps } from "./settings/types";
import { useSettingsWorkspace } from "./settings/useSettingsWorkspace";
import { ClassicSettings } from "./classic/ClassicSettings";

export function SettingsPanel(props: SettingsPanelProps) {
  const workspace = useSettingsWorkspace(props);
  return <ClassicSettings workspace={workspace} />;
}
