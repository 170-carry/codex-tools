import { useState, type InputHTMLAttributes } from "react";
import { useI18n } from "../../../i18n/I18nProvider";
import { getAccountImportCopy } from "./copy";

export function SecretInput(
  props: Omit<InputHTMLAttributes<HTMLInputElement>, "type">,
) {
  const [visible, setVisible] = useState(false);
  const { locale } = useI18n();
  const text = getAccountImportCopy(locale);
  return (
    <span className="secretInput">
      <input
        {...props}
        type={visible ? "text" : "password"}
        autoComplete="off"
      />
      <button
        type="button"
        className="secretVisibilityButton"
        onClick={() => setVisible(!visible)}
        aria-label={visible ? text.hideKey : text.showKey}
        aria-pressed={visible}
      >
        {visible ? text.hide : text.show}
      </button>
    </span>
  );
}
