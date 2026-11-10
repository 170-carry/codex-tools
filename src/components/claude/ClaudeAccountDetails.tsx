import { claudeText } from "../../i18n/claudeCopy";
import { useI18n } from "../../i18n/I18nProvider";
import { Inspector } from "../ui/Inspector";
import { SectionGroup } from "../ui/PageFrame";
import type { ClaudeAccount } from "./types";
export function ClaudeAccountDetails({ account: a, onClose }: { account: ClaudeAccount; onClose: () => void }) {
  const { locale } = useI18n();
  const ct = claudeText(locale);
  return <Inspector title={a.label} onClose={onClose}><div className="uiDetailContent"><p>{a.email}</p><SectionGroup title={ct("Authorization")}><dl className="uiDefinitionList"><div><dt>{ct("Client")}</dt><dd>{a.kind === "desktop" ? "Claude Desktop" : a.kind === "api" ? "API" : "Claude Code"}</dd></div><div><dt>{ct("Renewal")}</dt><dd>{a.sync?.needsAuth ? (ct("Authorization required")) : a.canRefresh ? (ct("Automatic renewal enabled")) : (ct("Check authorization for renewal"))}</dd></div><div><dt>{ct("Updated")}</dt><dd>{a.sync?.lastUsageAt ? new Date(a.sync.lastUsageAt * 1000).toLocaleString(locale) : "—"}</dd></div></dl></SectionGroup>{a.baseUrl && <SectionGroup title="Base URL"><code>{a.baseUrl}</code></SectionGroup>}{a.sync?.error && <p role="alert" className="uiInlineError">{a.sync.error}</p>}</div></Inspector>;
}
