use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

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
pub(crate) struct CurrentAuthStatus {
    pub(crate) available: bool,
    pub(crate) account_id: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) plan_type: Option<String>,
    pub(crate) auth_mode: Option<String>,
    pub(crate) last_refresh: Option<String>,
    pub(crate) file_modified_at: Option<i64>,
    pub(crate) fingerprint: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExtractedAuth {
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
pub(crate) struct ApiProxyStatus {
    pub(crate) running: bool,
    pub(crate) port: Option<u16>,
    pub(crate) api_key: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) lan_base_url: Option<String>,
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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            launch_at_startup: false,
            tray_usage_display_mode: TrayUsageDisplayMode::Remaining,
            launch_codex_after_switch: true,
            sync_opencode_openai_auth: false,
            restart_opencode_desktop_on_switch: false,
            restart_editors_on_switch: false,
            restart_editor_targets: Vec::new(),
            auto_start_api_proxy: false,
            api_proxy_port: default_api_proxy_port(),
            remote_servers: Vec::new(),
            api_proxy_api_key: None,
            locale: AppLocale::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettingsPatch {
    pub(crate) launch_at_startup: Option<bool>,
    pub(crate) tray_usage_display_mode: Option<TrayUsageDisplayMode>,
    pub(crate) launch_codex_after_switch: Option<bool>,
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
        account_variant_key(&self.account_id, self.resolved_plan_type().as_deref())
    }

    pub(crate) fn to_summary(
        &self,
        current_account_id: Option<&str>,
        current_variant_key: Option<&str>,
    ) -> AccountSummary {
        let is_current = current_variant_key
            .map(|variant_key| variant_key == self.variant_key())
            .unwrap_or_else(|| {
                current_account_id
                    .map(|id| id == self.account_id)
                    .unwrap_or(false)
            });

        AccountSummary {
            id: self.id.clone(),
            label: self.label.clone(),
            email: self.email.clone(),
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
