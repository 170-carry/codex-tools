import { useI18n } from "../i18n/I18nProvider";

export type MultiSelectOption<T extends string> = { id: T; label: string };

type EditorMultiSelectProps<T extends string> = {
  options: MultiSelectOption<T>[];
  value: T | null;
  disabled?: boolean;
  ariaLabel?: string;
  placeholder?: string;
  className?: string;
  onChange: (next: T) => void;
};

/** Native selection supplies keyboard navigation, typeahead and viewport-aware menus. */
export function EditorMultiSelect<T extends string>({ options, value, disabled, ariaLabel, placeholder, className = "", onChange }: EditorMultiSelectProps<T>) {
  const { copy } = useI18n();
  return (
    <div className={`editorPicker ${className}`}>
      <select
        className="editorPickerTrigger"
        value={value ?? ""}
        disabled={disabled}
        aria-label={ariaLabel ?? copy.editorPicker.ariaLabel}
        onChange={(event) => onChange(event.currentTarget.value as T)}
      >
        {value === null && <option value="" disabled>{placeholder ?? copy.editorPicker.placeholder}</option>}
        {options.map((option) => <option key={option.id} value={option.id}>{option.label}</option>)}
      </select>
    </div>
  );
}
