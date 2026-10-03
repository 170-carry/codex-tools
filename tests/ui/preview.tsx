import { createRoot } from "react-dom/client";
import { I18nProvider } from "../../src/i18n/I18nProvider";
import "../../src/index.css";
import App from "../../src/App";
import { installPreviewBackend } from "./mockBackend";

installPreviewBackend();
localStorage.setItem(
  "codex-tools-locale",
  new URLSearchParams(location.search).get("locale") ?? "zh-CN",
);
createRoot(document.getElementById("root")!).render(
  <I18nProvider>
    <App />
  </I18nProvider>,
);
