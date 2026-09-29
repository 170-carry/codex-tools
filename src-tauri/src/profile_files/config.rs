//! Account-specific routing layered over shared desktop settings.
use super::*;

pub(super) fn build_chatgpt_profile_config(current_config: Option<&str>) -> String {
    let mut document = parse_config_or_default(current_config);
    let had_base_url = document.get("openai_base_url").is_some();
    document.remove("openai_base_url");
    if let Some(providers) = document
        .get_mut("model_providers")
        .and_then(toml_edit::Item::as_table_mut)
    {
        providers.remove(RELAY_PROVIDER);
    }
    if had_base_url {
        document.remove("model");
    }
    // Codex filters local history by model_provider; account switching should
    // keep official login history on the built-in provider key.
    document["model_provider"] = value("openai");
    // Codex Tools switches accounts by atomically replacing CODEX_HOME/auth.json.
    // `auto` may prefer a stale macOS Keychain entry in newer desktop builds,
    // so managed profiles must explicitly read the file that was just applied.
    document["cli_auth_credentials_store"] = value(MANAGED_AUTH_CREDENTIALS_STORE);
    document.to_string()
}

pub(super) fn build_relay_profile_config(
    current_config: Option<&str>,
    base_url: &str,
    model_name: &str,
) -> String {
    let mut document = parse_config_or_default(current_config);
    document["openai_base_url"] = value(base_url);
    document["model"] = value(model_name);
    configure_relay_provider(&mut document, base_url);
    document["cli_auth_credentials_store"] = value(MANAGED_AUTH_CREDENTIALS_STORE);
    document.to_string()
}

pub(super) fn build_codex_proxy_config(current_config: Option<&str>, base_url: &str) -> String {
    let mut document = parse_config_or_default(current_config);
    document["openai_base_url"] = value(base_url);
    document["model_provider"] = value("openai");
    document["cli_auth_credentials_store"] = value(MANAGED_AUTH_CREDENTIALS_STORE);
    set_missing_string_default(&mut document, "model", DEFAULT_API_PROXY_MODEL);
    set_missing_string_default(
        &mut document,
        "model_reasoning_effort",
        DEFAULT_API_PROXY_REASONING_EFFORT,
    );
    set_missing_string_default(
        &mut document,
        "service_tier",
        DEFAULT_API_PROXY_SERVICE_TIER,
    );
    document.to_string()
}

pub(super) fn merge_shared_config(
    profile_config: Option<&str>,
    current_config: Option<&str>,
) -> Option<String> {
    // Codex configuration is machine-local shared state. Use the current
    // configuration as the whole source of truth so changed and deleted
    // settings (including approval/sandbox preferences) survive a switch.
    // The target account builder reapplies only its required provider route
    // and the managed credential-store setting afterwards.
    current_config
        .and_then(|raw| raw.parse::<DocumentMut>().ok())
        .map(|document| document.to_string())
        .or_else(|| profile_config.map(str::to_string))
}

const RELAY_PROVIDER: &str = "codex_tools_relay";

fn configure_relay_provider(document: &mut DocumentMut, base_url: &str) {
    // Preserve the legacy route for existing OpenAI sessions. New sessions use
    // a provider which explicitly advertises HTTP Responses only.
    document["model_provider"] = value(RELAY_PROVIDER);
    let mut provider = toml_edit::Table::new();
    provider["name"] = value("Codex Tools Relay");
    provider["base_url"] = value(base_url);
    provider["wire_api"] = value("responses");
    provider["requires_openai_auth"] = value(true);
    provider["supports_websockets"] = value(false);
    document["model_providers"][RELAY_PROVIDER] = toml_edit::Item::Table(provider);
}

pub(crate) fn current_config_matches_account(account: &StoredAccount) -> bool {
    current_codex_config_path()
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|raw| raw.parse::<DocumentMut>().ok())
        .is_some_and(|config| {
            config
                .get("cli_auth_credentials_store")
                .and_then(toml_edit::Item::as_str)
                == Some(MANAGED_AUTH_CREDENTIALS_STORE)
                && (!matches!(account.source_kind, AccountSourceKind::Relay)
                    || (config
                        .get("openai_base_url")
                        .and_then(toml_edit::Item::as_str)
                        == account.api_base_url.as_deref()
                        && config.get("model").and_then(toml_edit::Item::as_str)
                            == account.model_name.as_deref()
                        && config
                            .get("model_provider")
                            .and_then(toml_edit::Item::as_str)
                            == Some(RELAY_PROVIDER)))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relay_uses_http_provider_and_retains_legacy_session_route() {
        let config = build_relay_profile_config(
            Some("[model_providers.user_provider]\nname = \"untouched\"\n"),
            "https://relay.invalid/v1",
            "gpt-6-sol",
        );
        let doc = config.parse::<DocumentMut>().unwrap();
        assert_eq!(doc["model_provider"].as_str(), Some(RELAY_PROVIDER));
        assert_eq!(
            doc["openai_base_url"].as_str(),
            Some("https://relay.invalid/v1")
        );
        assert_eq!(
            doc["model_providers"][RELAY_PROVIDER]["supports_websockets"].as_bool(),
            Some(false)
        );
        assert_eq!(
            doc["model_providers"][RELAY_PROVIDER]["requires_openai_auth"].as_bool(),
            Some(true)
        );
        assert_eq!(
            doc["model_providers"]["user_provider"]["name"].as_str(),
            Some("untouched")
        );
        let chatgpt = build_chatgpt_profile_config(Some(&config))
            .parse::<DocumentMut>()
            .unwrap();
        assert_eq!(chatgpt["model_provider"].as_str(), Some("openai"));
        assert!(chatgpt.get("openai_base_url").is_none());
        assert!(chatgpt["model_providers"].get(RELAY_PROVIDER).is_none());
        assert_eq!(
            chatgpt["model_providers"]["user_provider"]["name"].as_str(),
            Some("untouched")
        );
    }
}
