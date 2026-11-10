import { useRef, type InputHTMLAttributes } from "react";
import { useI18n } from "../../i18n/I18nProvider";
import { uiCopy } from "../../i18n/uiCopy";

export function SearchField({ value, onValueChange, className = "", ...props }: Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "onChange"> & { value: string; onValueChange: (value: string) => void }) {
  const input = useRef<HTMLInputElement>(null);
  const { locale } = useI18n();
  const clear = () => { onValueChange(""); input.current?.focus(); };
  return <div className={`uiSearchField ${className}`}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="10.5" cy="10.5" r="6.5" /><path d="m16 16 4 4" /></svg>
    <input {...props} ref={input} value={value} onChange={event => onValueChange(event.target.value)}
      onKeyDown={event => { if (event.key === "Escape" && value) { event.stopPropagation(); clear(); } props.onKeyDown?.(event); }} />
    {value && <button type="button" className="uiIconButton" aria-label={uiCopy(locale).clear} onClick={clear}>×</button>}
  </div>;
}
