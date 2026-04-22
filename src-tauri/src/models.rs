use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::auth::account_group_key;
use crate::auth::account_variant_key;
use crate::auth::extract_auth;

fn default_api_proxy_port() -> u16 {
    8787
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AccountsStore {
    #[serde(default = "default_store_version")]
    pub(crate) version: u8,
    #[serde(default)]
    pub(crate) accounts: Vec<StoredAccount>,
    #[serde(default)]
    pub(crate) settings: AppSettings,
}

fn default_store_version() -> u8 {
    2
}

impl Default for AccountsStore {
    fn default() -> Self {
        Self {
            version: default_store_version(),
            accounts: Vec::new(),
            settings: AppSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AccountSourceKind {
    Chatgpt,
    Relay,
}

impl Default for AccountSourceKind {
    fn default() -> Self {
        Self::Chatgpt
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredAccount {
    pub(crate) id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) source_kind: AccountSourceKind,
    #[serde(default)]
    pub(crate) principal_id: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) account_id: String,
    pub(crate) plan_type: Option<String>,
    pub(crate) auth_json: Value,
    #[serde(default)]
    pub(crate) api_base_url: Option<String>,
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    #[serde(default)]
    pub(crate) model_name: Option<String>,
    #[serde(default)]
    pub(crate) balance_text: Option<String>,
    #[serde(default)]
    pub(crate) profile_auth_path: Option<String>,
    #[serde(default)]
    pub(crate) profile_config_path: Option<String>,
    #[serde(default)]
    pub(crate) profile_auth_ready: bool,
    #[serde(default)]
    pub(crate) profile_config_ready: bool,
    #[serde(default)]
    pub(crate) profile_integrity_error: Option<String>,
    #[serde(default)]
    pub(crate) profile_last_validated_at: Option<i64>,
    #[serde(default)]
    pub(crate) profile_last_validation_error: Option<String>,
    pub(crate) added_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) usage: Option<UsageSnapshot>,
    pub(crate) usage_error: Option<String>,
    #[serde(default)]
    pub(crate) auth_refresh_blocked: bool,
    #[serde(default)]
    pub(crate) auth_refresh_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountSummary {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) source_kind: AccountSourceKind,
    pub(crate) email: Option<String>,
    pub(crate) account_key: String,
    pub(crate) account_id: String,
    pub(crate) plan_type: Option<String>,
    pub(crate) api_base_url: Option<String>,
    pub(crate) model_name: Option<String>,
    pub(crate) balance_text: Option<String>,
    pub(crate) profile_auth_ready: bool,
    pub(crate) profile_config_ready: bool,
    pub(crate) profile_integrity_error: Option<String>,
    pub(crate) profile_last_validated_at: Option<i64>,
    pub(crate) profile_last_validation_error: Option<String>,
    pub(crate) added_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) usage: Option<UsageSnapshot>,
    pub(crate) usage_error: Option<String>,
    pub(crate) auth_refresh_blocked: bool,
    pub(crate) auth_refresh_error: Option<String>,
    pub(crate) is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageSnapshot {
    pub(crate) fetched_at: i64,
    pub(crate) plan_type: Option<String>,
    pub(crate) five_hour: Option<UsageWindow>,
    pub(crate) one_week: Option<UsageWindow>,
    pub(crate) credits: Option<CreditSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageWindow {
    pub(crate) used_percent: f64,
    pub(crate) window_seconds: i64,
    pub(crate) reset_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreditSnapshot {
    pub(crate) has_credits: bool,
    pub(crate) unlimited: bool,
    pub(crate) balance: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwitchAccountResult {
    pub(crate) account_id: String,
    pub(crate) launched_app_path: Option<String>,
    pub(crate) used_fallback_cli: bool,
    pub(crate) opencode_synced: bool,
    pub(crate) opencode_sync_error: Option<String>,
    pub(crate) opencode_desktop_restarted: bool,
    pub(crate) opencode_desktop_restart_error: Option<String>,
    pub(crate) restarted_editor_apps: Vec<EditorAppId>,
    pub(crate) editor_restart_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparedOauthLogin {
    pub(crate) auth_url: String,
    pub(crate) redirect_uri: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ExtractedAuth {
    pub(crate) principal_id: String,
    pub(crate) account_id: String,
    pub(crate) access_token: String,
    pub(crate) email: Option<String>,
    pub(crate) plan_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthJsonImportInput {
    pub(crate) source: String,
    pub(crate) content: String,
    pub(crate) label: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateApiAccountInput {
    pub(crate) label: String,
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) model_name: String,
    #[serde(default)]
    pub(crate) force_save: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportAccountFailure {
    pub(crate) source: String,
    pub(crate) error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportAccountsResult {
    pub(crate) total_count: usize,
    pub(crate) imported_count: usize,
    pub(crate) updated_count: usize,
    pub(crate) failures: Vec<ImportAccountFailure>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OauthCallbackFinishedEvent {
    pub(crate) result: Option<ImportAccountsResult>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApiProxyStatus {
    pub(crate) running: bool,
    pub(crate) port: Option<u16>,
    pub(crate) api_key: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) lan_base_url: Option<String>,
    pub(crate) active_account_key: Option<String>,
    pub(crate) active_account_id: Option<String>,
    pub(crate) active_account_label: Option<String>,
    pub(crate) last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RemoteAuthMode {
    KeyContent,
    KeyFile,
    KeyPath,
    Password,
}

impl Default for RemoteAuthMode {
    fn default() -> Self {
        Self::KeyPath
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteServerConfig {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) host: String,
    pub(crate) ssh_port: u16,
    pub(crate) ssh_user: String,
    #[serde(default)]
    pub(crate) auth_mode: RemoteAuthMode,
    #[serde(default)]
    pub(crate) identity_file: Option<String>,
    #[serde(default)]
    pub(crate) private_key: Option<String>,
    #[serde(default)]
    pub(crate) password: Option<String>,
    pub(crate) remote_dir: String,
    pub(crate) listen_port: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteProxyStatus {
    pub(crate) installed: bool,
    pub(crate) service_installed: bool,
    pub(crate) running: bool,
    pub(crate) enabled: bool,
    pub(crate) service_name: String,
    pub(crate) pid: Option<u32>,
    pub(crate) base_url: String,
    pub(crate) api_key: Option<String>,
    pub(crate) last_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeployRemoteProxyInput {
    pub(crate) server: RemoteServerConfig,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CloudflaredTunnelMode {
    Quick,
    Named,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloudflaredStatus {
    pub(crate) installed: bool,
    pub(crate) binary_path: Option<String>,
    pub(crate) running: bool,
    pub(crate) tunnel_mode: Option<CloudflaredTunnelMode>,
    pub(crate) public_url: Option<String>,
    pub(crate) custom_hostname: Option<String>,
    pub(crate) use_http2: bool,
    pub(crate) last_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NamedCloudflaredTunnelInput {
    pub(crate) api_token: String,
    pub(crate) account_id: String,
    pub(crate) zone_id: String,
    pub(crate) hostname: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartCloudflaredTunnelInput {
    pub(crate) api_proxy_port: u16,
    pub(crate) use_http2: bool,
    pub(crate) mode: CloudflaredTunnelMode,
    pub(crate) named: Option<NamedCloudflaredTunnelInput>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TrayUsageDisplayMode {
    Used,
    Hidden,
    #[default]
    Remaining,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EditorAppId {
    Vscode,
    VscodeInsiders,
    Cursor,
    Antigravity,
    Kiro,
    Trae,
    Qoder,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub(crate) enum AppLocale {
    #[default]
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en-US")]
    EnUs,
    #[serde(rename = "ja-JP")]
    JaJp,
    #[serde(rename = "ko-KR")]
    KoKr,
    #[serde(rename = "ru-RU")]
    RuRu,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstalledEditorApp {
    pub(crate) id: EditorAppId,
    pub(crate) label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct AppSettings {
    pub(crate) launch_at_startup: bool,
    pub(crate) tray_usage_display_mode: TrayUsageDisplayMode,
    pub(crate) launch_codex_after_switch: bool,
    #[serde(default)]
    pub(crate) smart_switch_include_api: bool,
    pub(crate) codex_launch_path: Option<String>,
    #[serde(default)]
    pub(crate) active_account_id: Option<String>,
    pub(crate) sync_opencode_openai_auth: bool,
    pub(crate) restart_opencode_desktop_on_switch: bool,
    pub(crate) restart_editors_on_switch: bool,
    pub(crate) restart_editor_targets: Vec<EditorAppId>,
    pub(crate) auto_start_api_proxy: bool,
    #[serde(default = "default_api_proxy_port")]
    pub(crate) api_proxy_port: u16,
    pub(crate) remote_servers: Vec<RemoteServerConfig>,
    pub(crate) api_proxy_api_key: Option<String>,
    pub(crate) locale: AppLocale,
    pub(crate) skipped_update_version: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            launch_at_startup: false,
            tray_usage_display_mode: TrayUsageDisplayMode::Remaining,
            launch_codex_after_switch: true,
            smart_switch_include_api: false,
            codex_launch_path: None,
            active_account_id: None,
            sync_opencode_openai_auth: false,
            restart_opencode_desktop_on_switch: false,
            restart_editors_on_switch: false,
            restart_editor_targets: Vec::new(),
            auto_start_api_proxy: false,
            api_proxy_port: default_api_proxy_port(),
            remote_servers: Vec::new(),
            api_proxy_api_key: None,
            locale: AppLocale::default(),
            skipped_update_version: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettingsPatch {
    pub(crate) launch_at_startup: Option<bool>,
    pub(crate) tray_usage_display_mode: Option<TrayUsageDisplayMode>,
    pub(crate) launch_codex_after_switch: Option<bool>,
    pub(crate) smart_switch_include_api: Option<bool>,
    pub(crate) codex_launch_path: Option<Option<String>>,
    pub(crate) sync_opencode_openai_auth: Option<bool>,
    pub(crate) restart_opencode_desktop_on_switch: Option<bool>,
    pub(crate) restart_editors_on_switch: Option<bool>,
    pub(crate) restart_editor_targets: Option<Vec<EditorAppId>>,
    pub(crate) auto_start_api_proxy: Option<bool>,
    pub(crate) api_proxy_port: Option<u16>,
    pub(crate) remote_servers: Option<Vec<RemoteServerConfig>>,
    pub(crate) locale: Option<AppLocale>,
    pub(crate) skipped_update_version: Option<Option<String>>,
}

impl StoredAccount {
    pub(crate) fn principal_key(&self) -> String {
        if matches!(self.source_kind, AccountSourceKind::Relay) {
            return format!("relay:{}", self.id);
        }

        normalized_identity_key(self.principal_id.as_deref())
            .or_else(|| {
                extract_auth(&self.auth_json)
                    .ok()
                    .map(|auth| auth.principal_id)
            })
            .or_else(|| normalized_email_key(self.email.as_deref()))
            .unwrap_or_else(|| self.account_id.clone())
    }

    pub(crate) fn account_key(&self) -> String {
        if matches!(self.source_kind, AccountSourceKind::Relay) {
            return crate::profile_files::relay_account_key(&self.id);
        }

        account_group_key(&self.principal_key(), &self.account_id)
    }

    pub(crate) fn resolved_plan_type(&self) -> Option<String> {
        if matches!(self.source_kind, AccountSourceKind::Relay) {
            return self.plan_type.clone();
        }

        self.plan_type
            .clone()
            .or_else(|| {
                extract_auth(&self.auth_json)
                    .ok()
                    .and_then(|auth| auth.plan_type)
            })
            .or_else(|| {
                self.usage
                    .as_ref()
                    .and_then(|usage| usage.plan_type.clone())
            })
    }

    pub(crate) fn variant_key(&self) -> String {
        if matches!(self.source_kind, AccountSourceKind::Relay) {
            return self.account_key();
        }

        account_variant_key(
            &self.principal_key(),
            &self.account_id,
            self.resolved_plan_type().as_deref(),
        )
    }

    pub(crate) fn to_summary(
        &self,
        current_account_key: Option<&str>,
        current_variant_key: Option<&str>,
    ) -> AccountSummary {
        let account_key = self.account_key();
        let is_current = current_variant_key
            .map(|variant_key| variant_key == self.variant_key())
            .unwrap_or_else(|| {
                current_account_key
                    .map(|key| key == account_key)
                    .unwrap_or(false)
            });

        AccountSummary {
            id: self.id.clone(),
            label: self.label.clone(),
            source_kind: self.source_kind.clone(),
            email: self.email.clone(),
            account_key,
            account_id: self.account_id.clone(),
            plan_type: self.plan_type.clone(),
            api_base_url: self.api_base_url.clone(),
            model_name: self.model_name.clone(),
            balance_text: self.balance_text.clone(),
            profile_auth_ready: self.profile_auth_ready,
            profile_config_ready: self.profile_config_ready,
            profile_integrity_error: self.profile_integrity_error.clone(),
            profile_last_validated_at: self.profile_last_validated_at,
            profile_last_validation_error: self.profile_last_validation_error.clone(),
            added_at: self.added_at,
            updated_at: self.updated_at,
            usage: self.usage.clone(),
            usage_error: self.usage_error.clone(),
            auth_refresh_blocked: self.auth_refresh_blocked,
            auth_refresh_error: self.auth_refresh_error.clone(),
            is_current,
        }
    }
}

fn normalized_email_key(email: Option<&str>) -> Option<String> {
    email
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn normalized_identity_key(value: Option<&str>) -> Option<String> {
    let trimmed = value.map(str::trim).filter(|value| !value.is_empty())?;
    if trimmed.contains('@') {
        Some(trimmed.to_ascii_lowercase())
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn dedupe_account_variants(accounts: &mut Vec<StoredAccount>) -> bool {
    let mut changed = false;
    let mut merged_accounts: Vec<StoredAccount> = Vec::with_capacity(accounts.len());
    let mut index_by_variant: HashMap<String, usize> = HashMap::new();

    for account in std::mem::take(accounts) {
        let variant_key = account.variant_key();
        if let Some(existing_index) = index_by_variant.get(&variant_key).copied() {
            let merged =
                merge_duplicate_account_variant(merged_accounts[existing_index].clone(), account);
            merged_accounts[existing_index] = merged;
            changed = true;
        } else {
            index_by_variant.insert(variant_key, merged_accounts.len());
            merged_accounts.push(account);
        }
    }

    *accounts = merged_accounts;

    changed
}

fn merge_duplicate_account_variant(left: StoredAccount, right: StoredAccount) -> StoredAccount {
    let left_score = duplicate_account_merge_score(&left);
    let right_score = duplicate_account_merge_score(&right);
    let (mut preferred, alternate) = if right_score > left_score {
        (right, left)
    } else {
        (left, right)
    };

    preferred.added_at = preferred.added_at.min(alternate.added_at);
    preferred.updated_at = preferred.updated_at.max(alternate.updated_at);

    if preferred.email.is_none() {
        preferred.email = alternate.email.clone();
    }
    if preferred.plan_type.is_none() {
        preferred.plan_type = alternate.plan_type.clone();
    }
    if preferred.usage.is_none() {
        preferred.usage = alternate.usage.clone();
    }
    if preferred.usage_error.is_none() {
        preferred.usage_error = alternate.usage_error.clone();
    }
    if !preferred.auth_refresh_blocked && alternate.auth_refresh_blocked {
        preferred.auth_refresh_blocked = true;
    }
    if preferred.auth_refresh_error.is_none() {
        preferred.auth_refresh_error = alternate.auth_refresh_error.clone();
    }
    if preferred.auth_json.is_null() && !alternate.auth_json.is_null() {
        preferred.auth_json = alternate.auth_json.clone();
    }
    if preferred.api_base_url.is_none() {
        preferred.api_base_url = alternate.api_base_url.clone();
    }
    if preferred.api_key.is_none() {
        preferred.api_key = alternate.api_key.clone();
    }
    if preferred.model_name.is_none() {
        preferred.model_name = alternate.model_name.clone();
    }
    if preferred.balance_text.is_none() {
        preferred.balance_text = alternate.balance_text.clone();
    }
    if preferred.profile_auth_path.is_none() {
        preferred.profile_auth_path = alternate.profile_auth_path.clone();
    }
    if preferred.profile_config_path.is_none() {
        preferred.profile_config_path = alternate.profile_config_path.clone();
    }
    preferred.profile_auth_ready = preferred.profile_auth_ready || alternate.profile_auth_ready;
    preferred.profile_config_ready =
        preferred.profile_config_ready || alternate.profile_config_ready;
    if preferred.profile_integrity_error.is_none() {
        preferred.profile_integrity_error = alternate.profile_integrity_error.clone();
    }
    if preferred.profile_last_validated_at.is_none() {
        preferred.profile_last_validated_at = alternate.profile_last_validated_at;
    }
    if preferred.profile_last_validation_error.is_none() {
        preferred.profile_last_validation_error = alternate.profile_last_validation_error.clone();
    }

    preferred
}

fn duplicate_account_merge_score(account: &StoredAccount) -> (u8, u8, u8, u8, i64, i64) {
    (
        u8::from(account.usage.is_some() && account.usage_error.is_none()),
        u8::from(!account.auth_refresh_blocked),
        u8::from(account.resolved_plan_type().is_some()),
        u8::from(
            account
                .email
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some(),
        ),
        account.updated_at,
        account.added_at,
    )
}

#[cfg(test)]
mod tests {
    use super::dedupe_account_variants;
    use super::StoredAccount;
    use super::UsageSnapshot;
    use super::UsageWindow;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use serde_json::json;

    fn usage_snapshot(plan_type: &str) -> UsageSnapshot {
        UsageSnapshot {
            fetched_at: 10,
            plan_type: Some(plan_type.to_string()),
            five_hour: Some(UsageWindow {
                used_percent: 10.0,
                window_seconds: 18_000,
                reset_at: Some(20),
            }),
            one_week: Some(UsageWindow {
                used_percent: 20.0,
                window_seconds: 604_800,
                reset_at: Some(30),
            }),
            credits: None,
        }
    }

    fn jwt_with_plan(plan_type: &str) -> String {
        let payload = URL_SAFE_NO_PAD.encode(format!(
            r#"{{"email":"shared@example.com","https://api.openai.com/auth":{{"chatgpt_account_id":"account-1","chatgpt_plan_type":"{plan_type}"}}}}"#
        ));
        format!("header.{payload}.signature")
    }

    fn stored_account(
        id: &str,
        label: &str,
        account_id: &str,
        plan_type: Option<&str>,
        usage_plan_type: Option<&str>,
        updated_at: i64,
    ) -> StoredAccount {
        StoredAccount {
            id: id.to_string(),
            label: label.to_string(),
            source_kind: Default::default(),
            principal_id: Some("shared@example.com".to_string()),
            email: Some("shared@example.com".to_string()),
            account_id: account_id.to_string(),
            plan_type: plan_type.map(ToString::to_string),
            auth_json: json!({ "id": id }),
            api_base_url: None,
            api_key: None,
            model_name: None,
            balance_text: None,
            profile_auth_path: None,
            profile_config_path: None,
            profile_auth_ready: false,
            profile_config_ready: false,
            profile_integrity_error: None,
            profile_last_validated_at: None,
            profile_last_validation_error: None,
            added_at: updated_at - 1,
            updated_at,
            usage: usage_plan_type.map(usage_snapshot),
            usage_error: None,
            auth_refresh_blocked: false,
            auth_refresh_error: None,
        }
    }

    #[test]
    fn dedupe_account_variants_keeps_newest_variant_record() {
        let mut accounts = vec![
            stored_account(
                "old",
                "legacy",
                "account-1",
                Some("team"),
                Some("team"),
                100,
            ),
            stored_account("new", "fresh", "account-1", Some("team"), Some("team"), 200),
        ];

        let changed = dedupe_account_variants(&mut accounts);

        assert!(changed);
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, "new");
        assert_eq!(accounts[0].label, "fresh");
        assert_eq!(accounts[0].added_at, 99);
        assert_eq!(accounts[0].updated_at, 200);
    }

    #[test]
    fn dedupe_account_variants_merges_when_usage_reveals_same_variant() {
        let mut accounts = vec![
            stored_account("unknown", "legacy", "account-1", None, Some("team"), 100),
            stored_account(
                "team",
                "current",
                "account-1",
                Some("team"),
                Some("team"),
                200,
            ),
        ];

        let changed = dedupe_account_variants(&mut accounts);

        assert!(changed);
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, "team");
    }

    #[test]
    fn resolved_plan_type_prefers_stored_plan_type_over_usage_plan_type() {
        let account = StoredAccount {
            id: "mixed".to_string(),
            label: "mixed".to_string(),
            source_kind: Default::default(),
            principal_id: Some("shared@example.com".to_string()),
            email: Some("shared@example.com".to_string()),
            account_id: "account-1".to_string(),
            plan_type: Some("team".to_string()),
            auth_json: json!({ "kind": "mixed" }),
            api_base_url: None,
            api_key: None,
            model_name: None,
            balance_text: None,
            profile_auth_path: None,
            profile_config_path: None,
            profile_auth_ready: false,
            profile_config_ready: false,
            profile_integrity_error: None,
            profile_last_validated_at: None,
            profile_last_validation_error: None,
            added_at: 1,
            updated_at: 1,
            usage: Some(usage_snapshot("plus")),
            usage_error: None,
            auth_refresh_blocked: false,
            auth_refresh_error: None,
        };

        assert_eq!(account.resolved_plan_type().as_deref(), Some("team"));
        assert_eq!(account.variant_key(), "shared@example.com|account-1|team");
    }

    #[test]
    fn resolved_plan_type_falls_back_to_auth_claim_before_usage() {
        let account = StoredAccount {
            id: "auth".to_string(),
            label: "auth".to_string(),
            source_kind: Default::default(),
            principal_id: Some("shared@example.com".to_string()),
            email: Some("shared@example.com".to_string()),
            account_id: "account-1".to_string(),
            plan_type: None,
            auth_json: json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "access_token": "token",
                    "id_token": jwt_with_plan("team")
                }
            }),
            api_base_url: None,
            api_key: None,
            model_name: None,
            balance_text: None,
            profile_auth_path: None,
            profile_config_path: None,
            profile_auth_ready: false,
            profile_config_ready: false,
            profile_integrity_error: None,
            profile_last_validated_at: None,
            profile_last_validation_error: None,
            added_at: 1,
            updated_at: 1,
            usage: Some(usage_snapshot("plus")),
            usage_error: None,
            auth_refresh_blocked: false,
            auth_refresh_error: None,
        };

        assert_eq!(account.resolved_plan_type().as_deref(), Some("team"));
    }

    #[test]
    fn persisted_principal_id_keeps_same_workspace_different_users_separate() {
        let mut accounts = vec![
            StoredAccount {
                id: "first".to_string(),
                label: "first".to_string(),
                source_kind: Default::default(),
                principal_id: Some("first@example.com".to_string()),
                email: None,
                account_id: "workspace-1".to_string(),
                plan_type: Some("team".to_string()),
                auth_json: json!({ "kind": "legacy" }),
                api_base_url: None,
                api_key: None,
                model_name: None,
                balance_text: None,
                profile_auth_path: None,
                profile_config_path: None,
                profile_auth_ready: false,
                profile_config_ready: false,
                profile_integrity_error: None,
                profile_last_validated_at: None,
                profile_last_validation_error: None,
                added_at: 1,
                updated_at: 1,
                usage: None,
                usage_error: None,
                auth_refresh_blocked: false,
                auth_refresh_error: None,
            },
            StoredAccount {
                id: "second".to_string(),
                label: "second".to_string(),
                source_kind: Default::default(),
                principal_id: Some("second@example.com".to_string()),
                email: None,
                account_id: "workspace-1".to_string(),
                plan_type: Some("team".to_string()),
                auth_json: json!({ "kind": "legacy" }),
                api_base_url: None,
                api_key: None,
                model_name: None,
                balance_text: None,
                profile_auth_path: None,
                profile_config_path: None,
                profile_auth_ready: false,
                profile_config_ready: false,
                profile_integrity_error: None,
                profile_last_validated_at: None,
                profile_last_validation_error: None,
                added_at: 2,
                updated_at: 2,
                usage: None,
                usage_error: None,
                auth_refresh_blocked: false,
                auth_refresh_error: None,
            },
        ];

        let changed = dedupe_account_variants(&mut accounts);

        assert!(!changed);
        assert_eq!(accounts.len(), 2);
        assert_ne!(accounts[0].account_key(), accounts[1].account_key());
    }
}
