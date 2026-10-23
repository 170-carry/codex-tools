//! Account-specific routing layered over shared desktop settings.
use super::*;

pub(super) fn build_chatgpt_profile_config(current_config: Option<&str>) -> String {
    let mut document = parse_config_or_default(current_config);
    codex_catalog::remove_managed_catalog(&mut document);
    let had_base_url = document.get("openai_base_url").is_some();
    document.remove("openai_base_url");
    // Existing sessions and named profiles can still reference the relay ID.
    // Keep its definition when changing the active provider to OpenAI.
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
    codex_catalog::remove_managed_catalog(&mut document);
    document["openai_base_url"] = value(base_url);
    document["model"] = value(model_name);
    configure_relay_provider(&mut document, base_url);
    document["cli_auth_credentials_store"] = value(MANAGED_AUTH_CREDENTIALS_STORE);
    document.to_string()
}

pub(super) fn build_codex_proxy_config(
    current_config: Option<&str>,
    base_url: &str,
    catalog_path: &Path,
) -> String {
    let mut document = parse_config_or_default(current_config);
    document["openai_base_url"] = value(base_url);
    document["model_provider"] = value("openai");
    document["model_catalog_json"] = value(catalog_path.to_string_lossy().as_ref());
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
    // Normalize inline/dotted forms before adding a named provider. Inserting a
    // Table under an inline table can otherwise omit it from serialized TOML.
    let mut providers = document
        .remove("model_providers")
        .and_then(|item| item.into_table().ok())
        .unwrap_or_default();
    providers.set_dotted(false);
    providers.set_implicit(true);
    providers.insert(RELAY_PROVIDER, toml_edit::Item::Table(provider));
    document["model_providers"] = toml_edit::Item::Table(providers);
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
                            == Some(RELAY_PROVIDER)
                        && relay_definition_matches(&config, account.api_base_url.as_deref())))
        })
}

fn relay_definition_matches(config: &DocumentMut, base_url: Option<&str>) -> bool {
    let Some(provider) = config
        .get("model_providers")
        .and_then(|providers| providers.get(RELAY_PROVIDER))
    else {
        return false;
    };
    provider.get("base_url").and_then(toml_edit::Item::as_str) == base_url
        && provider.get("wire_api").and_then(toml_edit::Item::as_str) == Some("responses")
        && provider
            .get("requires_openai_auth")
            .and_then(toml_edit::Item::as_bool)
            == Some(true)
        && provider
            .get("supports_websockets")
            .and_then(toml_edit::Item::as_bool)
            == Some(false)
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
        assert!(chatgpt["model_providers"].get(RELAY_PROVIDER).is_some());
        assert_eq!(
            chatgpt["model_providers"]["user_provider"]["name"].as_str(),
            Some("untouched")
        );
    }
}

#[cfg(test)]
mod provider_regression_tests {
    use super::*;

    #[test]
    fn serialized_provider_survives_inline_and_dotted_configurations() {
        for original in [
            "model_providers = {}\n",
            "model_providers = { other = { name = \"other\", base_url = \"https://other.invalid\" } }\n",
            "model_providers.other.name = \"other\"\n",
            "[model_providers.other]\nname = \"other\"\n",
        ] {
            let serialized = build_relay_profile_config(Some(original), "https://relay.invalid/v1", "test-model");
            let parsed = serialized.parse::<DocumentMut>().unwrap();
            assert!(relay_definition_matches(&parsed, Some("https://relay.invalid/v1")), "{serialized}");
            if original.contains("other") { assert!(parsed["model_providers"].get("other").is_some()); }
        }
    }

    #[test]
    fn incomplete_or_missing_provider_is_not_a_matching_profile() {
        for original in [
            "model_provider = \"codex_tools_relay\"\n",
            "[model_providers.codex_tools_relay]\nbase_url = \"https://relay.invalid/v1\"\n",
        ] {
            let parsed = original.parse::<DocumentMut>().unwrap();
            assert!(!relay_definition_matches(
                &parsed,
                Some("https://relay.invalid/v1")
            ));
        }
    }

    #[test]
    fn returning_to_chatgpt_keeps_definitions_used_by_saved_profiles() {
        let relay = build_relay_profile_config(
            Some("[profiles.work]\nmodel_provider = \"codex_tools_relay\"\n"),
            "https://relay.invalid/v1",
            "test-model",
        );
        let restored = build_chatgpt_profile_config(Some(&relay))
            .parse::<DocumentMut>()
            .unwrap();
        assert_eq!(restored["model_provider"].as_str(), Some("openai"));
        assert!(relay_definition_matches(
            &restored,
            Some("https://relay.invalid/v1")
        ));
    }
}
