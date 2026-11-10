import { useI18n } from "../../i18n/I18nProvider";
import { uiCopy } from "../../i18n/uiCopy";
export function ImportMethods<T extends string>({ value, methods, disabled, onChange }: { value: T; methods: { id: T; label: string }[]; disabled: boolean; onChange: (value: T) => void }) {
  const { locale } = useI18n(); const text = uiCopy(locale);
  return <nav className="uiImportMethods" aria-label={text.importMethod}>{methods.map((method, index) => <div key={method.id}>{index === 0 ? <span className="uiMethodGroup">{text.recommended}</span> : index === 1 ? <span className="uiMethodGroup">{text.otherMethods}</span> : null}<button type="button" className="uiMethodButton" aria-pressed={value === method.id} disabled={disabled} onClick={() => onChange(method.id)}>{method.label}</button></div>)}</nav>;
}
