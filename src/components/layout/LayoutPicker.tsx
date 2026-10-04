import { useAppLayout } from "../../hooks/useAppLayout";
import { useI18n } from "../../i18n/I18nProvider";
import { getLayoutCopy } from "../../i18n/layoutCopy";
import { parseAppLayout } from "../../utils/layoutPreference";

export function LayoutPicker({ settings = false }: { settings?: boolean }) {
  const { layout, setLayout } = useAppLayout();
  const { locale } = useI18n();
  const text = getLayoutCopy(locale);
  const control = (
    <select
      className="layoutPicker"
      aria-label={text.label}
      value={layout}
      onChange={(event) => setLayout(parseAppLayout(event.currentTarget.value))}
    >
      <option value="classic">{text.classic}</option>
      <option value="compact">{text.compact}</option>
    </select>
  );
  return settings ? (
    <div className="settingRow layoutSettingRow">
      <div className="settingMeta">
        <strong>{text.label}</strong>
        <span>{text.description}</span>
      </div>
      {control}
    </div>
  ) : (
    control
  );
}
