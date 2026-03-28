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
    1
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
pub(crate) struct StoredAccount {
    pub(crate) id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) principal_id: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) account_id: String,
    pub(crate) plan_type: Option<String>,
    pub(crate) auth_json: Value,
    pub(crate) added_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) usage: Option<UsageSnapshot>,
    pub(crate) usage_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountSummary {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) email: Option<String>,
    pub(crate) account_key: String,
    pub(crate) account_id: String,
    pub(crate) plan_type: Option<String>,
    pub(crate) added_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) usage: Option<UsageSnapshot>,
    pub(crate) usage_error: Option<String>,
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
    pub(crate) codex_launch_path: Option<String>,
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
    /// Unix seconds; updated whenever the daemon successfully refreshes a token.
    #[serde(default)]
    pub(crate) last_token_refresh_at: Option<i64>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            launch_at_startup: false,
            tray_usage_display_mode: TrayUsageDisplayMode::Remaining,
            launch_codex_after_switch: true,
            codex_launch_path: None,
            sync_opencode_openai_auth: false,
            restart_opencode_desktop_on_switch: false,
            restart_editors_on_switch: false,
            restart_editor_targets: Vec::new(),
            auto_start_api_proxy: false,
            api_proxy_port: default_api_proxy_port(),
            remote_servers: Vec::new(),
            api_proxy_api_key: None,
            locale: AppLocale::default(),
            last_token_refresh_at: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettingsPatch {
    pub(crate) launch_at_startup: Option<bool>,
    pub(crate) tray_usage_display_mode: Option<TrayUsageDisplayMode>,
    pub(crate) launch_codex_after_switch: Option<bool>,
    pub(crate) codex_launch_path: Option<Option<String>>,
    pub(crate) sync_opencode_openai_auth: Option<bool>,
    pub(crate) restart_opencode_desktop_on_switch: Option<bool>,
    pub(crate) restart_editors_on_switch: Option<bool>,
    pub(crate) restart_editor_targets: Option<Vec<EditorAppId>>,
    pub(crate) auto_start_api_proxy: Option<bool>,
    pub(crate) api_proxy_port: Option<u16>,
    pub(crate) remote_servers: Option<Vec<RemoteServerConfig>>,
    pub(crate) locale: Option<AppLocale>,
}

impl StoredAccount {
    pub(crate) fn principal_key(&self) -> String {
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
        account_group_key(&self.principal_key(), &self.account_id)
    }

    pub(crate) fn resolved_plan_type(&self) -> Option<String> {
        self.usage
            .as_ref()
            .and_then(|usage| usage.plan_type.clone())
            .or(self.plan_type.clone())
            .or_else(|| {
                extract_auth(&self.auth_json)
                    .ok()
                    .and_then(|auth| auth.plan_type)
            })
    }

    pub(crate) fn variant_key(&self) -> String {
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
            email: self.email.clone(),
            account_key,
            account_id: self.account_id.clone(),
            plan_type: self.plan_type.clone(),
            added_at: self.added_at,
            updated_at: self.updated_at,
            usage: self.usage.clone(),
            usage_error: self.usage_error.clone(),
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
    if preferred.auth_json.is_null() && !alternate.auth_json.is_null() {
        preferred.auth_json = alternate.auth_json.clone();
    }

    preferred
}

fn duplicate_account_merge_score(account: &StoredAccount) -> (u8, u8, u8, i64, i64) {
    (
        u8::from(account.usage.is_some() && account.usage_error.is_none()),
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
            principal_id: Some("shared@example.com".to_string()),
            email: Some("shared@example.com".to_string()),
            account_id: account_id.to_string(),
            plan_type: plan_type.map(ToString::to_string),
            auth_json: json!({ "id": id }),
            added_at: updated_at - 1,
            updated_at,
            usage: usage_plan_type.map(usage_snapshot),
            usage_error: None,
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
    fn persisted_principal_id_keeps_same_workspace_different_users_separate() {
        let mut accounts = vec![
            StoredAccount {
                id: "first".to_string(),
                label: "first".to_string(),
                principal_id: Some("first@example.com".to_string()),
                email: None,
                account_id: "workspace-1".to_string(),
                plan_type: Some("team".to_string()),
                auth_json: json!({ "kind": "legacy" }),
                added_at: 1,
                updated_at: 1,
                usage: None,
                usage_error: None,
            },
            StoredAccount {
                id: "second".to_string(),
                label: "second".to_string(),
                principal_id: Some("second@example.com".to_string()),
                email: None,
                account_id: "workspace-1".to_string(),
                plan_type: Some("team".to_string()),
                auth_json: json!({ "kind": "legacy" }),
                added_at: 2,
                updated_at: 2,
                usage: None,
                usage_error: None,
            },
        ];

        let changed = dedupe_account_variants(&mut accounts);

        assert!(!changed);
        assert_eq!(accounts.len(), 2);
        assert_ne!(accounts[0].account_key(), accounts[1].account_key());
    }
}
