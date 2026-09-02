use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::Infallible;
use std::fs;
use std::future::Future;
use std::io::BufWriter;
use std::io::Read;
use std::io::Write;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::RwLock;
use std::thread::JoinHandle as ThreadJoinHandle;

use async_stream::stream;
use axum::body::Body;
use axum::body::Bytes;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::Message as AxumWebSocketMessage;
use axum::extract::ws::WebSocket as AxumWebSocket;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::DefaultBodyLimit;
use axum::extract::FromRequest;
use axum::extract::Multipart;
use axum::extract::Request;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::Method;
use axum::http::Response;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::response::IntoResponse;
use axum::routing::any;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use flate2::read::DeflateDecoder;
use flate2::read::GzDecoder;
use flate2::read::ZlibDecoder;
use futures_util::SinkExt;
use futures_util::Stream;
use futures_util::StreamExt;
use if_addrs::IfAddr;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OpenFlags;
use rusqlite::OptionalExtension;
use rusqlite::TransactionBehavior;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use serde_json::Map;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::sync::Semaphore;
use tokio_tungstenite::client_async_tls_with_config;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

#[cfg(feature = "desktop")]
use tauri::AppHandle;

use crate::app_paths;
use crate::auth::extract_auth;
use crate::auth::refresh_chatgpt_auth_tokens;
use crate::models::normalize_api_proxy_sequential_five_hour_limit_percent;
use crate::models::AccountSourceKind;
use crate::models::ApiProxyKey;
use crate::models::ApiProxyKeyUsageLogEntry;
use crate::models::ApiProxyLoadBalanceMode;
use crate::models::ApiProxyStatus;
use crate::models::ApiProxyUsageKeySeries;
use crate::models::ApiProxyUsagePoint;
use crate::models::ApiProxyUsageSeries;
use crate::models::ApiProxyUsageStats;
use crate::models::AppSettings;
use crate::models::CreateApiProxyKeyInput;
use crate::models::StoredAccount;
use crate::models::UpdateApiProxyKeyInput;
use crate::models::UsageSnapshot;
use crate::models::UsageWindow;
use crate::models::DEFAULT_API_PROXY_MODEL;
use crate::models::DEFAULT_API_PROXY_REASONING_EFFORT;
use crate::models::DEFAULT_API_PROXY_SERVICE_TIER;
use crate::profile_files;
use crate::state::ApiProxyRuntimeHandle;
use crate::state::ApiProxyRuntimeSnapshot;
use crate::state::ApiProxySessionAffinity;
#[cfg(feature = "desktop")]
use crate::state::AppState;
use crate::store::account_store_path_from_data_dir;
use crate::store::load_store_from_path;
use crate::store::save_store_to_path;
use crate::store::update_account_group_refresh_state_in_path;
use crate::usage::resolve_chatgpt_base_origin;
use crate::utils::now_unix_seconds;
use crate::utils::private_create_new_options;
use crate::utils::set_private_permissions;
use crate::utils::truncate_for_error;

const DEFAULT_PROXY_PORT: u16 = 8787;
const DEFAULT_PROXY_REQUEST_BODY_LIMIT_MIB: usize = 512;
const DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES: usize =
    DEFAULT_PROXY_REQUEST_BODY_LIMIT_MIB * 1024 * 1024;
const MAX_CONCURRENT_PROXY_JSON_DECODE_JOBS: usize = 4;
const MAX_PROXY_DECOMPRESSED_BODY_BYTES: usize = 128 * 1024 * 1024;
const MIN_PROXY_DECOMPRESSED_BODY_BUDGET_BYTES: usize = 1024 * 1024;
const MAX_PROXY_BODY_EXPANSION_RATIO: usize = 128;
const DEFAULT_PROXY_UPSTREAM_TIMEOUT_SECS: u64 = 1_800;
const DEFAULT_PROXY_CONNECT_TIMEOUT_SECS: u64 = 30;
#[cfg(feature = "desktop")]
const DESKTOP_API_PROXY_BIND_HOST: &str = "0.0.0.0";
const MAX_PROXY_CONNECT_RESPONSE_BYTES: usize = 16 * 1024;
const PROXY_REQUEST_BODY_LIMIT_MIB_ENV_VAR: &str = "CODEX_TOOLS_PROXY_MAX_BODY_MIB";
const CODEX_CLIENT_VERSION: &str = "0.144.0";
const CODEX_USER_AGENT: &str = "codex_cli_rs/0.144.0";
const RESPONSES_WEBSOCKETS_BETA: &str = "responses_websockets=2026-02-06";
const ANTHROPIC_MESSAGES_REQUIRED_VERSION: &str = "2023-06-01";
const SSE_DONE: &str = "data: [DONE]\n\n";
const DEFAULT_IMAGE_CONTROLLER_MODEL: &str = "gpt-5.5";
const DEFAULT_IMAGE_TOOL_MODEL: &str = "gpt-image-2";
const DEFAULT_UPSTREAM_SERVICE_TIER: &str = "default";
const PROXY_DEFAULT_SERVICE_TIER_ENV_VAR: &str = "CODEX_TOOLS_PROXY_SERVICE_TIER";
const RESPONSES_LITE_HEADER: &str = "x-openai-internal-codex-responses-lite";
const IMAGE_VARIATION_PROMPT: &str = "Create a faithful variation of the provided image.";
const COMPACT_SSE_KEEPALIVE: &str =
    "event: response.in_progress\ndata: {\"type\":\"response.in_progress\"}\n\n";
const COMPACT_SSE_KEEPALIVE_INTERVAL_SECS: u64 = 10;
const MAX_COMPACT_ERROR_BODY_BYTES: usize = 1024 * 1024;
const MAX_COMPACT_RESPONSE_BODY_BYTES: usize = 32 * 1024 * 1024;
const MAX_ACCOUNT_WARMUP_RESPONSE_BYTES: usize = 1024 * 1024;
const ACCOUNT_WARMUP_TIMEOUT_SECS: u64 = 90;
const CODEX_TURN_STATE_HEADER: &str = "x-codex-turn-state";
const CODEX_CONTEXT_REQUEST_HEADERS: &[&str] = &[
    CODEX_TURN_STATE_HEADER,
    "x-codex-beta-features",
    "x-codex-turn-metadata",
    "x-codex-window-id",
    "x-codex-installation-id",
    "x-codex-parent-thread-id",
    "thread-id",
    "x-openai-subagent",
    "x-openai-memgen-request",
    "x-responsesapi-include-timing-metrics",
    "x-client-request-id",
    "traceparent",
    "tracestate",
];
static PROXY_JSON_DECODE_LIMITER: OnceLock<Arc<Semaphore>> = OnceLock::new();
const MODELS: &[&str] = &[
    "gpt-5.6-sol",
    "gpt-5.6-terra",
    "gpt-5.6-luna",
    "gpt-5.5",
    "gpt-5.4",
    "gpt-image-2",
];
const REQUEST_MODEL_MAPPINGS: &[(&str, &str)] = &[
    ("gpt-5.6-sol", "gpt-5.6-sol"),
    ("gpt-5.6-terra", "gpt-5.6-terra"),
    ("gpt-5.6-luna", "gpt-5.6-luna"),
    ("gpt5.6-sol", "gpt-5.6-sol"),
    ("gpt-5-6-sol", "gpt-5.6-sol"),
    ("gpt5.6-terra", "gpt-5.6-terra"),
    ("gpt-5-6-terra", "gpt-5.6-terra"),
    ("gpt5.6-luna", "gpt-5.6-luna"),
    ("gpt-5-6-luna", "gpt-5.6-luna"),
    ("gpt5.6", "gpt-5.6-sol"),
    ("gpt-5-6", "gpt-5.6-sol"),
    ("gpt-5.6", "gpt-5.6-sol"),
    ("gpt5.5", "gpt-5.5"),
    ("gpt-5-5", "gpt-5.5"),
    ("gpt5.4", "gpt-5.4"),
    ("gpt-5-4", "gpt-5.4"),
];
const RESPONSE_MODEL_NORMALIZATIONS: &[(&str, &str)] = &[
    ("gpt-5.6-sol", "gpt-5.6-sol"),
    ("gpt-5.6-terra", "gpt-5.6-terra"),
    ("gpt-5.6-luna", "gpt-5.6-luna"),
    ("gpt5.6-sol", "gpt-5.6-sol"),
    ("gpt-5-6-sol", "gpt-5.6-sol"),
    ("gpt5.6-terra", "gpt-5.6-terra"),
    ("gpt-5-6-terra", "gpt-5.6-terra"),
    ("gpt5.6-luna", "gpt-5.6-luna"),
    ("gpt-5-6-luna", "gpt-5.6-luna"),
    ("gpt5.6", "gpt-5.6-sol"),
    ("gpt-5-6", "gpt-5.6-sol"),
    ("gpt-5.6", "gpt-5.6-sol"),
    ("gpt5.5", "gpt-5.5"),
    ("gpt-5-5", "gpt-5.5"),
    ("gpt5.4", "gpt-5.4"),
    ("gpt-5-4", "gpt-5.4"),
];
const UNSUPPORTED_RESPONSES_REQUEST_FIELDS: &[&str] = &["metadata", "prompt_cache_retention"];
const API_PROXY_USAGE_FILE_NAME: &str = "api-proxy-usage.json";
const API_PROXY_USAGE_DATABASE_FILE_NAME: &str = "api-proxy-usage.sqlite3";
const API_PROXY_USAGE_LEGACY_MIGRATION_KEY: &str = "legacy-json-v1";
const API_PROXY_USAGE_STORE_VERSION: u8 = 1;
const API_PROXY_KEYS_FILE_NAME: &str = "api-proxy-keys.json";
const API_PROXY_KEYS_STORE_VERSION: u8 = 1;
const API_PROXY_USAGE_RANGE_1H_SECONDS: i64 = 60 * 60;
const API_PROXY_USAGE_RANGE_24H_SECONDS: i64 = 24 * 60 * 60;
const API_PROXY_USAGE_RANGE_7D_SECONDS: i64 = 7 * 24 * 60 * 60;
const API_PROXY_USAGE_RANGE_14D_SECONDS: i64 = 14 * 24 * 60 * 60;
const API_PROXY_USAGE_RANGE_30D_SECONDS: i64 = 30 * 24 * 60 * 60;
const DEFAULT_API_PROXY_USAGE_RANGE_SECONDS: i64 = API_PROXY_USAGE_RANGE_24H_SECONDS;
const API_PROXY_REASONING_EFFORTS: &[&str] =
    &["none", "minimal", "low", "medium", "high", "xhigh", "max"];
const API_PROXY_SERVICE_TIERS: &[&str] = &["auto", "default", "fast", "flex"];
const MAX_PROXY_SESSION_AFFINITY_ENTRIES: usize = 512;
// Idle session bindings expire so long-gone conversations stop pinning
// accounts; active sessions refresh their binding on every success.
const PROXY_SESSION_AFFINITY_TTL_SECS: i64 = 3_600;
const SESSION_AFFINITY_HEADERS: &[&str] = &[
    "x-claude-code-session-id",
    "x-claude-session-id",
    "x-codex-tools-session",
    "x-codex-session-id",
    "session_id",
    "session-id",
    "x-client-request-id",
    "openai-conversation-id",
];
const SESSION_AFFINITY_PAYLOAD_KEYS: &[&str] = &[
    "session_id",
    "sessionId",
    "conversation_id",
    "conversationId",
    "thread_id",
    "threadId",
    "previous_response_id",
    "previousResponseId",
];

#[derive(Clone)]
pub(crate) struct ProxyStorageContext {
    pub(crate) data_dir: PathBuf,
    pub(crate) store_lock: Arc<tokio::sync::Mutex<()>>,
    pub(crate) auth_refresh_lock: Arc<tokio::sync::Mutex<()>>,
    pub(crate) usage_writer: Arc<tokio::sync::Mutex<Option<ApiProxyUsageWriter>>>,
    pub(crate) sync_active_auth_on_refresh: bool,
}

#[derive(Clone)]
struct ProxyCandidate {
    id: String,
    label: String,
    source_kind: AccountSourceKind,
    account_key: String,
    account_id: String,
    access_token: String,
    auth_json: Value,
    api_base_url: Option<String>,
    model_name: Option<String>,
    variant_key: String,
    plan_type: Option<String>,
    usage: Option<UsageSnapshot>,
    auth_refresh_blocked: bool,
    auth_refresh_error: Option<String>,
    updated_at: i64,
}

struct ProxyCandidateSelection {
    candidates: Vec<ProxyCandidate>,
    load_balance: ProxyLoadBalanceConfig,
    settings: AppSettings,
    persisted_sequential_account_key: Option<String>,
}

#[derive(Clone, Debug)]
struct ApiProxyUsageMetadata {
    key_id: Option<String>,
    key_label: Option<String>,
    model: String,
    route: String,
    reasoning_effort: Option<String>,
    service_tier: Option<String>,
}

#[derive(Clone, Debug)]
struct ApiProxyTokenUsageBreakdown {
    total_tokens: i64,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cached_input_tokens: Option<i64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ApiProxyUsageEvent {
    timestamp: i64,
    key_id: Option<String>,
    key_label: Option<String>,
    model: String,
    route: Option<String>,
    reasoning_effort: Option<String>,
    service_tier: Option<String>,
    calls: i64,
    tokens: i64,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cached_input_tokens: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ApiProxyUsageStore {
    version: u8,
    updated_at: i64,
    events: Vec<ApiProxyUsageEvent>,
}

impl Default for ApiProxyUsageStore {
    fn default() -> Self {
        Self {
            version: API_PROXY_USAGE_STORE_VERSION,
            updated_at: 0,
            events: Vec::new(),
        }
    }
}

pub(crate) struct ApiProxyUsageWriter {
    sender: Option<mpsc::Sender<ApiProxyUsageCommand>>,
    thread: Option<ThreadJoinHandle<()>>,
}

enum ApiProxyUsageCommand {
    Append(ApiProxyUsageEvent),
    QueryKeyLogs {
        limit: usize,
        reply: oneshot::Sender<Result<Vec<ApiProxyKeyUsageLogEntry>, String>>,
    },
    QueryStats {
        now: i64,
        range_seconds: i64,
        reply: oneshot::Sender<Result<ApiProxyUsageStats, String>>,
    },
    Barrier {
        reply: oneshot::Sender<()>,
    },
    Clear {
        reply: oneshot::Sender<Result<(), String>>,
    },
}

impl ApiProxyUsageWriter {
    fn start(data_dir: PathBuf) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("api-proxy-usage-writer".to_string())
            .spawn(move || run_api_proxy_usage_writer(data_dir, receiver, ready_sender))
            .map_err(|error| format!("Failed to start API proxy usage writer: {error}"))?;

        // 只有数据库建表/迁移完成后才发布 sender；初始化失败时后续请求仍可重新尝试启动。
        match ready_receiver.recv() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                drop(sender);
                let _ = thread.join();
                return Err(error);
            }
            Err(error) => {
                drop(sender);
                let _ = thread.join();
                return Err(format!(
                    "API proxy usage writer initialization stopped: {error}"
                ));
            }
        }

        Ok(Self {
            sender: Some(sender),
            thread: Some(thread),
        })
    }

    fn sender(&self) -> Result<mpsc::Sender<ApiProxyUsageCommand>, String> {
        self.sender
            .as_ref()
            .cloned()
            .ok_or_else(|| "API proxy usage writer is closed".to_string())
    }
}

impl Drop for ApiProxyUsageWriter {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ApiProxyKeyStore {
    version: u8,
    updated_at: i64,
    keys: Vec<ApiProxyKey>,
}

impl Default for ApiProxyKeyStore {
    fn default() -> Self {
        Self {
            version: API_PROXY_KEYS_STORE_VERSION,
            updated_at: 0,
            keys: Vec::new(),
        }
    }
}

#[derive(Default)]
struct ApiProxyUsageSeriesAccumulator {
    total_calls: i64,
    total_tokens: i64,
    points: BTreeMap<i64, (i64, i64)>,
}

#[derive(Clone, Copy)]
struct ProxyLoadBalanceConfig {
    mode: ApiProxyLoadBalanceMode,
    sequential_five_hour_limit_percent: f64,
}

impl ProxyLoadBalanceConfig {
    fn from_settings(settings: &AppSettings) -> Self {
        Self {
            mode: settings.api_proxy_load_balance_mode,
            sequential_five_hour_limit_percent:
                normalize_api_proxy_sequential_five_hour_limit_percent(
                    settings.api_proxy_sequential_five_hour_limit_percent,
                ),
        }
    }
}

#[derive(Clone)]
struct ProxyContext {
    storage: ProxyStorageContext,
    api_keys: Arc<RwLock<Vec<ApiProxyKey>>>,
    upstream_base_url: String,
    client: reqwest::Client,
    shared: Arc<tokio::sync::Mutex<ApiProxyRuntimeSnapshot>>,
    request_body_limit_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HttpProxyConfig {
    host: String,
    port: u16,
    authorization: Option<String>,
    source: String,
}

type UpstreamByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, String>> + Send>>;

enum CodexUpstreamResponse {
    Http(reqwest::Response),
    WebSocket {
        headers: HeaderMap,
        stream: UpstreamByteStream,
    },
}

impl CodexUpstreamResponse {
    fn status(&self) -> StatusCode {
        match self {
            Self::Http(response) => response.status(),
            Self::WebSocket { .. } => StatusCode::OK,
        }
    }

    fn headers(&self) -> &HeaderMap {
        match self {
            Self::Http(response) => response.headers(),
            Self::WebSocket { headers, .. } => headers,
        }
    }

    fn into_stream(self) -> (HeaderMap, UpstreamByteStream) {
        match self {
            Self::Http(mut response) => {
                let headers = response.headers().clone();
                let output = stream! {
                    loop {
                        match response.chunk().await {
                            Ok(Some(chunk)) => yield Ok(chunk),
                            Ok(None) => break,
                            Err(error) => {
                                yield Err(error.to_string());
                                break;
                            }
                        }
                    }
                };
                (headers, Box::pin(output))
            }
            Self::WebSocket { headers, stream } => (headers, stream),
        }
    }

    async fn into_bytes(self) -> Result<(HeaderMap, Bytes), String> {
        match self {
            Self::Http(response) => {
                let headers = response.headers().clone();
                let body = response
                    .bytes()
                    .await
                    .map_err(|error| format!("读取上游响应失败: {error}"))?;
                Ok((headers, body))
            }
            Self::WebSocket {
                headers,
                mut stream,
            } => {
                let mut body = Vec::new();
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk?;
                    body.extend_from_slice(&chunk);
                }
                Ok((headers, Bytes::from(body)))
            }
        }
    }

    async fn into_bytes_limited(self, max_bytes: usize) -> Result<(HeaderMap, Bytes), String> {
        let (headers, mut stream) = self.into_stream();
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if chunk.len() > max_bytes.saturating_sub(body.len()) {
                return Err(format!("上游响应体超过限制（最大 {max_bytes} 字节）"));
            }
            body.extend_from_slice(&chunk);
        }
        Ok((headers, Bytes::from(body)))
    }
}

type CompactUpstreamResult = Result<(ProxyCandidate, Value, HeaderMap), CompactUpstreamFailure>;
type CompactUpstreamFuture = Pin<Box<dyn Future<Output = CompactUpstreamResult> + Send>>;

struct CompactUpstreamFailure {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
    message: String,
}

impl CompactUpstreamFailure {
    fn gateway(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            headers: HeaderMap::new(),
            body: Bytes::new(),
            message: message.into(),
        }
    }

    fn into_response(self) -> Response<Body> {
        if self.body.is_empty() {
            json_error_response(self.status, &self.message)
        } else {
            build_proxy_response(self.status, &self.headers, self.body)
        }
    }
}

struct ApiProxyHandleState {
    port: u16,
    api_keys: Arc<RwLock<Vec<ApiProxyKey>>>,
    task_finished: bool,
    shared: Arc<tokio::sync::Mutex<ApiProxyRuntimeSnapshot>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RetryFailureCategory {
    QuotaExceeded,
    RateLimited,
    ModelRestricted,
    Authentication,
    Permission,
}

struct RetryFailureInfo {
    category: RetryFailureCategory,
    detail: String,
}

#[derive(Default)]
struct SseDecoder {
    buffer: Vec<u8>,
}

#[derive(Debug, Clone)]
struct SseEvent {
    event: Option<String>,
    data: String,
}

struct ChatStreamState {
    response_id: String,
    created_at: i64,
    model: String,
    function_call_index: i64,
    has_received_arguments_delta: bool,
    has_tool_call_announced: bool,
}

impl Default for ChatStreamState {
    fn default() -> Self {
        Self {
            response_id: String::new(),
            created_at: 0,
            model: String::new(),
            // OpenAI tool call chunk indices are zero-based.
            function_call_index: -1,
            has_received_arguments_delta: false,
            has_tool_call_announced: false,
        }
    }
}

struct AnthropicStreamState {
    response_id: String,
    model: String,
    content_index: i64,
    open_text_index: Option<i64>,
    open_tool_index: Option<i64>,
    saw_tool_use: bool,
    has_received_tool_arguments_delta: bool,
    announced_tool_call_ids: HashSet<String>,
}

impl Default for AnthropicStreamState {
    fn default() -> Self {
        Self {
            response_id: String::new(),
            model: String::new(),
            // Anthropic content block indices are zero-based.
            content_index: -1,
            open_text_index: None,
            open_tool_index: None,
            saw_tool_use: false,
            has_received_tool_arguments_delta: false,
            announced_tool_call_ids: HashSet::new(),
        }
    }
}

pub(crate) fn new_proxy_storage_context(
    data_dir: PathBuf,
    store_lock: Arc<tokio::sync::Mutex<()>>,
    auth_refresh_lock: Arc<tokio::sync::Mutex<()>>,
    sync_active_auth_on_refresh: bool,
) -> ProxyStorageContext {
    ProxyStorageContext {
        data_dir,
        store_lock,
        auth_refresh_lock,
        usage_writer: Arc::new(tokio::sync::Mutex::new(None)),
        sync_active_auth_on_refresh,
    }
}

#[cfg(feature = "desktop")]
fn app_proxy_storage_context(
    app: &AppHandle,
    state: &AppState,
) -> Result<ProxyStorageContext, String> {
    let mut storage = new_proxy_storage_context(
        app_data_dir(app)?,
        state.store_lock.clone(),
        state.auth_operation_lock.clone(),
        true,
    );
    storage.usage_writer = state.api_proxy_usage_writer.clone();
    Ok(storage)
}

#[cfg(feature = "desktop")]
pub(crate) async fn get_api_proxy_status_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<ApiProxyStatus, String> {
    let storage = app_proxy_storage_context(app, state)?;
    get_api_proxy_status_with_runtime(&storage, &state.api_proxy).await
}

pub(crate) async fn get_api_proxy_status_with_runtime(
    storage: &ProxyStorageContext,
    runtime_slot: &tokio::sync::Mutex<Option<ApiProxyRuntimeHandle>>,
) -> Result<ApiProxyStatus, String> {
    let handle_state = {
        let guard = runtime_slot.lock().await;
        guard.as_ref().map(snapshot_handle_state)
    };

    match handle_state {
        Some(handle_state) => Ok(status_from_handle_state(handle_state).await),
        None => Ok(stopped_status(
            read_persisted_api_proxy_key(storage).await?,
            None,
        )),
    }
}

#[cfg(feature = "desktop")]
pub(crate) async fn list_api_proxy_keys_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<Vec<ApiProxyKey>, String> {
    let storage = app_proxy_storage_context(app, state)?;
    list_api_proxy_keys_with_storage(&storage).await
}

pub(crate) async fn list_api_proxy_keys_with_storage(
    storage: &ProxyStorageContext,
) -> Result<Vec<ApiProxyKey>, String> {
    ensure_persisted_api_proxy_keys(storage).await
}

#[cfg(feature = "desktop")]
pub(crate) async fn create_api_proxy_key_internal(
    app: &AppHandle,
    state: &AppState,
    input: CreateApiProxyKeyInput,
) -> Result<Vec<ApiProxyKey>, String> {
    let storage = app_proxy_storage_context(app, state)?;
    create_api_proxy_key_with_runtime(&storage, &state.api_proxy, input).await
}

pub(crate) async fn create_api_proxy_key_with_runtime(
    storage: &ProxyStorageContext,
    runtime_slot: &tokio::sync::Mutex<Option<ApiProxyRuntimeHandle>>,
    input: CreateApiProxyKeyInput,
) -> Result<Vec<ApiProxyKey>, String> {
    let now = now_unix_seconds();
    let label = normalized_api_proxy_key_label(&input.label);
    let key = normalized_custom_api_proxy_key(input.key.as_deref())?
        .unwrap_or_else(generate_api_proxy_key);
    let mut store = load_or_initialize_api_proxy_key_store(storage).await?;
    if store.keys.iter().any(|item| item.key == key) {
        return Err("平台 Key 已存在，请使用不同的 Key。".to_string());
    }
    store.keys.push(ApiProxyKey {
        id: uuid::Uuid::new_v4().to_string(),
        label,
        key,
        enabled: true,
        allowed_models: sanitize_api_proxy_allowed_models(input.allowed_models),
        allowed_reasoning_efforts: sanitize_api_proxy_reasoning_efforts(
            input.allowed_reasoning_efforts,
        ),
        allowed_service_tiers: sanitize_api_proxy_service_tiers(input.allowed_service_tiers),
        created_at: now,
        updated_at: now,
    });
    store.updated_at = now;
    save_api_proxy_key_store(storage, &store).await?;
    refresh_runtime_api_proxy_keys(storage, runtime_slot).await?;
    Ok(store.keys)
}

#[cfg(feature = "desktop")]
pub(crate) async fn update_api_proxy_key_internal(
    app: &AppHandle,
    state: &AppState,
    input: UpdateApiProxyKeyInput,
) -> Result<Vec<ApiProxyKey>, String> {
    let storage = app_proxy_storage_context(app, state)?;
    update_api_proxy_key_with_runtime(&storage, &state.api_proxy, input).await
}

pub(crate) async fn update_api_proxy_key_with_runtime(
    storage: &ProxyStorageContext,
    runtime_slot: &tokio::sync::Mutex<Option<ApiProxyRuntimeHandle>>,
    input: UpdateApiProxyKeyInput,
) -> Result<Vec<ApiProxyKey>, String> {
    let mut store = load_or_initialize_api_proxy_key_store(storage).await?;
    let key = store
        .keys
        .iter_mut()
        .find(|item| item.id == input.id)
        .ok_or_else(|| "平台 Key 不存在。".to_string())?;
    if let Some(label) = input.label {
        key.label = normalized_api_proxy_key_label(&label);
    }
    if let Some(enabled) = input.enabled {
        key.enabled = enabled;
    }
    if let Some(models) = input.allowed_models {
        key.allowed_models = sanitize_api_proxy_allowed_models(models);
    }
    if let Some(reasoning_efforts) = input.allowed_reasoning_efforts {
        key.allowed_reasoning_efforts = sanitize_api_proxy_reasoning_efforts(reasoning_efforts);
    }
    if let Some(service_tiers) = input.allowed_service_tiers {
        key.allowed_service_tiers = sanitize_api_proxy_service_tiers(service_tiers);
    }
    key.updated_at = now_unix_seconds();
    store.updated_at = key.updated_at;
    save_api_proxy_key_store(storage, &store).await?;
    refresh_runtime_api_proxy_keys(storage, runtime_slot).await?;
    Ok(store.keys)
}

#[cfg(feature = "desktop")]
pub(crate) async fn delete_api_proxy_key_internal(
    app: &AppHandle,
    state: &AppState,
    id: String,
) -> Result<Vec<ApiProxyKey>, String> {
    let storage = app_proxy_storage_context(app, state)?;
    delete_api_proxy_key_with_runtime(&storage, &state.api_proxy, &id).await
}

pub(crate) async fn delete_api_proxy_key_with_runtime(
    storage: &ProxyStorageContext,
    runtime_slot: &tokio::sync::Mutex<Option<ApiProxyRuntimeHandle>>,
    id: &str,
) -> Result<Vec<ApiProxyKey>, String> {
    let mut store = load_or_initialize_api_proxy_key_store(storage).await?;
    let before = store.keys.len();
    store.keys.retain(|key| key.id != id);
    if store.keys.len() == before {
        return Err("平台 Key 不存在。".to_string());
    }
    if store.keys.is_empty() {
        store
            .keys
            .push(default_api_proxy_key(now_unix_seconds(), None));
    }
    store.updated_at = now_unix_seconds();
    save_api_proxy_key_store(storage, &store).await?;
    refresh_runtime_api_proxy_keys(storage, runtime_slot).await?;
    Ok(store.keys)
}

#[cfg(feature = "desktop")]
pub(crate) async fn regenerate_api_proxy_key_internal(
    app: &AppHandle,
    state: &AppState,
    id: String,
) -> Result<Vec<ApiProxyKey>, String> {
    let storage = app_proxy_storage_context(app, state)?;
    regenerate_api_proxy_key_with_runtime(&storage, &state.api_proxy, Some(&id)).await
}

pub(crate) async fn regenerate_api_proxy_key_with_runtime(
    storage: &ProxyStorageContext,
    runtime_slot: &tokio::sync::Mutex<Option<ApiProxyRuntimeHandle>>,
    id: Option<&str>,
) -> Result<Vec<ApiProxyKey>, String> {
    let mut store = load_or_initialize_api_proxy_key_store(storage).await?;
    let target_index = match id {
        Some(id) => store
            .keys
            .iter()
            .position(|key| key.id == id)
            .ok_or_else(|| "平台 Key 不存在。".to_string())?,
        None => 0,
    };
    let mut new_key = generate_api_proxy_key();
    while store.keys.iter().any(|item| item.key == new_key) {
        new_key = generate_api_proxy_key();
    }
    let now = now_unix_seconds();
    store.keys[target_index].key = new_key;
    store.keys[target_index].updated_at = now;
    store.updated_at = now;
    save_api_proxy_key_store(storage, &store).await?;
    refresh_runtime_api_proxy_keys(storage, runtime_slot).await?;
    Ok(store.keys)
}

#[cfg(feature = "desktop")]
pub(crate) async fn get_api_proxy_key_usage_logs_internal(
    app: &AppHandle,
    state: &AppState,
    limit: Option<usize>,
) -> Result<Vec<ApiProxyKeyUsageLogEntry>, String> {
    let storage = app_proxy_storage_context(app, state)?;
    get_api_proxy_key_usage_logs_with_storage(&storage, limit).await
}

pub(crate) async fn get_api_proxy_key_usage_logs_with_storage(
    storage: &ProxyStorageContext,
    limit: Option<usize>,
) -> Result<Vec<ApiProxyKeyUsageLogEntry>, String> {
    let limit = limit.unwrap_or(200).clamp(1, 1_000);
    let sender = api_proxy_usage_sender(storage).await?;
    let (reply, response) = oneshot::channel();
    sender
        .send(ApiProxyUsageCommand::QueryKeyLogs { limit, reply })
        .map_err(|_| "API proxy usage writer is unavailable".to_string())?;
    response
        .await
        .map_err(|_| "API proxy usage query was interrupted".to_string())?
}

#[cfg(feature = "desktop")]
pub(crate) async fn get_api_proxy_usage_stats_internal(
    app: &AppHandle,
    state: &AppState,
    range_seconds: Option<i64>,
) -> Result<ApiProxyUsageStats, String> {
    let storage = app_proxy_storage_context(app, state)?;
    get_api_proxy_usage_stats_with_storage(&storage, range_seconds).await
}

pub(crate) async fn get_api_proxy_usage_stats_with_storage(
    storage: &ProxyStorageContext,
    range_seconds: Option<i64>,
) -> Result<ApiProxyUsageStats, String> {
    let now = now_unix_seconds();
    let range_seconds = normalize_api_proxy_usage_range_seconds(range_seconds);
    let sender = api_proxy_usage_sender(storage).await?;
    let (reply, response) = oneshot::channel();
    sender
        .send(ApiProxyUsageCommand::QueryStats {
            now,
            range_seconds,
            reply,
        })
        .map_err(|_| "API proxy usage writer is unavailable".to_string())?;
    response
        .await
        .map_err(|_| "API proxy usage query was interrupted".to_string())?
}

#[cfg(feature = "desktop")]
pub(crate) async fn export_api_proxy_usage_internal(
    app: &AppHandle,
    state: &AppState,
    range_seconds: Option<i64>,
    key_id: Option<String>,
    export_path: PathBuf,
) -> Result<(), String> {
    let storage = app_proxy_storage_context(app, state)?;
    export_api_proxy_usage_with_storage(&storage, range_seconds, key_id, export_path).await
}

pub(crate) async fn export_api_proxy_usage_with_storage(
    storage: &ProxyStorageContext,
    range_seconds: Option<i64>,
    key_id: Option<String>,
    export_path: PathBuf,
) -> Result<(), String> {
    // FIFO barrier 保证导出快照包含此前已入队的 append；长查询仍走独立只读连接，不阻塞写入线程。
    let sender = api_proxy_usage_sender(storage).await?;
    let (reply, response) = oneshot::channel();
    sender
        .send(ApiProxyUsageCommand::Barrier { reply })
        .map_err(|_| "API proxy usage writer is unavailable".to_string())?;
    response
        .await
        .map_err(|_| "API proxy usage export barrier was interrupted".to_string())?;
    let database_path = api_proxy_usage_database_path(&storage.data_dir);
    let now = now_unix_seconds();
    let range_seconds = normalize_api_proxy_usage_range_seconds(range_seconds);
    let key_id = key_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    tokio::task::spawn_blocking(move || {
        export_api_proxy_usage_csv(
            &database_path,
            &export_path,
            now,
            range_seconds,
            key_id.as_deref(),
        )
    })
    .await
    .map_err(|error| format!("API proxy usage export task failed: {error}"))?
}

#[cfg(feature = "desktop")]
pub(crate) async fn clear_api_proxy_usage_stats_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<(), String> {
    let storage = app_proxy_storage_context(app, state)?;
    clear_api_proxy_usage_stats_with_storage(&storage).await
}

pub(crate) async fn clear_api_proxy_usage_stats_with_storage(
    storage: &ProxyStorageContext,
) -> Result<(), String> {
    let sender = api_proxy_usage_sender(storage).await?;
    let (reply, response) = oneshot::channel();
    sender
        .send(ApiProxyUsageCommand::Clear { reply })
        .map_err(|_| "API proxy usage writer is unavailable".to_string())?;
    response
        .await
        .map_err(|_| "API proxy usage clear was interrupted".to_string())?
}

#[cfg(feature = "desktop")]
pub(crate) async fn start_api_proxy_internal(
    app: &AppHandle,
    state: &AppState,
    preferred_port: Option<u16>,
) -> Result<ApiProxyStatus, String> {
    let storage = app_proxy_storage_context(app, state)?;
    start_api_proxy_with_runtime(
        &storage,
        &state.api_proxy,
        preferred_port,
        DESKTOP_API_PROXY_BIND_HOST,
    )
    .await
}

pub(crate) async fn start_api_proxy_with_runtime(
    storage: &ProxyStorageContext,
    runtime_slot: &tokio::sync::Mutex<Option<ApiProxyRuntimeHandle>>,
    preferred_port: Option<u16>,
    bind_host: &str,
) -> Result<ApiProxyStatus, String> {
    let existing_handle = {
        let mut guard = runtime_slot.lock().await;
        if let Some(existing) = guard.as_ref() {
            if !existing.task.is_finished() {
                Some(snapshot_handle_state(existing))
            } else {
                guard.take();
                None
            }
        } else {
            None
        }
    };

    if let Some(existing_handle) = existing_handle {
        return Ok(status_from_handle_state(existing_handle).await);
    }

    let available_accounts = load_proxy_candidates(storage).await?;
    if available_accounts.is_empty() {
        return Err("暂无可用于代理的账号，请先添加并授权账号。".to_string());
    }

    let preferred_port = match preferred_port {
        Some(port) => port,
        None => read_persisted_api_proxy_port(storage).await?,
    };
    let listener = TcpListener::bind((bind_host, preferred_port))
        .await
        .map_err(|error| {
            format!("启动代理监听失败，端口 {preferred_port} 可能已被占用: {error}")
        })?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("读取代理端口失败: {error}"))?
        .port();
    let api_keys = ensure_persisted_api_proxy_keys(storage).await?;
    let shared_api_keys = Arc::new(RwLock::new(api_keys));

    let client = reqwest::Client::builder()
        .user_agent("codex-tools-proxy/0.1")
        .timeout(std::time::Duration::from_secs(
            DEFAULT_PROXY_UPSTREAM_TIMEOUT_SECS,
        ))
        .build()
        .map_err(|error| format!("创建代理 HTTP 客户端失败: {error}"))?;

    let shared = Arc::new(tokio::sync::Mutex::new(ApiProxyRuntimeSnapshot::default()));
    let request_body_limit = resolve_proxy_request_body_limit_bytes();
    let context = Arc::new(ProxyContext {
        storage: storage.clone(),
        api_keys: shared_api_keys.clone(),
        upstream_base_url: resolve_codex_upstream_base_url(),
        client,
        shared: shared.clone(),
        request_body_limit_bytes: request_body_limit,
    });

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let router = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/models", get(models_handler))
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/v1/images/generations", post(image_generations_handler))
        .route("/v1/images/edits", post(image_edits_handler))
        .route("/v1/images/variations", post(image_variations_handler))
        .route(
            "/v1/responses",
            post(responses_handler).get(responses_websocket_handler),
        )
        .route("/v1/responses/compact", post(responses_compact_handler))
        .route("/v1/messages", post(anthropic_messages_handler))
        .fallback(any(unsupported_proxy_handler))
        .layer(DefaultBodyLimit::max(request_body_limit))
        .with_state(context.clone());

    let task = tokio::spawn(async move {
        let server = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });

        if let Err(error) = server.await {
            let mut snapshot = context.shared.lock().await;
            snapshot.last_error = Some(format!("代理服务异常退出: {error}"));
        }
    });

    let handle = ApiProxyRuntimeHandle {
        port,
        api_keys: shared_api_keys,
        shutdown_tx: Some(shutdown_tx),
        task,
        shared,
    };
    let status = status_from_handle_state(snapshot_handle_state(&handle)).await;

    let mut guard = runtime_slot.lock().await;
    *guard = Some(handle);

    Ok(status)
}

#[cfg(feature = "desktop")]
pub(crate) async fn stop_api_proxy_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<ApiProxyStatus, String> {
    let storage = app_proxy_storage_context(app, state)?;
    stop_api_proxy_with_runtime(&storage, &state.api_proxy).await
}

pub(crate) async fn stop_api_proxy_with_runtime(
    storage: &ProxyStorageContext,
    runtime_slot: &tokio::sync::Mutex<Option<ApiProxyRuntimeHandle>>,
) -> Result<ApiProxyStatus, String> {
    let handle = {
        let mut guard = runtime_slot.lock().await;
        guard.take()
    };

    let Some(mut handle) = handle else {
        return Ok(stopped_status(
            read_persisted_api_proxy_key(storage).await?,
            None,
        ));
    };

    if let Some(shutdown_tx) = handle.shutdown_tx.take() {
        let _ = shutdown_tx.send(());
    }
    let _ = handle.task.await;

    let snapshot = handle.shared.lock().await.clone();
    Ok(stopped_status(
        read_current_api_key(&handle.api_keys),
        snapshot.last_error,
    ))
}

#[cfg(feature = "desktop")]
pub(crate) async fn refresh_api_proxy_key_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<ApiProxyStatus, String> {
    let storage = app_proxy_storage_context(app, state)?;
    refresh_api_proxy_key_with_runtime(&storage, &state.api_proxy).await
}

#[cfg(feature = "desktop")]
pub(crate) async fn bind_codex_to_api_proxy_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<ApiProxyStatus, String> {
    let status = get_api_proxy_status_internal(app, state).await?;
    let base_url = status
        .base_url
        .as_deref()
        .ok_or_else(|| "请先启动 API 反代，再绑定 Codex App/CLI。".to_string())?;
    let api_key = status
        .api_key
        .as_deref()
        .ok_or_else(|| "请先生成 API 反代 API Key，再绑定 Codex App/CLI。".to_string())?;

    profile_files::bind_codex_to_api_proxy(base_url, api_key)?;
    get_api_proxy_status_internal(app, state).await
}

#[cfg(feature = "desktop")]
pub(crate) async fn restore_codex_proxy_binding_internal(
    app: &AppHandle,
    state: &AppState,
) -> Result<ApiProxyStatus, String> {
    profile_files::restore_codex_proxy_binding()?;
    get_api_proxy_status_internal(app, state).await
}

pub(crate) async fn refresh_api_proxy_key_with_runtime(
    storage: &ProxyStorageContext,
    runtime_slot: &tokio::sync::Mutex<Option<ApiProxyRuntimeHandle>>,
) -> Result<ApiProxyStatus, String> {
    let keys = regenerate_api_proxy_key_with_runtime(storage, runtime_slot, None).await?;
    let new_api_key = primary_api_proxy_key_value(&keys);

    let handle_state = {
        let guard = runtime_slot.lock().await;
        guard.as_ref().map(snapshot_handle_state)
    };

    match handle_state {
        Some(handle_state) => Ok(status_from_handle_state(handle_state).await),
        None => Ok(stopped_status(new_api_key, None)),
    }
}

async fn health_handler() -> impl IntoResponse {
    Json(json!({ "ok": true }))
}

pub(crate) fn get_api_proxy_supported_models_internal() -> Vec<String> {
    MODELS.iter().map(|model| (*model).to_string()).collect()
}

pub(crate) fn sanitize_api_proxy_disabled_models_for_settings(models: Vec<String>) -> Vec<String> {
    let disabled = models
        .into_iter()
        .filter_map(|model| normalize_api_proxy_config_model_name(&model))
        .filter(|model| MODELS.contains(&model.as_str()))
        .collect::<HashSet<_>>();

    MODELS
        .iter()
        .filter(|model| disabled.contains(**model))
        .map(|model| (*model).to_string())
        .collect()
}

async fn models_handler(
    State(context): State<Arc<ProxyContext>>,
    headers: HeaderMap,
) -> Response<Body> {
    let proxy_key = match authorize_proxy_request(&headers, &context.api_keys) {
        Ok(proxy_key) => proxy_key,
        Err(response) => return response,
    };

    let settings = match load_api_proxy_settings(&context.storage).await {
        Ok(settings) => settings,
        Err(error) => {
            update_proxy_error(&context, Some(error.clone())).await;
            return json_error_response(StatusCode::BAD_GATEWAY, &error);
        }
    };

    Json(json!({
        "object": "list",
        "data": api_proxy_visible_models_for_key(&settings, &proxy_key)
            .iter()
            .map(|model| {
                json!({
                    "id": model,
                    "object": "model",
                    "created": 0,
                    "owned_by": "openai",
                })
            })
            .collect::<Vec<_>>(),
    }))
    .into_response()
}

async fn chat_completions_handler(
    State(context): State<Arc<ProxyContext>>,
    request: Request,
) -> Response<Body> {
    let headers = request.headers().clone();
    let proxy_key = match authorize_proxy_request(&headers, &context.api_keys) {
        Ok(proxy_key) => proxy_key,
        Err(response) => return response,
    };
    let body = match read_bounded_proxy_request_body(
        request.into_body(),
        context.request_body_limit_bytes,
    )
    .await
    {
        Ok(body) => body,
        Err(response) => return response,
    };

    let request_json =
        match parse_json_request_async(headers.clone(), body, context.request_body_limit_bytes)
            .await
        {
            Ok(value) => value,
            Err(response) => return response,
        };
    let session_affinity_key = request_session_affinity_key(&headers, &request_json);
    let request_specifies_tier = request_json.get("service_tier").is_some();

    let default_service_tier = resolve_proxy_default_service_tier();
    let (mut upstream_payload, downstream_stream) =
        match convert_openai_chat_request_to_codex(&request_json) {
            Ok(value) => value,
            Err(message) => return invalid_request_response(&message),
        };
    apply_configured_default_service_tier(
        request_specifies_tier,
        &mut upstream_payload,
        &default_service_tier,
    );

    let upstream = match send_codex_request_over_candidates(
        &context,
        &proxy_key,
        "/v1/chat/completions",
        CodexUpstreamEndpoint::Responses,
        &headers,
        &upstream_payload,
        session_affinity_key.as_deref(),
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return response,
    };

    let (candidate, upstream_response) = upstream;
    update_proxy_target(&context, &candidate).await;
    update_proxy_error(&context, None).await;
    let usage_metadata = api_proxy_usage_metadata(
        &proxy_key,
        &candidate,
        "/v1/chat/completions",
        &upstream_payload,
    );

    if downstream_stream {
        build_chat_streaming_response(upstream_response, context.storage.clone(), usage_metadata)
    } else {
        let (upstream_headers, upstream_body) = match upstream_response.into_bytes().await {
            Ok(value) => value,
            Err(error) => {
                let message = format!("读取 Codex 上游响应失败: {error}");
                update_proxy_error(&context, Some(message.clone())).await;
                return json_error_response(StatusCode::BAD_GATEWAY, &message);
            }
        };

        let completed = match extract_completed_response_from_sse(&upstream_body) {
            Ok(value) => value,
            Err(message) => {
                update_proxy_error(&context, Some(message.clone())).await;
                return json_error_response(StatusCode::BAD_GATEWAY, &message);
            }
        };
        record_api_proxy_tokens_from_response(&context.storage, &usage_metadata, &completed).await;

        let body =
            match serde_json::to_vec(&convert_completed_response_to_chat_completion(&completed)) {
                Ok(bytes) => Bytes::from(bytes),
                Err(error) => {
                    let message = format!("序列化聊天响应失败: {error}");
                    update_proxy_error(&context, Some(message.clone())).await;
                    return json_error_response(StatusCode::BAD_GATEWAY, &message);
                }
            };

        build_json_proxy_response(StatusCode::OK, &upstream_headers, body)
    }
}

async fn responses_handler(
    State(context): State<Arc<ProxyContext>>,
    request: Request,
) -> Response<Body> {
    let headers = request.headers().clone();
    let proxy_key = match authorize_proxy_request(&headers, &context.api_keys) {
        Ok(proxy_key) => proxy_key,
        Err(response) => return response,
    };
    let body = match read_bounded_proxy_request_body(
        request.into_body(),
        context.request_body_limit_bytes,
    )
    .await
    {
        Ok(body) => body,
        Err(response) => return response,
    };

    let request_json =
        match parse_json_request_async(headers.clone(), body, context.request_body_limit_bytes)
            .await
        {
            Ok(value) => value,
            Err(response) => return response,
        };

    let session_affinity_key = request_session_affinity_key(&headers, &request_json);
    let request_specifies_tier = request_json.get("service_tier").is_some();

    let default_service_tier = resolve_proxy_default_service_tier();
    let (mut upstream_payload, downstream_stream) =
        match normalize_openai_responses_request(request_json) {
            Ok(value) => value,
            Err(message) => return invalid_request_response(&message),
        };
    apply_configured_default_service_tier(
        request_specifies_tier,
        &mut upstream_payload,
        &default_service_tier,
    );

    let upstream = match send_codex_request_over_candidates(
        &context,
        &proxy_key,
        "/v1/responses",
        CodexUpstreamEndpoint::Responses,
        &headers,
        &upstream_payload,
        session_affinity_key.as_deref(),
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return response,
    };

    let (candidate, upstream_response) = upstream;
    update_proxy_target(&context, &candidate).await;
    update_proxy_error(&context, None).await;
    let usage_metadata =
        api_proxy_usage_metadata(&proxy_key, &candidate, "/v1/responses", &upstream_payload);

    if downstream_stream {
        build_passthrough_sse_response(upstream_response, context.storage.clone(), usage_metadata)
    } else {
        let (upstream_headers, upstream_body) = match upstream_response.into_bytes().await {
            Ok(value) => value,
            Err(error) => {
                let message = format!("读取 Codex 上游响应失败: {error}");
                update_proxy_error(&context, Some(message.clone())).await;
                return json_error_response(StatusCode::BAD_GATEWAY, &message);
            }
        };

        let completed = match extract_completed_response_from_sse(&upstream_body) {
            Ok(value) => value,
            Err(message) => {
                update_proxy_error(&context, Some(message.clone())).await;
                return json_error_response(StatusCode::BAD_GATEWAY, &message);
            }
        };
        record_api_proxy_tokens_from_response(&context.storage, &usage_metadata, &completed).await;

        let completed = rewrite_response_models_for_client(completed);
        let body = match serde_json::to_vec(&completed) {
            Ok(bytes) => Bytes::from(bytes),
            Err(error) => {
                let message = format!("序列化 responses 响应失败: {error}");
                update_proxy_error(&context, Some(message.clone())).await;
                return json_error_response(StatusCode::BAD_GATEWAY, &message);
            }
        };

        build_json_proxy_response(StatusCode::OK, &upstream_headers, body)
    }
}

async fn responses_compact_handler(
    State(context): State<Arc<ProxyContext>>,
    request: Request,
) -> Response<Body> {
    let headers = request.headers().clone();
    let proxy_key = match authorize_proxy_request(&headers, &context.api_keys) {
        Ok(proxy_key) => proxy_key,
        Err(response) => return response,
    };
    let body = match read_bounded_proxy_request_body(
        request.into_body(),
        context.request_body_limit_bytes,
    )
    .await
    {
        Ok(body) => body,
        Err(response) => return response,
    };

    let request_json =
        match parse_json_request_async(headers.clone(), body, context.request_body_limit_bytes)
            .await
        {
            Ok(value) => value,
            Err(response) => return response,
        };
    let client_wants_stream = request_json
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    handle_responses_compact(
        context,
        proxy_key,
        headers,
        request_json,
        client_wants_stream,
        "/v1/responses/compact",
    )
    .await
}

async fn handle_responses_compact(
    context: Arc<ProxyContext>,
    proxy_key: ApiProxyKey,
    headers: HeaderMap,
    request_json: Value,
    client_wants_stream: bool,
    route: &'static str,
) -> Response<Body> {
    let session_affinity_key = request_session_affinity_key(&headers, &request_json);
    let request_specifies_tier = request_json.get("service_tier").is_some();

    let default_service_tier = resolve_proxy_default_service_tier();
    let mut upstream_payload = match normalize_openai_compact_request(request_json) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&message),
    };
    apply_configured_default_service_tier(
        request_specifies_tier,
        &mut upstream_payload,
        &default_service_tier,
    );

    if client_wants_stream {
        return stream_compact_response_with_optional_keepalive(
            context,
            proxy_key,
            headers,
            upstream_payload,
            session_affinity_key,
            route,
        )
        .await;
    }

    match fetch_compact_upstream_response(
        &context,
        &proxy_key,
        route,
        &headers,
        &upstream_payload,
        session_affinity_key.as_deref(),
    )
    .await
    {
        Ok((candidate, compact_response, upstream_headers)) => {
            finish_compact_success(
                &context,
                &proxy_key,
                route,
                &upstream_payload,
                candidate,
                compact_response,
                upstream_headers,
                false,
            )
            .await
        }
        Err(error) => {
            update_proxy_error(&context, Some(error.message.clone())).await;
            error.into_response()
        }
    }
}

async fn stream_compact_response_with_optional_keepalive(
    context: Arc<ProxyContext>,
    proxy_key: ApiProxyKey,
    headers: HeaderMap,
    upstream_payload: Value,
    session_affinity_key: Option<String>,
    route: &'static str,
) -> Response<Body> {
    let context_for_work = context.clone();
    let proxy_key_for_work = proxy_key.clone();
    let headers_for_work = headers.clone();
    let payload_for_work = upstream_payload.clone();
    let incoming_turn_state = headers.get(CODEX_TURN_STATE_HEADER).cloned();
    // 直接把 future 移交给响应流；客户端断开并丢弃 Body 时 future 同步取消，不留下 detached 上游任务。
    let mut work: CompactUpstreamFuture = Box::pin(async move {
        fetch_compact_upstream_response(
            &context_for_work,
            &proxy_key_for_work,
            route,
            &headers_for_work,
            &payload_for_work,
            session_affinity_key.as_deref(),
        )
        .await
    });

    // 没有可安全回显的 turn-state 时必须等上游 headers，不能提前提交一个缺失状态的 SSE 响应。
    if incoming_turn_state.is_none() {
        return finish_initial_compact_result(
            &context,
            &proxy_key,
            route,
            &upstream_payload,
            work.await,
        )
        .await;
    }

    // First beat is delayed so fast failures can still return JSON status codes.
    tokio::select! {
        result = &mut work => {
            finish_initial_compact_result(
                &context,
                &proxy_key,
                route,
                &upstream_payload,
                result,
            ).await
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(COMPACT_SSE_KEEPALIVE_INTERVAL_SECS)) => {
            build_compact_keepalive_stream_response(
                context,
                proxy_key,
                upstream_payload,
                route,
                work,
                incoming_turn_state.expect("turn state checked above"),
            )
        }
    }
}

async fn finish_initial_compact_result(
    context: &ProxyContext,
    proxy_key: &ApiProxyKey,
    route: &str,
    upstream_payload: &Value,
    result: CompactUpstreamResult,
) -> Response<Body> {
    match result {
        Ok((candidate, compact_response, upstream_headers)) => {
            finish_compact_success(
                context,
                proxy_key,
                route,
                upstream_payload,
                candidate,
                compact_response,
                upstream_headers,
                true,
            )
            .await
        }
        Err(error) => {
            update_proxy_error(context, Some(error.message.clone())).await;
            error.into_response()
        }
    }
}

fn build_compact_keepalive_stream_response(
    context: Arc<ProxyContext>,
    proxy_key: ApiProxyKey,
    upstream_payload: Value,
    route: &'static str,
    mut work: CompactUpstreamFuture,
    turn_state: HeaderValue,
) -> Response<Body> {
    let output = stream! {
        yield Ok::<Bytes, Infallible>(Bytes::from_static(COMPACT_SSE_KEEPALIVE.as_bytes()));

        let mut keepalive = tokio::time::interval_at(
            tokio::time::Instant::now()
                + std::time::Duration::from_secs(COMPACT_SSE_KEEPALIVE_INTERVAL_SECS),
            std::time::Duration::from_secs(COMPACT_SSE_KEEPALIVE_INTERVAL_SECS),
        );
        keepalive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let result = loop {
            tokio::select! {
                result = &mut work => break result,
                _ = keepalive.tick() => {
                    yield Ok(Bytes::from_static(COMPACT_SSE_KEEPALIVE.as_bytes()));
                }
            }
        };

        match result {
            Ok((candidate, compact_response, _headers)) => {
                update_proxy_target(&context, &candidate).await;
                update_proxy_error(&context, None).await;
                let usage_metadata =
                    api_proxy_usage_metadata(&proxy_key, &candidate, route, &upstream_payload);
                record_api_proxy_tokens_from_response(
                    &context.storage,
                    &usage_metadata,
                    &compact_response,
                )
                .await;

                let compact_response = rewrite_response_models_for_client(compact_response);
                match build_compact_sse_response(&compact_response) {
                    Ok(sse_body) => yield Ok(sse_body),
                    Err(message) => {
                        update_proxy_error(&context, Some(message.clone())).await;
                        yield Ok(build_compact_sse_failure(&message));
                    }
                }
            }
            Err(error) => {
                update_proxy_error(&context, Some(error.message.clone())).await;
                yield Ok(build_compact_sse_failure_from_upstream(&error));
            }
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(CODEX_TURN_STATE_HEADER, turn_state)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .header("connection", "keep-alive")
        .header("x-accel-buffering", "no")
        .body(Body::from_stream(output))
        .unwrap_or_else(|_| {
            json_error_response(StatusCode::BAD_GATEWAY, "构建 compact SSE 响应失败")
        })
}

async fn finish_compact_success(
    context: &ProxyContext,
    proxy_key: &ApiProxyKey,
    route: &str,
    upstream_payload: &Value,
    candidate: ProxyCandidate,
    compact_response: Value,
    upstream_headers: HeaderMap,
    as_stream: bool,
) -> Response<Body> {
    update_proxy_target(context, &candidate).await;
    update_proxy_error(context, None).await;
    let usage_metadata = api_proxy_usage_metadata(proxy_key, &candidate, route, upstream_payload);
    record_api_proxy_tokens_from_response(&context.storage, &usage_metadata, &compact_response)
        .await;

    let compact_response = rewrite_response_models_for_client(compact_response);
    if as_stream {
        match build_compact_sse_response(&compact_response) {
            Ok(sse_body) => build_sse_bytes_response(StatusCode::OK, &upstream_headers, sse_body),
            Err(message) => {
                update_proxy_error(context, Some(message.clone())).await;
                json_error_response(StatusCode::BAD_GATEWAY, &message)
            }
        }
    } else {
        let body = match serde_json::to_vec(&compact_response) {
            Ok(bytes) => Bytes::from(bytes),
            Err(error) => {
                let message = format!("序列化 compact 响应失败: {error}");
                update_proxy_error(context, Some(message.clone())).await;
                return json_error_response(StatusCode::BAD_GATEWAY, &message);
            }
        };
        build_json_proxy_response(StatusCode::OK, &upstream_headers, body)
    }
}

async fn fetch_compact_upstream_response(
    context: &ProxyContext,
    proxy_key: &ApiProxyKey,
    route: &str,
    headers: &HeaderMap,
    upstream_payload: &Value,
    session_affinity_key: Option<&str>,
) -> CompactUpstreamResult {
    let (candidate, upstream_response) = match send_codex_request_over_candidates(
        context,
        proxy_key,
        route,
        CodexUpstreamEndpoint::Compact,
        headers,
        upstream_payload,
        session_affinity_key,
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return Err(compact_upstream_failure_from_response(response).await),
    };

    let (upstream_headers, upstream_body) = upstream_response
        .into_bytes_limited(MAX_COMPACT_RESPONSE_BODY_BYTES)
        .await
        .map_err(|error| {
            CompactUpstreamFailure::gateway(format!("读取 Codex compact 上游响应失败: {error}"))
        })?;
    let compact_response =
        parse_compact_upstream_response(&upstream_body).map_err(CompactUpstreamFailure::gateway)?;
    Ok((candidate, compact_response, upstream_headers))
}

async fn compact_upstream_failure_from_response(
    response: Response<Body>,
) -> CompactUpstreamFailure {
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), MAX_COMPACT_ERROR_BODY_BYTES)
        .await
        .unwrap_or_default();
    let message = proxy_response_error_message(status, &body);
    CompactUpstreamFailure {
        status,
        headers,
        body,
        message,
    }
}

fn proxy_response_error_message(status: StatusCode, body: &[u8]) -> String {
    if let Ok(value) = serde_json::from_slice::<Value>(&body) {
        if let Some(message) = response_error_message_from_value(&value) {
            return message;
        }
        if let Some(message) = value
            .pointer("/error/message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return message.to_string();
        }
    }
    let text = String::from_utf8_lossy(&body).trim().to_string();
    if !text.is_empty() {
        return truncate_for_error(&text, 240);
    }
    format!("compact 上游请求失败: HTTP {}", status.as_u16())
}

fn build_compact_sse_failure(message: &str) -> Bytes {
    build_compact_sse_failure_with_error(json!({
        "code": "upstream_error",
        "message": message,
    }))
}

fn build_compact_sse_failure_from_upstream(error: &CompactUpstreamFailure) -> Bytes {
    let mut error_value = serde_json::from_slice::<Value>(&error.body)
        .ok()
        .and_then(|value| value.get("error").cloned())
        .filter(Value::is_object)
        .unwrap_or_else(|| Value::Object(Map::new()));
    if let Some(object) = error_value.as_object_mut() {
        object.entry("code".to_string()).or_insert_with(|| {
            Value::String(
                match error.status {
                    StatusCode::BAD_REQUEST => "invalid_request_error",
                    StatusCode::UNAUTHORIZED => "invalid_api_key",
                    StatusCode::FORBIDDEN => "permission_denied",
                    StatusCode::TOO_MANY_REQUESTS => "rate_limit_exceeded",
                    StatusCode::PAYLOAD_TOO_LARGE => "request_too_large",
                    _ => "upstream_error",
                }
                .to_string(),
            )
        });
        object
            .entry("message".to_string())
            .or_insert_with(|| Value::String(error.message.clone()));
    }
    build_compact_sse_failure_with_error(error_value)
}

fn build_compact_sse_failure_with_error(error: Value) -> Bytes {
    let payload = json!({
        "type": "response.failed",
        "response": {
            "id": format!("resp_{}", uuid::Uuid::new_v4().simple()),
            "object": "response",
            "status": "failed",
            "output": [],
            "error": error,
        }
    });
    let data = serde_json::to_string(&payload).unwrap_or_else(|_| {
        r#"{"type":"response.failed","response":{"status":"failed","output":[],"error":{"code":"upstream_error","message":"compact failed"}}}"#.to_string()
    });
    Bytes::from(format!("event: response.failed\ndata: {data}\n\n"))
}

async fn anthropic_messages_handler(
    State(context): State<Arc<ProxyContext>>,
    request: Request,
) -> Response<Body> {
    let headers = request.headers().clone();
    let proxy_key = match authorize_anthropic_proxy_request(&headers, &context.api_keys) {
        Ok(proxy_key) => proxy_key,
        Err(response) => return response,
    };
    let body = match read_bounded_proxy_request_body(
        request.into_body(),
        context.request_body_limit_bytes,
    )
    .await
    {
        Ok(body) => body,
        Err(response) => return response,
    };

    let request_json =
        match parse_json_request_async(headers.clone(), body, context.request_body_limit_bytes)
            .await
        {
            Ok(value) => value,
            Err(response) => return response,
        };
    let session_affinity_key = request_session_affinity_key(&headers, &request_json);
    let headers = apply_anthropic_upstream_session_id(headers, &request_json);
    let request_specifies_tier = request_json.get("service_tier").is_some();

    let default_service_tier = resolve_proxy_default_service_tier();
    let (mut upstream_payload, downstream_stream) =
        match convert_anthropic_messages_request_to_codex(&request_json) {
            Ok(value) => value,
            Err(message) => return invalid_request_response(&message),
        };
    apply_configured_default_service_tier(
        request_specifies_tier,
        &mut upstream_payload,
        &default_service_tier,
    );

    let upstream = match send_codex_request_over_candidates(
        &context,
        &proxy_key,
        "/v1/messages",
        CodexUpstreamEndpoint::Responses,
        &headers,
        &upstream_payload,
        session_affinity_key.as_deref(),
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return response,
    };

    let (candidate, upstream_response) = upstream;
    update_proxy_target(&context, &candidate).await;
    update_proxy_error(&context, None).await;
    let usage_metadata =
        api_proxy_usage_metadata(&proxy_key, &candidate, "/v1/messages", &upstream_payload);

    if downstream_stream {
        build_anthropic_streaming_response(
            upstream_response,
            context.storage.clone(),
            usage_metadata,
        )
    } else {
        let (upstream_headers, upstream_body) = match upstream_response.into_bytes().await {
            Ok(value) => value,
            Err(error) => {
                let message = format!("读取 Codex 上游响应失败: {error}");
                update_proxy_error(&context, Some(message.clone())).await;
                return json_error_response(StatusCode::BAD_GATEWAY, &message);
            }
        };

        let completed = match extract_completed_response_from_sse(&upstream_body) {
            Ok(value) => value,
            Err(message) => {
                update_proxy_error(&context, Some(message.clone())).await;
                return json_error_response(StatusCode::BAD_GATEWAY, &message);
            }
        };
        record_api_proxy_tokens_from_response(&context.storage, &usage_metadata, &completed).await;

        let body = match serde_json::to_vec(&convert_completed_response_to_anthropic_message(
            &completed,
        )) {
            Ok(bytes) => Bytes::from(bytes),
            Err(error) => {
                let message = format!("序列化 Anthropic Messages 响应失败: {error}");
                update_proxy_error(&context, Some(message.clone())).await;
                return json_error_response(StatusCode::BAD_GATEWAY, &message);
            }
        };

        build_json_proxy_response(StatusCode::OK, &upstream_headers, body)
    }
}

async fn image_generations_handler(
    State(context): State<Arc<ProxyContext>>,
    request: Request,
) -> Response<Body> {
    let headers = request.headers().clone();
    let proxy_key = match authorize_proxy_request(&headers, &context.api_keys) {
        Ok(proxy_key) => proxy_key,
        Err(response) => return response,
    };
    let body = match read_bounded_proxy_request_body(
        request.into_body(),
        context.request_body_limit_bytes,
    )
    .await
    {
        Ok(body) => body,
        Err(response) => return response,
    };

    let request_json =
        match parse_json_request_async(headers.clone(), body, context.request_body_limit_bytes)
            .await
        {
            Ok(value) => value,
            Err(response) => return response,
        };
    let session_affinity_key = request_session_affinity_key(&headers, &request_json);

    let image_request = match convert_openai_image_generation_request_to_codex(&request_json) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&message),
    };

    forward_image_request(
        context,
        proxy_key,
        "/v1/images/generations",
        headers,
        image_request,
        session_affinity_key.as_deref(),
    )
    .await
}

async fn image_edits_handler(
    State(context): State<Arc<ProxyContext>>,
    request: Request,
) -> Response<Body> {
    let headers = request.headers().clone();
    let proxy_key = match authorize_proxy_request(&headers, &context.api_keys) {
        Ok(proxy_key) => proxy_key,
        Err(response) => return response,
    };
    let multipart = match Multipart::from_request(request, &context).await {
        Ok(multipart) => multipart,
        Err(rejection) => return rejection.into_response(),
    };

    let request = match parse_image_multipart_request(multipart).await {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&message),
    };
    let session_affinity_key = request_session_affinity_key_from_map(&headers, &request.fields);
    let image_request = match convert_openai_image_edit_request_to_codex(&request, false) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&message),
    };

    forward_image_request(
        context,
        proxy_key,
        "/v1/images/edits",
        headers,
        image_request,
        session_affinity_key.as_deref(),
    )
    .await
}

async fn image_variations_handler(
    State(context): State<Arc<ProxyContext>>,
    request: Request,
) -> Response<Body> {
    let headers = request.headers().clone();
    let proxy_key = match authorize_proxy_request(&headers, &context.api_keys) {
        Ok(proxy_key) => proxy_key,
        Err(response) => return response,
    };
    let multipart = match Multipart::from_request(request, &context).await {
        Ok(multipart) => multipart,
        Err(rejection) => return rejection.into_response(),
    };

    let request = match parse_image_multipart_request(multipart).await {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&message),
    };
    let session_affinity_key = request_session_affinity_key_from_map(&headers, &request.fields);
    let image_request = match convert_openai_image_edit_request_to_codex(&request, true) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&message),
    };

    forward_image_request(
        context,
        proxy_key,
        "/v1/images/variations",
        headers,
        image_request,
        session_affinity_key.as_deref(),
    )
    .await
}

async fn forward_image_request(
    context: Arc<ProxyContext>,
    proxy_key: ApiProxyKey,
    route: &'static str,
    headers: HeaderMap,
    image_request: ConvertedImageRequest,
    session_affinity_key: Option<&str>,
) -> Response<Body> {
    let ConvertedImageRequest {
        upstream_payload,
        downstream_stream,
        image_count,
    } = image_request;

    let upstream = match send_codex_request_over_candidates(
        &context,
        &proxy_key,
        route,
        CodexUpstreamEndpoint::Responses,
        &headers,
        &upstream_payload,
        session_affinity_key,
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return response,
    };

    let (candidate, upstream_response) = upstream;
    update_proxy_target(&context, &candidate).await;
    update_proxy_error(&context, None).await;
    let usage_metadata = api_proxy_usage_metadata(&proxy_key, &candidate, route, &upstream_payload);

    if downstream_stream {
        build_image_streaming_response(upstream_response, context.storage.clone(), usage_metadata)
    } else {
        let mut upstream_headers = HeaderMap::new();
        let mut created = now_unix_seconds();
        let mut data = Vec::new();
        let mut first_upstream = Some((candidate, upstream_response));

        for index in 0..image_count {
            let (candidate, upstream_response) = if index == 0 {
                first_upstream
                    .take()
                    .expect("first image upstream response should be present")
            } else {
                let upstream = match send_codex_request_over_candidates(
                    &context,
                    &proxy_key,
                    route,
                    CodexUpstreamEndpoint::Responses,
                    &headers,
                    &upstream_payload,
                    session_affinity_key,
                )
                .await
                {
                    Ok(value) => value,
                    Err(response) => return response,
                };
                let (candidate, upstream_response) = upstream;
                update_proxy_target(&context, &candidate).await;
                update_proxy_error(&context, None).await;
                (candidate, upstream_response)
            };

            let (headers, upstream_body) = match upstream_response.into_bytes().await {
                Ok(value) => value,
                Err(error) => {
                    let message = format!("读取 Codex 上游响应失败: {error}");
                    update_proxy_error(&context, Some(message.clone())).await;
                    return json_error_response(StatusCode::BAD_GATEWAY, &message);
                }
            };
            upstream_headers = headers;

            let completed = match extract_completed_response_from_sse(&upstream_body) {
                Ok(value) => value,
                Err(message) => {
                    update_proxy_error(&context, Some(message.clone())).await;
                    return json_error_response(StatusCode::BAD_GATEWAY, &message);
                }
            };
            let usage_metadata =
                api_proxy_usage_metadata(&proxy_key, &candidate, route, &upstream_payload);
            record_api_proxy_tokens_from_response(&context.storage, &usage_metadata, &completed)
                .await;
            let converted = match convert_responses_image_output_to_images_response(&completed) {
                Ok(value) => value,
                Err(message) => {
                    update_proxy_error(&context, Some(message.clone())).await;
                    return json_error_response(StatusCode::BAD_GATEWAY, &message);
                }
            };
            if index == 0 {
                created = converted
                    .get("created")
                    .and_then(Value::as_i64)
                    .unwrap_or(created);
            }
            if let Some(items) = converted.get("data").and_then(Value::as_array) {
                data.extend(items.iter().cloned());
            }
        }

        let body = match serde_json::to_vec(&json!({
            "created": created,
            "data": data,
        }))
        .map(Bytes::from)
        .map_err(|error| format!("序列化图片响应失败: {error}"))
        {
            Ok(value) => value,
            Err(message) => {
                update_proxy_error(&context, Some(message.clone())).await;
                return json_error_response(StatusCode::BAD_GATEWAY, &message);
            }
        };

        build_json_proxy_response(StatusCode::OK, &upstream_headers, body)
    }
}

async fn responses_websocket_handler(
    State(context): State<Arc<ProxyContext>>,
    headers: HeaderMap,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response<Body> {
    let proxy_key = match authorize_proxy_request(&headers, &context.api_keys) {
        Ok(proxy_key) => proxy_key,
        Err(response) => return response,
    };

    // Codex 0.144 WebSocket v2 uses a generate=false warmup and then reuses the
    // connection with previous_response_id. This proxy intentionally forwards
    // Responses over HTTP, so return 426 and let the official client use its
    // built-in HTTP fallback instead of accepting a stateful protocol we cannot
    // preserve end to end.
    if responses_websocket_requires_http_fallback(&headers) {
        return json_error_response(
            StatusCode::UPGRADE_REQUIRED,
            "Responses WebSocket v2 is served through the HTTP fallback.",
        );
    }

    let Ok(ws) = ws else {
        return json_error_response(
            StatusCode::UPGRADE_REQUIRED,
            "GET /v1/responses requires a WebSocket upgrade.",
        );
    };

    ws.on_upgrade(move |socket| handle_responses_websocket(socket, context, proxy_key, headers))
        .into_response()
}

fn responses_websocket_requires_http_fallback(headers: &HeaderMap) -> bool {
    headers
        .get_all("openai-beta")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|value| value.trim().eq_ignore_ascii_case(RESPONSES_WEBSOCKETS_BETA))
}

async fn handle_responses_websocket(
    mut socket: AxumWebSocket,
    context: Arc<ProxyContext>,
    proxy_key: ApiProxyKey,
    headers: HeaderMap,
) {
    let session_affinity_key = request_session_affinity_key_from_headers(&headers);
    let (mut upstream_payload, request_specifies_tier) =
        match receive_responses_websocket_create(&mut socket).await {
            Ok(value) => value,
            Err(message) => {
                let _ = send_responses_websocket_error(&mut socket, &message).await;
                let _ = socket.close().await;
                return;
            }
        };
    let session_affinity_key = session_affinity_key
        .or_else(|| request_session_affinity_key_from_payload(&upstream_payload));
    let default_service_tier = resolve_proxy_default_service_tier();
    apply_configured_default_service_tier(
        request_specifies_tier,
        &mut upstream_payload,
        &default_service_tier,
    );

    let upstream = match send_codex_request_over_candidates(
        &context,
        &proxy_key,
        "/v1/responses websocket",
        CodexUpstreamEndpoint::Responses,
        &headers,
        &upstream_payload,
        session_affinity_key.as_deref(),
    )
    .await
    {
        Ok(value) => value,
        Err(response) => {
            let message = format!(
                "Codex upstream request failed with status {}",
                response.status()
            );
            let _ = send_responses_websocket_error(&mut socket, &message).await;
            let _ = socket.close().await;
            return;
        }
    };

    let (candidate, upstream_response) = upstream;
    update_proxy_target(&context, &candidate).await;
    update_proxy_error(&context, None).await;
    let usage_metadata = api_proxy_usage_metadata(
        &proxy_key,
        &candidate,
        "/v1/responses websocket",
        &upstream_payload,
    );

    if let Err(message) = relay_responses_sse_to_websocket(
        &mut socket,
        upstream_response,
        context.storage.clone(),
        usage_metadata,
    )
    .await
    {
        update_proxy_error(&context, Some(message.clone())).await;
        let _ = send_responses_websocket_error(&mut socket, &message).await;
    }

    let _ = socket.close().await;
}

async fn receive_responses_websocket_create(
    socket: &mut AxumWebSocket,
) -> Result<(Value, bool), String> {
    while let Some(message) = socket.recv().await {
        let message = message.map_err(|error| format!("读取 WebSocket 首帧失败: {error}"))?;
        match message {
            AxumWebSocketMessage::Text(text) => {
                return normalize_responses_websocket_create(text.as_bytes())
            }
            AxumWebSocketMessage::Binary(bytes) => {
                return normalize_responses_websocket_create(&bytes)
            }
            AxumWebSocketMessage::Close(_) => {
                return Err("WebSocket 在发送 response.create 前已关闭".to_string())
            }
            AxumWebSocketMessage::Ping(_) | AxumWebSocketMessage::Pong(_) => {}
        }
    }

    Err("WebSocket 未收到 response.create 首帧".to_string())
}

fn normalize_responses_websocket_create(bytes: &[u8]) -> Result<(Value, bool), String> {
    let mut request = serde_json::from_slice::<Value>(bytes)
        .map_err(|error| format!("WebSocket 首帧不是合法 JSON: {error}"))?;
    let object = request
        .as_object_mut()
        .ok_or_else(|| "WebSocket 首帧必须是 JSON 对象".to_string())?;

    if object.get("type").and_then(Value::as_str) == Some("response.create") {
        object.remove("type");
    } else if object.contains_key("type") {
        return Err("WebSocket 首帧必须是 response.create 或 Responses payload".to_string());
    }

    object.insert("stream".to_string(), Value::Bool(true));
    let request_specifies_tier = object.get("service_tier").is_some();

    normalize_openai_responses_request(request)
        .map(|(payload, _)| payload)
        .map(|payload| (payload, request_specifies_tier))
}

async fn relay_responses_sse_to_websocket(
    socket: &mut AxumWebSocket,
    upstream: CodexUpstreamResponse,
    usage_storage: ProxyStorageContext,
    usage_metadata: ApiProxyUsageMetadata,
) -> Result<(), String> {
    let (_, mut upstream_stream) = upstream.into_stream();
    let mut decoder = SseDecoder::default();
    let mut recorded_usage = false;

    while let Some(chunk) = upstream_stream.next().await {
        let chunk = chunk?;
        for event in decoder.push(&chunk) {
            maybe_record_stream_usage_tokens(
                &usage_storage,
                &usage_metadata,
                &event,
                &mut recorded_usage,
            )
            .await;
            let done = is_responses_terminal_event(&event);
            send_responses_websocket_event(socket, &event).await?;
            if done {
                return Ok(());
            }
        }
    }

    for event in decoder.finish() {
        maybe_record_stream_usage_tokens(
            &usage_storage,
            &usage_metadata,
            &event,
            &mut recorded_usage,
        )
        .await;
        let done = is_responses_terminal_event(&event);
        send_responses_websocket_event(socket, &event).await?;
        if done {
            return Ok(());
        }
    }

    Ok(())
}

async fn send_responses_websocket_event(
    socket: &mut AxumWebSocket,
    event: &SseEvent,
) -> Result<(), String> {
    let data = rewrite_sse_event_data_models_for_client(&event.data);
    let text = match serde_json::from_str::<Value>(&data) {
        Ok(value) => serde_json::to_string(&value).unwrap_or(data),
        Err(_) => data,
    };

    socket
        .send(AxumWebSocketMessage::Text(text))
        .await
        .map_err(|error| format!("发送 WebSocket 响应帧失败: {error}"))
}

async fn send_responses_websocket_error(
    socket: &mut AxumWebSocket,
    message: &str,
) -> Result<(), String> {
    let payload = json!({
        "type": "error",
        "error": {
            "message": message,
        }
    });
    let text = serde_json::to_string(&payload).unwrap_or_else(|_| {
        "{\"type\":\"error\",\"error\":{\"message\":\"WebSocket error\"}}".to_string()
    });

    socket
        .send(AxumWebSocketMessage::Text(text))
        .await
        .map_err(|error| format!("发送 WebSocket 错误帧失败: {error}"))
}

fn is_responses_terminal_event(event: &SseEvent) -> bool {
    let event_type = serde_json::from_str::<Value>(&event.data)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .or_else(|| event.event.clone());

    matches!(
        event_type.as_deref(),
        Some(
            "response.completed"
                | "response.done"
                | "response.failed"
                | "response.incomplete"
                | "response.cancelled"
                | "response.canceled"
        )
    )
}

async fn unsupported_proxy_handler(
    State(context): State<Arc<ProxyContext>>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
) -> Response<Body> {
    if uri.path() == "/health" {
        return health_handler().await.into_response();
    }

    if let Err(response) = authorize_proxy_request(&headers, &context.api_keys) {
        return response;
    }

    json_error_response(
        StatusCode::NOT_FOUND,
        &format!(
            "当前反代只支持 GET /v1/models、POST /v1/chat/completions、POST /v1/responses、POST /v1/responses/compact、POST /v1/messages、POST /v1/images/generations、POST /v1/images/edits、POST /v1/images/variations，收到的是 {method} {}",
            uri.path()
        ),
    )
}

fn authorize_proxy_request(
    headers: &HeaderMap,
    api_keys: &Arc<RwLock<Vec<ApiProxyKey>>>,
) -> Result<ApiProxyKey, Response<Body>> {
    match read_authorized_api_proxy_key(headers, api_keys) {
        Some(key) => Ok(key),
        None => Err(json_error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid proxy api key.",
        )),
    }
}

fn authorize_anthropic_proxy_request(
    headers: &HeaderMap,
    api_keys: &Arc<RwLock<Vec<ApiProxyKey>>>,
) -> Result<ApiProxyKey, Response<Body>> {
    let version = headers
        .get("anthropic-version")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            anthropic_error_response(StatusCode::BAD_REQUEST, "missing anthropic-version header")
        })?;
    if version != ANTHROPIC_MESSAGES_REQUIRED_VERSION {
        return Err(anthropic_error_response(
            StatusCode::BAD_REQUEST,
            &format!("unsupported anthropic-version: {version}"),
        ));
    }

    match read_authorized_api_proxy_key(headers, api_keys) {
        Some(key) => Ok(key),
        None => Err(anthropic_error_response(
            StatusCode::UNAUTHORIZED,
            "invalid API key",
        )),
    }
}

async fn read_bounded_proxy_request_body(
    body: Body,
    max_bytes: usize,
) -> Result<Bytes, Response<Body>> {
    // Request extractor 之前先完成平台 Key 鉴权，再在这里读取并限制 body，避免未认证大包占满内存。
    axum::body::to_bytes(body, max_bytes)
        .await
        .map_err(|_| proxy_request_body_too_large_response(max_bytes))
}

async fn parse_json_request_async(
    headers: HeaderMap,
    body: Bytes,
    max_decoded_bytes: usize,
) -> Result<Value, Response<Body>> {
    // 限制解码并发；permit 移入 blocking job，客户端取消后也不会提前释放并放大压缩炸弹。
    let permit = PROXY_JSON_DECODE_LIMITER
        .get_or_init(|| Arc::new(Semaphore::new(MAX_CONCURRENT_PROXY_JSON_DECODE_JOBS)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            json_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "并发请求体解码已达上限，请稍后重试",
            )
        })?;
    // 压缩解码和大 JSON 解析都可能占用 CPU，移出 Tokio 请求执行线程以免阻塞其他代理流量。
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        parse_json_request(&headers, body, max_decoded_bytes)
    })
    .await
    .unwrap_or_else(|error| {
        Err(json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("解析请求体的后台任务失败: {error}"),
        ))
    })
}

fn parse_json_request(
    headers: &HeaderMap,
    body: Bytes,
    max_decoded_bytes: usize,
) -> Result<Value, Response<Body>> {
    let body = decode_proxy_request_body(headers, body, max_decoded_bytes)?;
    if body.is_empty() {
        return Err(invalid_request_response(
            "请求体为空。请确认客户端向 POST /v1/responses（或 /v1/chat/completions）发送了 JSON body，且 Base URL 只填到 /v1。",
        ));
    }
    serde_json::from_slice::<Value>(&body).map_err(|error| {
        invalid_request_response(&format!(
            "请求体不是合法 JSON: {error}; body_len={}",
            body.len()
        ))
    })
}

fn decode_proxy_request_body(
    headers: &HeaderMap,
    body: Bytes,
    max_decoded_bytes: usize,
) -> Result<Bytes, Response<Body>> {
    if body.is_empty() {
        return Ok(body);
    }
    if body.len() > max_decoded_bytes {
        return Err(proxy_request_body_too_large_response(max_decoded_bytes));
    }

    let encoding = headers
        .get(axum::http::header::CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty() && value != "identity");

    let encoding = encoding.or_else(|| detect_content_encoding_by_magic(&body));
    let Some(encoding) = encoding else {
        return Ok(body);
    };
    // 压缩请求除配置上限外再受硬上限和膨胀比约束，避免小压缩包占用数百 MiB。
    let max_decoded_bytes = max_decoded_bytes
        .min(MAX_PROXY_DECOMPRESSED_BODY_BYTES)
        .min(
            body.len()
                .saturating_mul(MAX_PROXY_BODY_EXPANSION_RATIO)
                .max(MIN_PROXY_DECOMPRESSED_BODY_BUDGET_BYTES),
        );

    match encoding.as_str() {
        "gzip" | "x-gzip" => decompress_gzip_body(&body, max_decoded_bytes),
        "deflate" => decompress_deflate_body(&body, max_decoded_bytes),
        "zstd" => decompress_zstd_body(&body, max_decoded_bytes),
        other => Err(invalid_request_response(&format!(
            "不支持的 Content-Encoding: {other}"
        ))),
    }
}

fn detect_content_encoding_by_magic(body: &[u8]) -> Option<String> {
    if body.len() >= 2 && body[0] == 0x1f && body[1] == 0x8b {
        return Some("gzip".to_string());
    }
    // zstd frame magic: 0xFD2FB528 (little-endian bytes 28 B5 2F FD)
    if body.len() >= 4 && body[0] == 0x28 && body[1] == 0xb5 && body[2] == 0x2f && body[3] == 0xfd {
        return Some("zstd".to_string());
    }
    None
}

fn proxy_request_body_too_large_response(max_decoded_bytes: usize) -> Response<Body> {
    json_error_response(
        StatusCode::PAYLOAD_TOO_LARGE,
        &format!("解压后的请求体超过限制（最大 {max_decoded_bytes} 字节）"),
    )
}

fn read_decompressed_body(
    decoder: impl Read,
    encoding: &str,
    max_decoded_bytes: usize,
) -> Result<Bytes, Response<Body>> {
    let mut decoder = decoder.take(max_decoded_bytes.saturating_add(1) as u64);
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).map_err(|error| {
        invalid_request_response(&format!("{encoding} 请求体解压失败: {error}"))
    })?;
    if decoded.len() > max_decoded_bytes {
        return Err(proxy_request_body_too_large_response(max_decoded_bytes));
    }
    Ok(Bytes::from(decoded))
}

fn decompress_gzip_body(body: &[u8], max_decoded_bytes: usize) -> Result<Bytes, Response<Body>> {
    read_decompressed_body(GzDecoder::new(body), "gzip", max_decoded_bytes)
}

fn decompress_deflate_body(body: &[u8], max_decoded_bytes: usize) -> Result<Bytes, Response<Body>> {
    // HTTP deflate 标准使用 zlib wrapper；同时识别历史客户端发送的 raw DEFLATE。
    let is_zlib_wrapped =
        body.len() >= 2 && body[0] & 0x0f == 8 && u16::from_be_bytes([body[0], body[1]]) % 31 == 0;
    if is_zlib_wrapped {
        read_decompressed_body(ZlibDecoder::new(body), "deflate", max_decoded_bytes)
    } else {
        read_decompressed_body(DeflateDecoder::new(body), "deflate", max_decoded_bytes)
    }
}

fn decompress_zstd_body(body: &[u8], max_decoded_bytes: usize) -> Result<Bytes, Response<Body>> {
    let decoder = zstd::stream::read::Decoder::new(body)
        .map_err(|error| invalid_request_response(&format!("zstd 请求体解压失败: {error}")))?;
    read_decompressed_body(decoder, "zstd", max_decoded_bytes)
}

fn anthropic_error_response(status: StatusCode, message: &str) -> Response<Body> {
    let mut response = Json(json!({
        "type": "error",
        "error": {
            "type": "invalid_request_error",
            "message": message,
        }
    }))
    .into_response();
    *response.status_mut() = status;
    response
}

fn request_session_affinity_key(headers: &HeaderMap, payload: &Value) -> Option<String> {
    request_session_affinity_key_from_headers(headers)
        .or_else(|| request_session_affinity_key_from_payload(payload))
}

fn request_session_affinity_key_from_headers(headers: &HeaderMap) -> Option<String> {
    SESSION_AFFINITY_HEADERS.iter().find_map(|name| {
        headers
            .get(*name)
            .and_then(|value| value.to_str().ok())
            .and_then(normalize_session_affinity_value)
            .map(|value| format!("header:{name}:{value}"))
    })
}

fn request_session_affinity_key_from_payload(payload: &Value) -> Option<String> {
    let object = payload.as_object()?;
    SESSION_AFFINITY_PAYLOAD_KEYS
        .iter()
        .find_map(|name| {
            object
                .get(*name)
                .and_then(Value::as_str)
                .and_then(normalize_session_affinity_value)
                .map(|value| format!("payload:{name}:{value}"))
        })
        .or_else(|| claude_metadata_session_affinity_key(object))
}

// Claude Code embeds a JSON blob in metadata.user_id whose session_id is the
// most reliable per-conversation identity; reuse it for account affinity.
fn claude_metadata_session_affinity_key(object: &Map<String, Value>) -> Option<String> {
    let user_id = object
        .get("metadata")
        .and_then(|metadata| metadata.get("user_id"))
        .and_then(Value::as_str)?;
    let parsed = serde_json::from_str::<Value>(user_id).ok()?;
    parsed
        .get("session_id")
        .and_then(Value::as_str)
        .and_then(normalize_session_affinity_value)
        .map(|value| format!("metadata:user_id:{value}"))
}

fn request_session_affinity_key_from_map(
    headers: &HeaderMap,
    payload: &Map<String, Value>,
) -> Option<String> {
    request_session_affinity_key_from_headers(headers)
        .or_else(|| request_session_affinity_key_from_payload(&Value::Object(payload.clone())))
}

fn normalize_session_affinity_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(160).collect())
    }
}

// Claude Code does not send `session-id`/`session_id`, which the Codex upstream
// expects for per-conversation routing; without it each request would otherwise
// fall back to a random UUID and land on a random prompt-cache shard. Derive a
// stable per-conversation identifier from the client-supplied session markers.
const ANTHROPIC_SESSION_ID_HEADERS: &[&str] = &["x-claude-code-session-id", "x-claude-session-id"];

fn anthropic_upstream_session_id(headers: &HeaderMap, request_json: &Value) -> Option<String> {
    for name in ANTHROPIC_SESSION_ID_HEADERS {
        if let Some(value) = headers
            .get(*name)
            .and_then(|value| value.to_str().ok())
            .and_then(normalize_session_affinity_value)
        {
            return Some(value);
        }
    }

    let user_id = request_json
        .get("metadata")
        .and_then(|metadata| metadata.get("user_id"))
        .and_then(Value::as_str)?;
    let parsed = serde_json::from_str::<Value>(user_id).ok()?;
    parsed
        .get("session_id")
        .and_then(Value::as_str)
        .and_then(normalize_session_affinity_value)
}

fn apply_anthropic_upstream_session_id(headers: HeaderMap, request_json: &Value) -> HeaderMap {
    if headers.get("session-id").is_some() || headers.get("session_id").is_some() {
        return headers;
    }
    let Some(session_id) = anthropic_upstream_session_id(&headers, request_json) else {
        return headers;
    };
    let Ok(value) = HeaderValue::from_str(&session_id) else {
        return headers;
    };
    let mut headers = headers;
    headers.insert("session-id", value);
    headers
}

fn invalid_request_response(message: &str) -> Response<Body> {
    let mut response = Json(json!({
        "error": {
            "message": message,
            "type": "invalid_request_error",
        }
    }))
    .into_response();
    *response.status_mut() = StatusCode::BAD_REQUEST;
    response
}

fn convert_openai_chat_request_to_codex(request: &Value) -> Result<(Value, bool), String> {
    let request_object = request
        .as_object()
        .ok_or_else(|| "聊天请求必须是 JSON 对象".to_string())?;

    // Cursor Agent may send Responses-style payloads to /v1/chat/completions.
    if request_object
        .get("messages")
        .and_then(Value::as_array)
        .is_none()
        && request_object.contains_key("input")
    {
        return normalize_openai_responses_request(request.clone());
    }

    let model = map_client_model_to_upstream(&request_model_or_default(request_object)?)?;
    let messages = request_object
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| "聊天请求缺少 messages 数组".to_string())?;
    if request_object.contains_key("tools")
        && !request_object.get("tools").is_some_and(Value::is_array)
    {
        return Err("tools 必须是数组".to_string());
    }

    let downstream_stream = request_object
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut root = Map::new();
    root.insert("model".to_string(), Value::String(model));
    root.insert("stream".to_string(), Value::Bool(true));
    root.insert("store".to_string(), Value::Bool(false));
    root.insert("instructions".to_string(), Value::String(String::new()));
    root.insert(
        "parallel_tool_calls".to_string(),
        Value::Bool(
            request_object
                .get("parallel_tool_calls")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        ),
    );
    root.insert(
        "include".to_string(),
        Value::Array(vec![Value::String(
            "reasoning.encrypted_content".to_string(),
        )]),
    );
    root.insert(
        "reasoning".to_string(),
        json!({
            "effort": normalize_api_proxy_reasoning_effort_for_upstream(
                request_reasoning_effort(request_object)?
                    .unwrap_or(DEFAULT_API_PROXY_REASONING_EFFORT),
            )?,
            "summary": request_object
                .get("reasoning")
                .and_then(|value| value.get("summary"))
                .and_then(Value::as_str)
                .unwrap_or("auto"),
        }),
    );

    let mut input = Vec::new();
    for message in messages {
        let message_object = message
            .as_object()
            .ok_or_else(|| "messages 数组中的每一项都必须是对象".to_string())?;
        let role = required_string(message_object, "role")?;

        if role == "tool" {
            let tool_call_id = required_string(message_object, "tool_call_id")?;
            input.push(json!({
                "type": "function_call_output",
                "call_id": tool_call_id,
                "output": stringify_message_content(message_object.get("content")),
            }));
            continue;
        }

        let codex_role = match role.as_str() {
            "system" => "developer",
            "developer" => "developer",
            "assistant" => "assistant",
            _ => "user",
        };
        let mut content_parts = Vec::new();

        if let Some(content) = message_object.get("content") {
            content_parts.extend(convert_message_content_to_codex_parts(
                role.as_str(),
                content,
            ));
        }

        input.push(json!({
            "type": "message",
            "role": codex_role,
            "content": content_parts,
        }));

        if role == "assistant" {
            if let Some(tool_calls) = message_object.get("tool_calls").and_then(Value::as_array) {
                for tool_call in tool_calls {
                    let tool_call_object = match tool_call.as_object() {
                        Some(value) => value,
                        None => continue,
                    };
                    if tool_call_object
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("function")
                        != "function"
                    {
                        continue;
                    }

                    let function = match tool_call_object.get("function").and_then(Value::as_object)
                    {
                        Some(value) => value,
                        None => continue,
                    };
                    let name = function
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let arguments = stringify_json_field(function.get("arguments"));
                    let call_id = tool_call_object
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default();

                    input.push(json!({
                        "type": "function_call",
                        "call_id": call_id,
                        "name": name,
                        "arguments": arguments,
                    }));
                }
            }
        }
    }
    root.insert("input".to_string(), Value::Array(input));

    if let Some(tools) = request_object.get("tools").and_then(Value::as_array) {
        let mut converted = Vec::new();
        for tool in tools {
            let tool_object = match tool.as_object() {
                Some(value) => value,
                None => continue,
            };
            match tool_object
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
            {
                "function" => {
                    let Some(function) = tool_object.get("function").and_then(Value::as_object)
                    else {
                        continue;
                    };
                    let mut converted_tool = Map::new();
                    converted_tool
                        .insert("type".to_string(), Value::String("function".to_string()));
                    if let Some(name) = function.get("name").and_then(Value::as_str) {
                        converted_tool.insert("name".to_string(), Value::String(name.to_string()));
                    }
                    if let Some(description) = function.get("description") {
                        converted_tool.insert("description".to_string(), description.clone());
                    }
                    if let Some(parameters) = function.get("parameters") {
                        converted_tool.insert("parameters".to_string(), parameters.clone());
                    }
                    if let Some(strict) = function.get("strict") {
                        converted_tool.insert("strict".to_string(), strict.clone());
                    }
                    converted.push(Value::Object(converted_tool));
                }
                _ => converted.push(tool.clone()),
            }
        }

        if !converted.is_empty() {
            root.insert("tools".to_string(), Value::Array(converted));
        }
    }

    if let Some(tool_choice) = request_object.get("tool_choice") {
        root.insert("tool_choice".to_string(), tool_choice.clone());
    }

    if let Some(response_format) = request_object.get("response_format") {
        map_response_format(&mut root, response_format);
    }
    if let Some(text) = request_object.get("text") {
        map_text_settings(&mut root, text);
    }
    root.insert(
        "service_tier".to_string(),
        Value::String(api_proxy_service_tier_for_upstream(request_object)?),
    );

    normalize_responses_lite_payload(&mut root)?;

    Ok((Value::Object(root), downstream_stream))
}

fn normalize_openai_responses_request(mut request: Value) -> Result<(Value, bool), String> {
    let object = request
        .as_object_mut()
        .ok_or_else(|| "responses 请求必须是 JSON 对象".to_string())?;

    let model = map_client_model_to_upstream(&request_model_or_default(object)?)?;
    let downstream_stream = object
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    object.insert("model".to_string(), Value::String(model));
    object.insert("stream".to_string(), Value::Bool(true));
    object.insert("store".to_string(), Value::Bool(false));
    object.insert(
        "service_tier".to_string(),
        Value::String(api_proxy_service_tier_for_upstream(object)?),
    );
    if !object.contains_key("instructions") {
        object.insert("instructions".to_string(), Value::String(String::new()));
    }
    if !object.contains_key("parallel_tool_calls") {
        object.insert("parallel_tool_calls".to_string(), Value::Bool(true));
    }

    let legacy_reasoning_effort = match object.remove("reasoning_effort") {
        Some(Value::String(effort)) => Some(effort),
        Some(Value::Null) | None => None,
        Some(_) => return Err("reasoning_effort 必须是字符串".to_string()),
    };
    let reasoning = object
        .entry("reasoning".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if reasoning.is_null() {
        *reasoning = Value::Object(Map::new());
    } else if !reasoning.is_object() {
        return Err("reasoning 必须是对象".to_string());
    }
    if let Some(reasoning_object) = reasoning.as_object_mut() {
        if !reasoning_object.contains_key("effort")
            || reasoning_object.get("effort").is_some_and(Value::is_null)
        {
            reasoning_object.insert(
                "effort".to_string(),
                Value::String(match legacy_reasoning_effort.as_deref() {
                    Some(effort) => normalize_api_proxy_reasoning_effort_for_upstream(effort)?,
                    None => DEFAULT_API_PROXY_REASONING_EFFORT.to_string(),
                }),
            );
        } else if let Some(effort) = reasoning_object.get("effort").and_then(Value::as_str) {
            reasoning_object.insert(
                "effort".to_string(),
                Value::String(normalize_api_proxy_reasoning_effort_for_upstream(effort)?),
            );
        } else {
            return Err("reasoning.effort 必须是字符串".to_string());
        }
        if !reasoning_object.contains_key("summary") {
            reasoning_object.insert("summary".to_string(), Value::String("auto".to_string()));
        }
    }

    let include = object
        .entry("include".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !include.is_array() {
        *include = Value::Array(Vec::new());
    }
    if let Some(items) = include.as_array_mut() {
        let exists = items.iter().any(|value| {
            value
                .as_str()
                .map(|value| value == "reasoning.encrypted_content")
                .unwrap_or(false)
        });
        if !exists {
            items.push(Value::String("reasoning.encrypted_content".to_string()));
        }
    }

    // Cursor may attach OpenAI-compatible fields that Codex upstream rejects.
    for key in UNSUPPORTED_RESPONSES_REQUEST_FIELDS {
        object.remove(*key);
    }

    normalize_responses_lite_payload(object)?;

    Ok((request, downstream_stream))
}

const COMPACT_REQUEST_FIELDS: &[&str] = &[
    "model",
    "input",
    "instructions",
    "tools",
    "parallel_tool_calls",
    "reasoning",
    "text",
    "service_tier",
    "prompt_cache_key",
];

fn normalize_openai_compact_request(request: Value) -> Result<Value, String> {
    let request_object = request
        .as_object()
        .ok_or_else(|| "compact 请求必须是 JSON 对象".to_string())?;

    let mut normalized = Map::new();
    for field in COMPACT_REQUEST_FIELDS {
        if let Some(value) = request_object.get(*field) {
            normalized.insert((*field).to_string(), value.clone());
        }
    }

    let model = map_client_model_to_upstream(&request_model_or_default(&normalized)?)?;
    normalized.insert("model".to_string(), Value::String(model));
    normalized.insert(
        "service_tier".to_string(),
        Value::String(api_proxy_service_tier_for_upstream(request_object)?),
    );

    if normalized
        .get("prompt_cache_key")
        .is_some_and(Value::is_null)
    {
        normalized.remove("prompt_cache_key");
    } else if normalized
        .get("prompt_cache_key")
        .is_some_and(|value| !value.is_string())
    {
        return Err("prompt_cache_key 必须是字符串".to_string());
    }

    if let Some(reasoning) = normalized.get_mut("reasoning") {
        if reasoning.is_null() {
            *reasoning = Value::Object(Map::new());
        } else if !reasoning.is_object() {
            return Err("reasoning 必须是对象".to_string());
        }
        if let Some(reasoning_object) = reasoning.as_object_mut() {
            if let Some(effort) = reasoning_object.get("effort").and_then(Value::as_str) {
                reasoning_object.insert(
                    "effort".to_string(),
                    Value::String(normalize_api_proxy_reasoning_effort_for_upstream(effort)?),
                );
            }
        }
    }

    Ok(Value::Object(normalized))
}

fn normalize_codex_compact_reasoning_effort(mut payload: Value) -> Value {
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !model.contains("gpt-5.6") && !model.contains("gpt-5.5") {
        return payload;
    }
    let Some(object) = payload.as_object_mut() else {
        return payload;
    };
    let Some(reasoning) = object.get_mut("reasoning").and_then(Value::as_object_mut) else {
        return payload;
    };
    if reasoning
        .get("effort")
        .and_then(Value::as_str)
        .is_some_and(|effort| effort.eq_ignore_ascii_case("max"))
    {
        // ChatGPT compact currently accepts xhigh, not max (Codex Ultra maps to max).
        reasoning.insert("effort".to_string(), Value::String("xhigh".to_string()));
    }
    payload
}

fn parse_compact_upstream_response(body: &[u8]) -> Result<Value, String> {
    if body.is_empty() {
        return Err("compact 上游返回空响应".to_string());
    }

    if let Ok(value) = serde_json::from_slice::<Value>(body) {
        if let Some(message) = response_error_message_from_value(&value) {
            return Err(message);
        }
        if value.get("type").and_then(Value::as_str) == Some("response.completed") {
            return value
                .get("response")
                .cloned()
                .ok_or_else(|| "compact 响应缺少 response 字段".to_string());
        }
        if value.get("output").is_some() || value.get("id").is_some() {
            return Ok(value);
        }
    }

    extract_completed_response_from_sse(body)
}

fn build_compact_sse_response(response: &Value) -> Result<Bytes, String> {
    let mut response = response.clone();
    if let Some(object) = response.as_object_mut() {
        if object
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .map(str::is_empty)
            .unwrap_or(true)
        {
            object.insert(
                "id".to_string(),
                Value::String(format!("resp_{}", uuid::Uuid::new_v4().simple())),
            );
        }
        if let Some(usage) = object.get("usage") {
            if !compact_usage_is_parsable(usage) {
                object.remove("usage");
            }
        }
    }

    let compact_json = serde_json::to_vec(&response)
        .map_err(|error| format!("序列化 compact SSE response 失败: {error}"))?;
    let mut payload = String::new();
    if let Some(items) = response.get("output").and_then(Value::as_array) {
        for (index, item) in items.iter().enumerate() {
            if !item.is_object() {
                continue;
            }
            let event = json!({
                "type": "response.output_item.done",
                "output_index": index,
                "item": item,
            });
            let event_json = serde_json::to_string(&event)
                .map_err(|error| format!("序列化 compact SSE item 失败: {error}"))?;
            payload.push_str("event: response.output_item.done\ndata: ");
            payload.push_str(&event_json);
            payload.push_str("\n\n");
        }
    }

    let completed = json!({
        "type": "response.completed",
        "response": serde_json::from_slice::<Value>(&compact_json)
            .map_err(|error| format!("重建 compact completed 失败: {error}"))?,
    });
    let completed_json = serde_json::to_string(&completed)
        .map_err(|error| format!("序列化 compact completed 失败: {error}"))?;
    payload.push_str("event: response.completed\ndata: ");
    payload.push_str(&completed_json);
    payload.push_str("\n\n");
    Ok(Bytes::from(payload))
}

fn compact_usage_is_parsable(usage: &Value) -> bool {
    let Some(object) = usage.as_object() else {
        return false;
    };
    ["input_tokens", "output_tokens", "total_tokens"]
        .iter()
        .all(|field| object.get(*field).and_then(Value::as_i64).is_some())
}

fn convert_anthropic_messages_request_to_codex(request: &Value) -> Result<(Value, bool), String> {
    let request_object = request
        .as_object()
        .ok_or_else(|| "Anthropic Messages 请求必须是 JSON 对象".to_string())?;
    let model = map_client_model_to_upstream(&request_model_or_default(request_object)?)?;
    let messages = request_object
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| "Anthropic Messages 请求缺少 messages 数组".to_string())?;
    if request_object.contains_key("tools")
        && !request_object.get("tools").is_some_and(Value::is_array)
    {
        return Err("tools 必须是数组".to_string());
    }
    let downstream_stream = bool_field(request_object, "stream", false);

    let mut root = Map::new();
    root.insert("model".to_string(), Value::String(model));
    root.insert("stream".to_string(), Value::Bool(true));
    root.insert("store".to_string(), Value::Bool(false));
    root.insert(
        "instructions".to_string(),
        Value::String(anthropic_system_to_instructions(
            request_object.get("system"),
        )),
    );
    root.insert(
        "parallel_tool_calls".to_string(),
        Value::Bool(bool_field(request_object, "parallel_tool_calls", true)),
    );
    root.insert(
        "include".to_string(),
        Value::Array(vec![Value::String(
            "reasoning.encrypted_content".to_string(),
        )]),
    );
    root.insert(
        "reasoning".to_string(),
        json!({
            "effort": anthropic_reasoning_effort(request_object)?,
            "summary": "auto",
        }),
    );

    // max_tokens is accepted but intentionally not forwarded: the Codex
    // upstream rejects requests carrying max_output_tokens with
    // "Unsupported parameter: max_output_tokens", and Anthropic clients
    // (e.g. Claude Code) are required to always send max_tokens.
    copy_anthropic_passthrough_field(request_object, &mut root, "temperature", "temperature");
    copy_anthropic_passthrough_field(request_object, &mut root, "top_p", "top_p");
    root.insert(
        "service_tier".to_string(),
        Value::String(api_proxy_service_tier_for_upstream(request_object)?),
    );
    if let Some(stop_sequences) = request_object.get("stop_sequences") {
        root.insert("stop".to_string(), stop_sequences.clone());
    }

    let mut input = Vec::new();
    for message in messages {
        let message_object = message
            .as_object()
            .ok_or_else(|| "Anthropic Messages 的 messages 数组每一项都必须是对象".to_string())?;
        let role = required_string(message_object, "role")?;
        let codex_role = if role == "assistant" {
            "assistant"
        } else {
            "user"
        };
        let (content_parts, special_items) =
            split_anthropic_content_for_codex(&role, message_object.get("content"));

        // 普通文本和图片仍按 message 传给上游，工具结果和工具调用则拆成 Responses 专用 item。
        if !content_parts.is_empty() || special_items.is_empty() {
            input.push(json!({
                "type": "message",
                "role": codex_role,
                "content": content_parts,
            }));
        }
        input.extend(special_items);
    }
    root.insert("input".to_string(), Value::Array(input));

    if let Some(tools) = request_object.get("tools").and_then(Value::as_array) {
        let converted = convert_anthropic_tools_to_codex(tools);
        if !converted.is_empty() {
            root.insert("tools".to_string(), Value::Array(converted));
        }
    }
    if let Some(tool_choice) = request_object.get("tool_choice") {
        if let Some(converted) = convert_anthropic_tool_choice_to_codex(tool_choice) {
            root.insert("tool_choice".to_string(), converted);
        }
    }

    normalize_responses_lite_payload(&mut root)?;

    Ok((Value::Object(root), downstream_stream))
}

const ANTHROPIC_BILLING_HEADER_LINE_PREFIX: &str = "x-anthropic-billing-header:";

fn strip_anthropic_billing_header_lines(text: &str) -> String {
    if !text.lines().any(|line| {
        line.trim_start()
            .starts_with(ANTHROPIC_BILLING_HEADER_LINE_PREFIX)
    }) {
        return text.to_string();
    }

    text.lines()
        .filter(|line| {
            !line
                .trim_start()
                .starts_with(ANTHROPIC_BILLING_HEADER_LINE_PREFIX)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn anthropic_system_to_instructions(system: Option<&Value>) -> String {
    match system {
        Some(Value::String(text)) => strip_anthropic_billing_header_lines(text),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                item.as_object()
                    .and_then(|object| object.get("text"))
                    .and_then(Value::as_str)
                    .map(strip_anthropic_billing_header_lines)
                    .filter(|text| !text.is_empty())
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn anthropic_reasoning_effort(request_object: &Map<String, Value>) -> Result<String, String> {
    if let Some(effort) = request_reasoning_effort(request_object)? {
        return normalize_api_proxy_reasoning_effort_for_upstream(effort);
    }

    // Claude Code reports the session effort picker as output_config.effort
    // (low/medium/high/xhigh/max); map it through when the request carries no
    // explicit reasoning field. Unknown values fall through to the default.
    if let Some(effort) = request_object
        .get("output_config")
        .and_then(|config| config.get("effort"))
        .and_then(Value::as_str)
    {
        if let Ok(effort) = normalize_api_proxy_reasoning_effort_for_upstream(effort) {
            return Ok(effort);
        }
    }

    if request_object
        .get("thinking")
        .and_then(Value::as_object)
        .and_then(|thinking| thinking.get("type"))
        .and_then(Value::as_str)
        == Some("enabled")
    {
        Ok("high".to_string())
    } else {
        Ok(DEFAULT_API_PROXY_REASONING_EFFORT.to_string())
    }
}

fn copy_anthropic_passthrough_field(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    source_key: &str,
    target_key: &str,
) {
    if let Some(value) = source.get(source_key) {
        target.insert(target_key.to_string(), value.clone());
    }
}

fn split_anthropic_content_for_codex(
    role: &str,
    content: Option<&Value>,
) -> (Vec<Value>, Vec<Value>) {
    let mut content_parts = Vec::new();
    let mut special_items = Vec::new();
    let text_type = if role == "assistant" {
        "output_text"
    } else {
        "input_text"
    };

    match content {
        Some(Value::String(text)) => {
            if !text.is_empty() {
                content_parts.push(json!({
                    "type": text_type,
                    "text": text,
                }));
            }
        }
        Some(Value::Array(items)) => {
            for item in items {
                let Some(item_object) = item.as_object() else {
                    continue;
                };
                match item_object
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                {
                    "text" => {
                        if let Some(text) = item_object.get("text").and_then(Value::as_str) {
                            content_parts.push(json!({
                                "type": text_type,
                                "text": text,
                            }));
                        }
                    }
                    "image" if role == "user" => {
                        if let Some(image_url) =
                            anthropic_image_source_to_codex_url(item_object.get("source"))
                        {
                            content_parts.push(json!({
                                "type": "input_image",
                                "image_url": image_url,
                            }));
                        }
                    }
                    "tool_use" if role == "assistant" => {
                        special_items.push(json!({
                            "type": "function_call",
                            "call_id": item_object.get("id").and_then(Value::as_str).unwrap_or_default(),
                            "name": item_object.get("name").and_then(Value::as_str).unwrap_or_default(),
                            "arguments": stringify_json_field(item_object.get("input")),
                        }));
                    }
                    "tool_result" if role == "user" => {
                        special_items.push(json!({
                            "type": "function_call_output",
                            "call_id": item_object.get("tool_use_id").and_then(Value::as_str).unwrap_or_default(),
                            "output": stringify_anthropic_tool_result_content(item_object.get("content")),
                        }));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    (content_parts, special_items)
}

fn anthropic_image_source_to_codex_url(source: Option<&Value>) -> Option<String> {
    let source = source?.as_object()?;
    match source.get("type").and_then(Value::as_str) {
        Some("base64") => {
            let media_type = source
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("application/octet-stream");
            let data = source.get("data").and_then(Value::as_str)?;
            Some(format!("data:{media_type};base64,{data}"))
        }
        Some("url") => source
            .get("url")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        _ => None,
    }
}

fn stringify_anthropic_tool_result_content(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                item.as_object()
                    .and_then(|object| object.get("text"))
                    .and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
        None => String::new(),
    }
}

fn convert_anthropic_tools_to_codex(tools: &[Value]) -> Vec<Value> {
    let mut converted = Vec::new();
    for tool in tools {
        let Some(tool_object) = tool.as_object() else {
            continue;
        };
        let Some(name) = tool_object.get("name").and_then(Value::as_str) else {
            continue;
        };
        let mut converted_tool = Map::new();
        converted_tool.insert("type".to_string(), Value::String("function".to_string()));
        converted_tool.insert("name".to_string(), Value::String(name.to_string()));
        if let Some(description) = tool_object.get("description") {
            converted_tool.insert("description".to_string(), description.clone());
        }
        converted_tool.insert(
            "parameters".to_string(),
            tool_object
                .get("input_schema")
                .cloned()
                .unwrap_or_else(|| json!({ "type": "object" })),
        );
        converted.push(Value::Object(converted_tool));
    }
    converted
}

fn convert_anthropic_tool_choice_to_codex(tool_choice: &Value) -> Option<Value> {
    let tool_choice = tool_choice.as_object()?;
    match tool_choice.get("type").and_then(Value::as_str)? {
        "auto" => Some(Value::String("auto".to_string())),
        "any" => Some(Value::String("required".to_string())),
        "none" => Some(Value::String("none".to_string())),
        "tool" => tool_choice.get("name").and_then(Value::as_str).map(|name| {
            json!({
                "type": "function",
                "name": name,
            })
        }),
        _ => None,
    }
}

#[derive(Default)]
struct ImageMultipartRequest {
    fields: Map<String, Value>,
    images: Vec<Value>,
    mask: Option<Value>,
}

#[derive(Debug)]
struct ConvertedImageRequest {
    upstream_payload: Value,
    downstream_stream: bool,
    image_count: usize,
}

async fn parse_image_multipart_request(
    mut multipart: Multipart,
) -> Result<ImageMultipartRequest, String> {
    let mut request = ImageMultipartRequest::default();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| format!("读取 multipart 字段失败: {error}"))?
    {
        let name = field.name().map(ToString::to_string).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let file_name = field.file_name().map(ToString::to_string);
        let content_type = field.content_type().map(ToString::to_string);
        let bytes = field
            .bytes()
            .await
            .map_err(|error| format!("读取 multipart 字段 {name} 失败: {error}"))?;

        match name.as_str() {
            "image" | "image[]" | "images[]" => {
                request.images.push(multipart_image_part(
                    &bytes,
                    content_type.as_deref(),
                    file_name.as_deref(),
                ));
            }
            "mask" => {
                request.mask = Some(multipart_image_part(
                    &bytes,
                    content_type.as_deref(),
                    file_name.as_deref(),
                ));
            }
            _ => {
                let value = String::from_utf8(bytes.to_vec())
                    .map_err(|_| format!("multipart 字段 {name} 必须是 UTF-8 文本"))?;
                insert_multipart_text_field(&mut request.fields, &name, value);
            }
        }
    }

    Ok(request)
}

fn multipart_image_part(
    bytes: &Bytes,
    content_type: Option<&str>,
    _file_name: Option<&str>,
) -> Value {
    let mime_type = content_type
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| guess_image_mime_type(bytes));
    let mut part = Map::new();
    part.insert("type".to_string(), Value::String("input_image".to_string()));
    part.insert(
        "image_url".to_string(),
        Value::String(format!(
            "data:{mime_type};base64,{}",
            BASE64_STANDARD.encode(bytes)
        )),
    );
    Value::Object(part)
}

fn guess_image_mime_type(bytes: &[u8]) -> &str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

fn insert_multipart_text_field(fields: &mut Map<String, Value>, name: &str, value: String) {
    if name.ends_with("[]") {
        let key = name.trim_end_matches("[]").to_string();
        if let Some(items) = fields
            .entry(key)
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
        {
            items.push(Value::String(value));
        }
    } else {
        fields.insert(name.to_string(), Value::String(value));
    }
}

fn convert_openai_image_generation_request_to_codex(
    request: &Value,
) -> Result<ConvertedImageRequest, String> {
    let object = request
        .as_object()
        .ok_or_else(|| "图片生成请求必须是 JSON 对象".to_string())?;
    let prompt = required_string(object, "prompt")?;
    convert_image_request_parts_to_codex(object, &prompt, Vec::new(), None, false)
}

fn convert_openai_image_edit_request_to_codex(
    request: &ImageMultipartRequest,
    variation: bool,
) -> Result<ConvertedImageRequest, String> {
    if request.images.is_empty() {
        return Err("图片编辑请求缺少 image 文件".to_string());
    }
    let prompt = if variation {
        IMAGE_VARIATION_PROMPT.to_string()
    } else {
        required_string(&request.fields, "prompt")?
    };
    convert_image_request_parts_to_codex(
        &request.fields,
        &prompt,
        request.images.clone(),
        request.mask.clone(),
        true,
    )
}

fn convert_image_request_parts_to_codex(
    object: &Map<String, Value>,
    prompt: &str,
    images: Vec<Value>,
    mask: Option<Value>,
    edit_action: bool,
) -> Result<ConvertedImageRequest, String> {
    if object
        .get("response_format")
        .and_then(Value::as_str)
        .map(|value| value == "url")
        .unwrap_or(false)
    {
        return Err("Codex 账号图片反代只支持 response_format=b64_json".to_string());
    }

    let tool_model = object
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_IMAGE_TOOL_MODEL);
    if !is_supported_image_model(tool_model) {
        return Err(format!("不支持的图片模型: {tool_model}"));
    }
    let downstream_stream = bool_field(object, "stream", false);
    let image_count = image_count_field(object)?;
    if downstream_stream && image_count > 1 {
        return Err("stream:true 暂只支持 n=1；请改用非流式请求生成多张图片".to_string());
    }

    let mut content = vec![json!({
        "type": "input_text",
        "text": prompt,
    })];
    content.extend(images);
    if let Some(mask) = mask {
        content.push(json!({
            "type": "input_text",
            "text": "Use the following image as the edit mask for the preceding image. Treat the light/white area as the region to edit and keep the dark/black area unchanged.",
        }));
        content.push(mask_as_input_image(mask));
    }

    let mut tool = Map::new();
    tool.insert(
        "type".to_string(),
        Value::String("image_generation".to_string()),
    );
    tool.insert("model".to_string(), Value::String(tool_model.to_string()));
    tool.insert(
        "action".to_string(),
        Value::String(if edit_action { "edit" } else { "generate" }.to_string()),
    );
    copy_image_tool_string_field(object, &mut tool, "size", "size");
    copy_image_tool_string_field(object, &mut tool, "quality", "quality");
    copy_image_tool_string_field(object, &mut tool, "background", "background");
    copy_image_tool_string_field(object, &mut tool, "output_format", "output_format");
    copy_image_tool_number_field(
        object,
        &mut tool,
        "output_compression",
        "output_compression",
    );
    copy_image_tool_number_field(object, &mut tool, "compression", "output_compression");
    copy_image_tool_number_field(object, &mut tool, "partial_images", "partial_images");
    copy_image_tool_string_field(object, &mut tool, "input_fidelity", "input_fidelity");

    let payload = json!({
        "model": DEFAULT_IMAGE_CONTROLLER_MODEL,
        "stream": true,
        "store": false,
        "instructions": "",
        "parallel_tool_calls": true,
        "reasoning": {
            "effort": "medium",
            "summary": "auto"
        },
        "input": [{
            "type": "message",
            "role": "user",
            "content": content,
        }],
        "tools": [Value::Object(tool)],
        "tool_choice": {
            "type": "image_generation"
        }
    });

    Ok(ConvertedImageRequest {
        upstream_payload: payload,
        downstream_stream,
        image_count,
    })
}

fn is_supported_image_model(model: &str) -> bool {
    model == DEFAULT_IMAGE_TOOL_MODEL
}

fn bool_field(object: &Map<String, Value>, key: &str, default: bool) -> bool {
    match object.get(key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => default,
        },
        _ => default,
    }
}

fn image_count_field(object: &Map<String, Value>) -> Result<usize, String> {
    let Some(value) = object.get("n") else {
        return Ok(1);
    };
    let count =
        integer_field_value(value).ok_or_else(|| "图片请求参数 n 必须是正整数".to_string())?;
    if !(1..=10).contains(&count) {
        return Err("图片请求参数 n 必须在 1 到 10 之间".to_string());
    }
    usize::try_from(count).map_err(|_| "图片请求参数 n 超出范围".to_string())
}

fn integer_field_value(value: &Value) -> Option<i64> {
    if let Some(value) = value.as_i64() {
        Some(value)
    } else if let Some(value) = value.as_u64() {
        i64::try_from(value).ok()
    } else if let Some(value) = value.as_str() {
        value.trim().parse::<i64>().ok()
    } else {
        None
    }
}

fn copy_image_tool_string_field(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    source_key: &str,
    target_key: &str,
) {
    if let Some(value) = source
        .get(source_key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        target.insert(target_key.to_string(), Value::String(value.to_string()));
    }
}

fn copy_image_tool_number_field(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    source_key: &str,
    target_key: &str,
) {
    if let Some(value) = source.get(source_key).and_then(integer_field_value) {
        target.insert(
            target_key.to_string(),
            Value::Number(serde_json::Number::from(value)),
        );
    }
}

fn mask_as_input_image(mut mask: Value) -> Value {
    if let Some(object) = mask.as_object_mut() {
        object.insert("type".to_string(), Value::String("input_image".to_string()));
    }
    mask
}

fn convert_responses_image_output_to_images_response(response: &Value) -> Result<Value, String> {
    let mut data = Vec::new();
    collect_images_from_response_value(response, &mut data);
    if data.is_empty() {
        return Err("Codex 图片响应中没有 image_generation_call.result".to_string());
    }

    Ok(json!({
        "created": response
            .get("created_at")
            .and_then(Value::as_i64)
            .unwrap_or_else(now_unix_seconds),
        "data": data,
    }))
}

fn collect_images_from_response_value(value: &Value, data: &mut Vec<Value>) {
    match value {
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("image_generation_call") {
                if let Some(result) = object.get("result").and_then(Value::as_str) {
                    if !result.is_empty() {
                        let mut item = Map::new();
                        item.insert("b64_json".to_string(), Value::String(result.to_string()));
                        if let Some(prompt) = object.get("revised_prompt").and_then(Value::as_str) {
                            item.insert(
                                "revised_prompt".to_string(),
                                Value::String(prompt.to_string()),
                            );
                        }
                        data.push(Value::Object(item));
                    }
                }
            }
            for value in object.values() {
                collect_images_from_response_value(value, data);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_images_from_response_value(item, data);
            }
        }
        _ => {}
    }
}

fn map_client_model_to_upstream(model: &str) -> Result<String, String> {
    Ok(remap_model_name(model, REQUEST_MODEL_MAPPINGS).unwrap_or_else(|| model.to_string()))
}

fn request_model_or_default(object: &Map<String, Value>) -> Result<String, String> {
    match object.get("model") {
        None | Some(Value::Null) => Ok(DEFAULT_API_PROXY_MODEL.to_string()),
        Some(Value::String(model)) if model.trim().is_empty() => {
            Ok(DEFAULT_API_PROXY_MODEL.to_string())
        }
        Some(Value::String(model)) => Ok(model.trim().to_string()),
        Some(_) => Err("model 必须是字符串".to_string()),
    }
}

fn request_reasoning_effort(object: &Map<String, Value>) -> Result<Option<&str>, String> {
    match object.get("reasoning_effort") {
        Some(Value::String(effort)) => return Ok(Some(effort.as_str())),
        Some(Value::Null) | None => {}
        Some(_) => return Err("reasoning_effort 必须是字符串".to_string()),
    }

    let Some(reasoning) = object.get("reasoning") else {
        return Ok(None);
    };
    if reasoning.is_null() {
        return Ok(None);
    }
    let reasoning = reasoning
        .as_object()
        .ok_or_else(|| "reasoning 必须是对象".to_string())?;
    match reasoning.get("effort") {
        Some(Value::String(effort)) => Ok(Some(effort.as_str())),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err("reasoning.effort 必须是字符串".to_string()),
    }
}

fn is_responses_lite_model(model: &str) -> bool {
    ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"]
        .iter()
        .any(|prefix| model == *prefix || model.starts_with(&format!("{prefix}-")))
}

fn payload_uses_responses_lite(payload: &Value) -> bool {
    payload
        .get("model")
        .and_then(Value::as_str)
        .is_some_and(is_responses_lite_model)
}

fn normalize_responses_lite_payload(object: &mut Map<String, Value>) -> Result<(), String> {
    let Some(model) = object.get("model").and_then(Value::as_str) else {
        return Ok(());
    };
    if !is_responses_lite_model(model) {
        return Ok(());
    }
    if object
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .and_then(Value::as_str)
        == Some("minimal")
    {
        return Err(
            "GPT-5.6 不支持 minimal 推理强度；请使用 none、low、medium、high、xhigh 或 max"
                .to_string(),
        );
    }

    if object.contains_key("tools") && !object.get("tools").is_some_and(Value::is_array) {
        return Err("GPT-5.6 Responses Lite 的 tools 必须是数组".to_string());
    }
    if object.contains_key("instructions")
        && !object.get("instructions").is_some_and(Value::is_string)
    {
        return Err("GPT-5.6 Responses Lite 的 instructions 必须是字符串".to_string());
    }

    let extra_tools = object
        .remove("tools")
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    let instructions = object
        .remove("instructions")
        .and_then(|value| value.as_str().map(ToString::to_string))
        .filter(|value| !value.is_empty());
    let mut input = responses_lite_input_items(object.remove("input"))?;
    for item in &mut input {
        prepare_responses_lite_images(item);
    }

    let additional_tools_count = input
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("additional_tools"))
        .count();
    if additional_tools_count > 1
        || (additional_tools_count == 1
            && input
                .first()
                .and_then(|item| item.get("type"))
                .and_then(Value::as_str)
                != Some("additional_tools"))
    {
        return Err(
            "GPT-5.6 Responses Lite 的 additional_tools 必须且只能位于 input 首项".to_string(),
        );
    }

    let already_has_additional_tools = input
        .first()
        .and_then(Value::as_object)
        .and_then(|item| item.get("type"))
        .and_then(Value::as_str)
        == Some("additional_tools");

    if already_has_additional_tools {
        let tools_are_array = input
            .first()
            .and_then(Value::as_object)
            .and_then(|item| item.get("tools"))
            .is_some_and(Value::is_array);
        if !tools_are_array {
            return Err("GPT-5.6 Responses Lite 的 additional_tools.tools 必须是数组".to_string());
        }
        if let Some(item) = input.first_mut().and_then(Value::as_object_mut) {
            item.insert("role".to_string(), Value::String("developer".to_string()));
        }
        if !extra_tools.is_empty() {
            if let Some(tools) = input
                .first_mut()
                .and_then(Value::as_object_mut)
                .and_then(|item| item.get_mut("tools"))
                .and_then(Value::as_array_mut)
            {
                tools.extend(extra_tools);
            }
        }
    } else {
        input.insert(
            0,
            json!({
                "type": "additional_tools",
                "role": "developer",
                "tools": extra_tools,
            }),
        );
    }

    if let Some(instructions) = instructions {
        input.insert(
            1,
            json!({
                "type": "message",
                "role": "developer",
                "content": [{
                    "type": "input_text",
                    "text": instructions,
                }],
            }),
        );
    }

    if let Some(tool_type) = responses_lite_unsupported_hosted_tool(&input) {
        return Err(format!(
            "GPT-5.6 Responses Lite 不支持 hosted tool 类型 {tool_type}；请使用客户端扩展工具"
        ));
    }

    object.insert("input".to_string(), Value::Array(input));
    object.insert("parallel_tool_calls".to_string(), Value::Bool(false));
    if !object.contains_key("tool_choice") {
        object.insert("tool_choice".to_string(), Value::String("auto".to_string()));
    }

    let reasoning = object
        .entry("reasoning".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !reasoning.is_object() {
        *reasoning = Value::Object(Map::new());
    }
    if let Some(reasoning) = reasoning.as_object_mut() {
        reasoning.insert(
            "context".to_string(),
            Value::String("all_turns".to_string()),
        );
    }

    Ok(())
}

fn responses_lite_unsupported_hosted_tool(input: &[Value]) -> Option<&str> {
    let tools = input
        .first()
        .and_then(Value::as_object)
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("additional_tools"))?
        .get("tools")?
        .as_array()?;

    tools.iter().find_map(|tool| {
        let tool_type = tool.get("type").and_then(Value::as_str)?;
        matches!(
            tool_type,
            "web_search" | "web_search_preview" | "image_generation"
        )
        .then_some(tool_type)
    })
}

fn responses_lite_input_items(input: Option<Value>) -> Result<Vec<Value>, String> {
    match input {
        Some(Value::Array(items)) => Ok(items),
        Some(Value::String(text)) => Ok(vec![json!({
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": text,
            }],
        })]),
        Some(Value::Null) | None => Ok(Vec::new()),
        Some(_) => Err("GPT-5.6 Responses Lite 的 input 必须是字符串或数组".to_string()),
    }
}

fn is_remote_image_url(url: &str) -> bool {
    url.split_once(':').is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    })
}

fn is_data_image_url(url: &str) -> bool {
    url.get(.."data:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
}

fn prepare_responses_lite_images(value: &mut Value) {
    let remote_image_url = value.as_object().is_some_and(|object| {
        object.get("type").and_then(Value::as_str) == Some("input_image")
            && object
                .get("image_url")
                .and_then(Value::as_str)
                .is_some_and(is_remote_image_url)
    });
    if remote_image_url {
        *value = json!({
            "type": "input_text",
            "text": "image content omitted because remote image URLs are not supported",
        });
        return;
    }

    let unsupported_low_detail = value.as_object().is_some_and(|object| {
        object.get("type").and_then(Value::as_str) == Some("input_image")
            && object
                .get("image_url")
                .and_then(Value::as_str)
                .is_some_and(is_data_image_url)
            && object
                .get("detail")
                .and_then(Value::as_str)
                .is_some_and(|detail| detail.eq_ignore_ascii_case("low"))
    });
    if unsupported_low_detail {
        *value = json!({
            "type": "input_text",
            "text": "image content omitted because detail 'low' is not supported; use 'high', 'original', or 'auto'",
        });
        return;
    }

    match value {
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("additional_tools") {
                return;
            }
            if object.get("type").and_then(Value::as_str) == Some("input_image") {
                object.remove("detail");
            }
            for value in object.values_mut() {
                prepare_responses_lite_images(value);
            }
        }
        Value::Array(items) => {
            for item in items {
                prepare_responses_lite_images(item);
            }
        }
        _ => {}
    }
}

fn payload_for_upstream(payload: &Value) -> Value {
    let mut payload = payload.clone();
    if let Some(object) = payload.as_object_mut() {
        if object.get("service_tier").and_then(Value::as_str) == Some("default") {
            object.remove("service_tier");
        }
    }
    payload
}

fn should_use_responses_websocket(payload: &Value) -> bool {
    let _ = payload;
    false
}

fn websocket_response_create_payload(payload: &Value, turn_state: Option<&str>) -> Value {
    let mut payload = payload.clone();
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "type".to_string(),
            Value::String("response.create".to_string()),
        );
        let uses_responses_lite = payload_uses_responses_lite(&Value::Object(object.clone()));
        if uses_responses_lite || turn_state.is_some() {
            let metadata = object
                .entry("client_metadata".to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if !metadata.is_object() {
                *metadata = Value::Object(Map::new());
            }
            if let Some(metadata) = metadata.as_object_mut() {
                if uses_responses_lite {
                    metadata.insert(
                        "ws_request_header_x_openai_internal_codex_responses_lite".to_string(),
                        Value::String("true".to_string()),
                    );
                }
                if let Some(turn_state) = turn_state {
                    // Codex WebSocket 在 response.create.client_metadata 中回放 turn-state。
                    metadata.insert(
                        CODEX_TURN_STATE_HEADER.to_string(),
                        Value::String(turn_state.to_string()),
                    );
                }
            }
        }
    }
    payload
}

fn websocket_url_from_http_url(url: &str) -> Result<String, String> {
    if let Some(rest) = url.strip_prefix("https://") {
        Ok(format!("wss://{rest}"))
    } else if let Some(rest) = url.strip_prefix("http://") {
        Ok(format!("ws://{rest}"))
    } else if url.starts_with("ws://") || url.starts_with("wss://") {
        Ok(url.to_string())
    } else {
        Err(format!("无法转换 Codex WebSocket 上游地址: {url}"))
    }
}

async fn connect_codex_websocket(
    request: tokio_tungstenite::tungstenite::handshake::client::Request,
    websocket_url: &str,
) -> Result<
    (
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
        tokio_tungstenite::tungstenite::handshake::client::Response,
    ),
    String,
> {
    let Some((target_host, target_port)) = websocket_target_host_port(websocket_url) else {
        return connect_async(request)
            .await
            .map_err(|error| format!("连接 Codex WebSocket 上游失败 {websocket_url}: {error}"));
    };

    if let Some(proxy) = resolve_websocket_http_proxy(websocket_url, &target_host) {
        let stream = connect_http_proxy_tunnel(&proxy, &target_host, target_port).await?;
        return client_async_tls_with_config(request, stream, None, None)
            .await
            .map_err(|error| {
                format!(
                    "通过 {} 连接 Codex WebSocket 上游失败 {websocket_url}: {error}",
                    proxy.source
                )
            });
    }

    connect_async(request)
        .await
        .map_err(|error| format!("连接 Codex WebSocket 上游失败 {websocket_url}: {error}"))
}

async fn connect_http_proxy_tunnel(
    proxy: &HttpProxyConfig,
    target_host: &str,
    target_port: u16,
) -> Result<TcpStream, String> {
    let proxy_addr = format!("{}:{}", proxy.host, proxy.port);
    let mut stream = tokio::time::timeout(
        std::time::Duration::from_secs(DEFAULT_PROXY_CONNECT_TIMEOUT_SECS),
        TcpStream::connect(&proxy_addr),
    )
    .await
    .map_err(|_| format!("连接代理 {} 超时", proxy.source))?
    .map_err(|error| format!("连接代理 {} 失败: {error}", proxy.source))?;

    let target = format!("{target_host}:{target_port}");
    let mut request = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n");
    if let Some(authorization) = proxy.authorization.as_deref() {
        request.push_str("Proxy-Authorization: ");
        request.push_str(authorization);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");

    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|error| format!("发送代理 CONNECT 请求失败: {error}"))?;

    let mut response = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = tokio::time::timeout(
            std::time::Duration::from_secs(DEFAULT_PROXY_CONNECT_TIMEOUT_SECS),
            stream.read(&mut buffer),
        )
        .await
        .map_err(|_| format!("等待代理 CONNECT 响应超时: {}", proxy.source))?
        .map_err(|error| format!("读取代理 CONNECT 响应失败: {error}"))?;
        if read == 0 {
            return Err(format!("代理 {} 提前关闭 CONNECT 连接", proxy.source));
        }
        response.extend_from_slice(&buffer[..read]);
        if find_http_header_end(&response).is_some() {
            break;
        }
        if response.len() > MAX_PROXY_CONNECT_RESPONSE_BYTES {
            return Err(format!("代理 {} 的 CONNECT 响应过大", proxy.source));
        }
    }

    let header_end = find_http_header_end(&response)
        .ok_or_else(|| format!("代理 {} 未返回完整 CONNECT 响应", proxy.source))?;
    let header_text = String::from_utf8_lossy(&response[..header_end]);
    let status = header_text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or_default();
    if status != "200" {
        return Err(format!(
            "代理 {} 拒绝 CONNECT {target}: {}",
            proxy.source,
            header_text.lines().next().unwrap_or("empty response")
        ));
    }

    Ok(stream)
}

fn find_http_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

fn websocket_target_host_port(websocket_url: &str) -> Option<(String, u16)> {
    let parsed = reqwest::Url::parse(websocket_url).ok()?;
    let host = parsed.host_str()?.to_string();
    let port = parsed.port_or_known_default()?;
    Some((host, port))
}

fn resolve_websocket_http_proxy(websocket_url: &str, target_host: &str) -> Option<HttpProxyConfig> {
    if should_bypass_proxy_for_host(target_host) {
        return None;
    }

    let parsed = reqwest::Url::parse(websocket_url).ok()?;
    let env_vars: &[&str] = match parsed.scheme() {
        "wss" => &[
            "HTTPS_PROXY",
            "https_proxy",
            "ALL_PROXY",
            "all_proxy",
            "HTTP_PROXY",
            "http_proxy",
        ],
        "ws" => &["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"],
        _ => &[],
    };

    for env_var in env_vars {
        if let Ok(value) = std::env::var(env_var) {
            if let Some(proxy) = parse_http_proxy_config(&value, env_var) {
                return Some(proxy);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if parsed.scheme() == "wss" {
            if let Some(proxy) = macos_system_proxy_config(true) {
                return Some(proxy);
            }
        }
        macos_system_proxy_config(false)
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

fn parse_http_proxy_config(raw: &str, source: &str) -> Option<HttpProxyConfig> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let parsed = reqwest::Url::parse(&normalized).ok()?;
    if parsed.scheme() != "http" {
        return None;
    }

    let host = parsed.host_str()?.to_string();
    let port = parsed.port_or_known_default().unwrap_or(80);
    let authorization = if parsed.username().is_empty() {
        None
    } else {
        let password = parsed.password().unwrap_or_default();
        let credentials = format!("{}:{password}", parsed.username());
        Some(format!("Basic {}", BASE64_STANDARD.encode(credentials)))
    };

    Some(HttpProxyConfig {
        host,
        port,
        authorization,
        source: source.to_string(),
    })
}

#[cfg(target_os = "macos")]
fn macos_system_proxy_config(https: bool) -> Option<HttpProxyConfig> {
    let output = Command::new("scutil").arg("--proxy").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let prefix = if https { "HTTPS" } else { "HTTP" };
    if macos_proxy_value(&text, &format!("{prefix}Enable")).as_deref() != Some("1") {
        return None;
    }
    let host = macos_proxy_value(&text, &format!("{prefix}Proxy"))?;
    let port = macos_proxy_value(&text, &format!("{prefix}Port"))?
        .parse::<u16>()
        .ok()?;

    Some(HttpProxyConfig {
        host,
        port,
        authorization: None,
        source: format!("macOS {prefix} system proxy"),
    })
}

#[cfg(target_os = "macos")]
fn macos_proxy_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (name, value) = line.trim().split_once(':')?;
        (name.trim() == key).then(|| value.trim().to_string())
    })
}

fn should_bypass_proxy_for_host(host: &str) -> bool {
    let Some(no_proxy) = std::env::var("NO_PROXY")
        .ok()
        .or_else(|| std::env::var("no_proxy").ok())
    else {
        return false;
    };
    host_matches_no_proxy(host, &no_proxy)
}

fn host_matches_no_proxy(host: &str, no_proxy: &str) -> bool {
    let host = host
        .trim()
        .trim_matches('[')
        .trim_matches(']')
        .to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    let host_is_ip = host.parse::<IpAddr>().is_ok();

    no_proxy.split(',').any(|entry| {
        let entry = entry.trim().to_ascii_lowercase();
        if entry.is_empty() {
            return false;
        }
        if entry == "*" {
            return true;
        }
        let pattern = strip_no_proxy_port(&entry);
        if host_is_ip {
            return host == pattern;
        }
        if let Some(suffix) = pattern.strip_prefix("*.") {
            return host == suffix || host.ends_with(&format!(".{suffix}"));
        }
        if let Some(suffix) = pattern.strip_prefix('.') {
            return host == suffix || host.ends_with(&format!(".{suffix}"));
        }
        host == pattern || host.ends_with(&format!(".{pattern}"))
    })
}

fn strip_no_proxy_port(entry: &str) -> &str {
    if entry.starts_with('[') {
        return entry
            .split_once(']')
            .map(|(host, _)| host.trim_start_matches('['))
            .unwrap_or(entry);
    }
    match entry.rsplit_once(':') {
        Some((host, port)) if port.parse::<u16>().is_ok() => host,
        _ => entry,
    }
}

fn insert_header(headers: &mut HeaderMap, name: &'static str, value: &str) -> Result<(), String> {
    let name = axum::http::HeaderName::from_bytes(name.as_bytes())
        .map_err(|error| format!("构建上游请求头名称 {name} 失败: {error}"))?;
    let value = axum::http::HeaderValue::from_str(value)
        .map_err(|error| format!("构建上游请求头 {name} 失败: {error}"))?;
    headers.insert(name, value);
    Ok(())
}

fn websocket_event_type(data: &str) -> Option<String> {
    serde_json::from_str::<Value>(data).ok().and_then(|value| {
        value
            .get("type")
            .and_then(Value::as_str)
            .map(ToString::to_string)
    })
}

fn websocket_event_is_terminal(data: &str) -> bool {
    matches!(
        websocket_event_type(data).as_deref(),
        Some("response.completed" | "response.done" | "response.failed")
    )
}

fn normalize_model_for_client(model: &str) -> String {
    remap_model_name(model, RESPONSE_MODEL_NORMALIZATIONS).unwrap_or_else(|| model.to_string())
}

fn remap_model_name(model: &str, mappings: &[(&str, &str)]) -> Option<String> {
    for (from, to) in mappings {
        if model == *from {
            return Some((*to).to_string());
        }
        if let Some(rest) = model.strip_prefix(from) {
            if rest.starts_with('-') {
                return Some(format!("{to}{rest}"));
            }
        }
    }

    None
}

fn rewrite_response_models_for_client(mut value: Value) -> Value {
    remap_model_fields_to_client(&mut value);
    value
}

fn remap_model_fields_to_client(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, item) in object.iter_mut() {
                if key == "model" {
                    if let Some(model) = item.as_str() {
                        *item = Value::String(normalize_model_for_client(model));
                    }
                    continue;
                }
                remap_model_fields_to_client(item);
            }
        }
        Value::Array(items) => {
            for item in items {
                remap_model_fields_to_client(item);
            }
        }
        _ => {}
    }
}

fn required_string(object: &Map<String, Value>, key: &str) -> Result<String, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| format!("缺少必填字段 {key}"))
}

fn convert_message_content_to_codex_parts(role: &str, content: &Value) -> Vec<Value> {
    let text_type = if role == "assistant" {
        "output_text"
    } else {
        "input_text"
    };

    match content {
        Value::String(text) => {
            if text.is_empty() {
                Vec::new()
            } else {
                vec![json!({
                    "type": text_type,
                    "text": text,
                })]
            }
        }
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                let Some(item_object) = item.as_object() else {
                    continue;
                };
                match item_object
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                {
                    "text" => {
                        if let Some(text) = item_object.get("text").and_then(Value::as_str) {
                            parts.push(json!({
                                "type": text_type,
                                "text": text,
                            }));
                        }
                    }
                    "image_url" if role == "user" || role == "developer" || role == "system" => {
                        if let Some(url) = item_object
                            .get("image_url")
                            .and_then(|value| value.get("url"))
                            .and_then(Value::as_str)
                        {
                            parts.push(json!({
                                "type": "input_image",
                                "image_url": url,
                            }));
                        }
                    }
                    "file" if role == "user" || role == "developer" || role == "system" => {
                        let Some(file_object) = item_object.get("file").and_then(Value::as_object)
                        else {
                            continue;
                        };
                        let mut file_part = Map::new();
                        file_part
                            .insert("type".to_string(), Value::String("input_file".to_string()));
                        if let Some(file_data) =
                            file_object.get("file_data").and_then(Value::as_str)
                        {
                            file_part.insert(
                                "file_data".to_string(),
                                Value::String(file_data.to_string()),
                            );
                        }
                        if let Some(file_id) = file_object.get("file_id").and_then(Value::as_str) {
                            file_part
                                .insert("file_id".to_string(), Value::String(file_id.to_string()));
                        }
                        if let Some(filename) = file_object.get("filename").and_then(Value::as_str)
                        {
                            file_part.insert(
                                "filename".to_string(),
                                Value::String(filename.to_string()),
                            );
                        }
                        parts.push(Value::Object(file_part));
                    }
                    _ => {}
                }
            }
            parts
        }
        _ => Vec::new(),
    }
}

fn stringify_message_content(content: Option<&Value>) -> String {
    let Some(content) = content else {
        return String::new();
    };

    match content {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                item.as_object()
                    .and_then(|object| object.get("text"))
                    .and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn stringify_json_field(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
        None => String::new(),
    }
}

fn map_response_format(root: &mut Map<String, Value>, response_format: &Value) {
    let Some(response_format_object) = response_format.as_object() else {
        return;
    };
    let Some(format_type) = response_format_object.get("type").and_then(Value::as_str) else {
        return;
    };

    let text = root
        .entry("text".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !text.is_object() {
        *text = Value::Object(Map::new());
    }
    let Some(text_object) = text.as_object_mut() else {
        return;
    };
    let format = text_object
        .entry("format".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !format.is_object() {
        *format = Value::Object(Map::new());
    }
    let Some(format_object) = format.as_object_mut() else {
        return;
    };

    match format_type {
        "text" => {
            format_object.insert("type".to_string(), Value::String("text".to_string()));
        }
        "json_schema" => {
            format_object.insert("type".to_string(), Value::String("json_schema".to_string()));
            if let Some(schema_object) = response_format_object
                .get("json_schema")
                .and_then(Value::as_object)
            {
                if let Some(name) = schema_object.get("name") {
                    format_object.insert("name".to_string(), name.clone());
                }
                if let Some(strict) = schema_object.get("strict") {
                    format_object.insert("strict".to_string(), strict.clone());
                }
                if let Some(schema) = schema_object.get("schema") {
                    format_object.insert("schema".to_string(), schema.clone());
                }
            }
        }
        _ => {}
    }
}

fn map_text_settings(root: &mut Map<String, Value>, text: &Value) {
    let Some(text_value) = text.as_object() else {
        return;
    };
    let Some(verbosity) = text_value.get("verbosity") else {
        return;
    };

    let target = root
        .entry("text".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    if let Some(target_object) = target.as_object_mut() {
        target_object.insert("verbosity".to_string(), verbosity.clone());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexUpstreamEndpoint {
    Responses,
    Compact,
}

async fn send_codex_request_over_candidates(
    context: &ProxyContext,
    proxy_key: &ApiProxyKey,
    route: &str,
    endpoint: CodexUpstreamEndpoint,
    headers: &HeaderMap,
    payload: &Value,
    session_affinity_key: Option<&str>,
) -> Result<(ProxyCandidate, CodexUpstreamResponse), Response<Body>> {
    let selection = match load_proxy_candidate_selection(&context.storage).await {
        Ok(selection) => selection,
        Err(error) => {
            update_proxy_error(context, Some(error.clone())).await;
            return Err(json_error_response(StatusCode::BAD_GATEWAY, &error));
        }
    };
    if let Err(message) = ensure_api_proxy_payload_models_enabled(payload, &selection.settings) {
        update_proxy_error(context, Some(message.clone())).await;
        return Err(json_error_response(StatusCode::BAD_REQUEST, &message));
    }
    if let Err(message) = ensure_api_proxy_key_allows_payload(proxy_key, payload) {
        update_proxy_error(context, Some(message.clone())).await;
        return Err(json_error_response(StatusCode::FORBIDDEN, &message));
    }
    if selection.candidates.is_empty() {
        return Err(json_error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "No authorized account is available for proxying.",
        ));
    }
    let load_balance = selection.load_balance;
    let session_bound_account_key =
        current_sequential_proxy_account_key(context, session_affinity_key).await;
    let current_sequential_account_key = sequential_account_key_for_request(
        session_bound_account_key.clone(),
        selection.persisted_sequential_account_key,
    );
    let candidates = order_proxy_candidates_for_request(
        selection.candidates,
        load_balance,
        current_sequential_account_key.as_deref(),
        session_bound_account_key.as_deref(),
    );

    let mut attempt_errors = Vec::new();
    let mut retriable_failures = Vec::new();

    for mut candidate in candidates {
        let mut did_refresh = false;

        loop {
            log_proxy_request_route(route);
            let upstream = match forward_codex_request_with_candidate(
                context, &candidate, headers, payload, endpoint,
            )
            .await
            {
                Ok(response) => response,
                Err(error) => {
                    attempt_errors.push(format!("{}: {}", candidate.label, error));
                    break;
                }
            };

            let status = upstream.status();
            log_proxy_response_route(route, status);
            if status.is_success() {
                record_api_proxy_call_success(context, proxy_key, &candidate, route, payload).await;
                bind_sequential_session_proxy_account(
                    context,
                    session_affinity_key,
                    &candidate.account_key,
                )
                .await;
                return Ok((candidate, upstream));
            }

            let upstream_headers = upstream.headers().clone();
            let body_result = if matches!(endpoint, CodexUpstreamEndpoint::Compact) {
                upstream
                    .into_bytes_limited(MAX_COMPACT_ERROR_BODY_BYTES)
                    .await
            } else {
                upstream.into_bytes().await
            };
            let upstream_body = match body_result {
                Ok((_, bytes)) => bytes,
                Err(error) => {
                    attempt_errors
                        .push(format!("{}: 读取上游响应失败: {}", candidate.label, error));
                    break;
                }
            };

            if matches!(endpoint, CodexUpstreamEndpoint::Compact)
                && matches!(&candidate.source_kind, AccountSourceKind::Relay)
                && matches!(
                    status,
                    StatusCode::NOT_FOUND
                        | StatusCode::METHOD_NOT_ALLOWED
                        | StatusCode::NOT_IMPLEMENTED
                )
            {
                // Responses-compatible Relay 不一定实现 compact；仅跳过该候选，继续尝试 ChatGPT。
                attempt_errors.push(format!(
                    "{}: compact endpoint unsupported (HTTP {})",
                    candidate.label,
                    status.as_u16()
                ));
                break;
            }

            if !did_refresh
                && matches!(&candidate.source_kind, AccountSourceKind::Chatgpt)
                && should_retry_with_token_refresh(status, &upstream_body)
            {
                if candidate.auth_refresh_blocked {
                    unbind_session_proxy_account(
                        context,
                        session_affinity_key,
                        &candidate.account_key,
                    )
                    .await;
                    attempt_errors.push(format!(
                        "{}: {}",
                        candidate.label,
                        candidate.auth_refresh_error.clone().unwrap_or_else(|| {
                            "工具保存的授权快照已失效，请重新登录授权。".to_string()
                        })
                    ));
                    break;
                }
                match refresh_proxy_candidate_auth(&context.storage, &candidate).await {
                    Ok(refreshed_candidate) => {
                        candidate = refreshed_candidate;
                        did_refresh = true;
                        continue;
                    }
                    Err(error) => {
                        unbind_session_proxy_account(
                            context,
                            session_affinity_key,
                            &candidate.account_key,
                        )
                        .await;
                        attempt_errors
                            .push(format!("{}: 刷新登录态失败: {}", candidate.label, error));
                        break;
                    }
                }
            }

            if let Some(failure) = classify_retriable_failure(status, &upstream_body) {
                // Quota/auth/rate-limit failures are attributed to this
                // account; release the session binding so the next request
                // re-picks instead of retrying the degraded account first.
                unbind_session_proxy_account(context, session_affinity_key, &candidate.account_key)
                    .await;
                retriable_failures.push(failure);
                break;
            }

            update_proxy_error(context, None).await;
            return Err(build_proxy_response(
                status,
                &upstream_headers,
                upstream_body,
            ));
        }
    }

    let merged_error = if !retriable_failures.is_empty() && attempt_errors.is_empty() {
        build_retriable_failure_summary(&retriable_failures)
    } else if attempt_errors.is_empty() {
        "全部代理账号均不可用".to_string()
    } else {
        let base = attempt_errors
            .into_iter()
            .take(3)
            .collect::<Vec<_>>()
            .join(" | ");
        if retriable_failures.is_empty() {
            base
        } else {
            format!(
                "{base} | {}",
                build_retriable_failure_summary(&retriable_failures)
            )
        }
    };

    update_proxy_error(context, Some(merged_error.clone())).await;
    Err(json_error_response(StatusCode::BAD_GATEWAY, &merged_error))
}

async fn forward_codex_request_with_candidate(
    context: &ProxyContext,
    candidate: &ProxyCandidate,
    headers: &HeaderMap,
    payload: &Value,
    endpoint: CodexUpstreamEndpoint,
) -> Result<CodexUpstreamResponse, String> {
    let upstream_url = candidate_upstream_url(context, candidate, endpoint)?;
    let mut upstream_payload = payload_for_upstream(&payload_for_candidate(candidate, payload));
    if matches!(endpoint, CodexUpstreamEndpoint::Compact)
        && matches!(&candidate.source_kind, AccountSourceKind::Chatgpt)
    {
        upstream_payload = normalize_codex_compact_reasoning_effort(upstream_payload);
    }
    let session_id = headers
        .get("session-id")
        .or_else(|| headers.get("session_id"))
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let uses_responses_lite = payload_uses_responses_lite(&upstream_payload);
    let (version, user_agent) = upstream_codex_client_identity(headers, uses_responses_lite);

    if matches!(endpoint, CodexUpstreamEndpoint::Responses)
        && matches!(&candidate.source_kind, AccountSourceKind::Chatgpt)
        && should_use_responses_websocket(&upstream_payload)
    {
        return forward_codex_websocket_request_with_candidate(
            context,
            candidate,
            headers,
            &upstream_payload,
            &session_id,
            version,
            user_agent,
        )
        .await;
    }

    let serialized = serde_json::to_vec(&upstream_payload)
        .map_err(|error| format!("序列化上游请求失败: {error}"))?;

    let accept = match endpoint {
        CodexUpstreamEndpoint::Responses => "text/event-stream",
        CodexUpstreamEndpoint::Compact => "application/json",
    };

    let mut request = context
        .client
        .post(&upstream_url)
        .header(
            "Authorization",
            format!("Bearer {}", candidate.access_token),
        )
        .header("Accept", accept)
        .header("Content-Type", "application/json")
        .header("User-Agent", user_agent)
        // Codex 0.144 使用连字符形式；继续读取旧下划线输入，但只向上游发送正式 header。
        .header("session-id", &session_id)
        .header("Connection", "Keep-Alive");
    if uses_responses_lite {
        // Responses Lite 标记同时属于 /responses 与显式 /responses/compact 契约。
        request = request.header(RESPONSES_LITE_HEADER, "true");
    }
    for name in CODEX_CONTEXT_REQUEST_HEADERS {
        if let Some(value) = headers.get(*name) {
            // 只放行 Codex 的 turn/trace 上下文，绝不透传下游 Authorization 或 hop-by-hop 头。
            request = request.header(*name, value.clone());
        }
    }

    let request = if matches!(&candidate.source_kind, AccountSourceKind::Chatgpt) {
        let mut request = request
            .header("ChatGPT-Account-Id", &candidate.account_id)
            .header("Originator", "codex_cli_rs")
            .header("Version", version);
        if matches!(endpoint, CodexUpstreamEndpoint::Compact) {
            request = request.header("OpenAI-Beta", "responses=experimental");
        }
        request
    } else {
        request
    };

    request
        .body(serialized)
        .send()
        .await
        .map(CodexUpstreamResponse::Http)
        .map_err(|error| format!("请求上游失败 {upstream_url}: {error}"))
}

fn upstream_codex_client_identity(headers: &HeaderMap, uses_responses_lite: bool) -> (&str, &str) {
    if uses_responses_lite {
        return (CODEX_CLIENT_VERSION, CODEX_USER_AGENT);
    }

    let version = headers
        .get("version")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(CODEX_CLIENT_VERSION);
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(CODEX_USER_AGENT);
    (version, user_agent)
}

async fn forward_codex_websocket_request_with_candidate(
    context: &ProxyContext,
    candidate: &ProxyCandidate,
    downstream_headers: &HeaderMap,
    payload: &Value,
    session_id: &str,
    version: &str,
    user_agent: &str,
) -> Result<CodexUpstreamResponse, String> {
    let upstream_url = candidate_responses_url(context, candidate)?;
    let websocket_url = websocket_url_from_http_url(&upstream_url)?;
    let mut request = websocket_url
        .as_str()
        .into_client_request()
        .map_err(|error| format!("构建 Codex WebSocket 请求失败 {websocket_url}: {error}"))?;

    insert_header(
        request.headers_mut(),
        "Authorization",
        &format!("Bearer {}", candidate.access_token),
    )?;
    insert_header(
        request.headers_mut(),
        "ChatGPT-Account-Id",
        &candidate.account_id,
    )?;
    insert_header(request.headers_mut(), "Originator", "codex_cli_rs")?;
    insert_header(request.headers_mut(), "Version", version)?;
    insert_header(request.headers_mut(), "session-id", session_id)?;
    insert_header(request.headers_mut(), "x-client-request-id", session_id)?;
    insert_header(
        request.headers_mut(),
        "OpenAI-Beta",
        RESPONSES_WEBSOCKETS_BETA,
    )?;
    insert_header(request.headers_mut(), "User-Agent", user_agent)?;
    for name in CODEX_CONTEXT_REQUEST_HEADERS {
        if let Some(value) = downstream_headers.get(*name) {
            // WebSocket 握手保持与 HTTP 相同的窄上下文白名单。
            request.headers_mut().insert(*name, value.clone());
        }
    }

    let (mut websocket, response) = connect_codex_websocket(request, &websocket_url).await?;

    let turn_state = downstream_headers
        .get(CODEX_TURN_STATE_HEADER)
        .and_then(|value| value.to_str().ok());
    let request_text =
        serde_json::to_string(&websocket_response_create_payload(payload, turn_state))
            .map_err(|error| format!("序列化 Codex WebSocket 请求失败: {error}"))?;
    websocket
        .send(Message::Text(request_text.into()))
        .await
        .map_err(|error| format!("发送 Codex WebSocket 请求失败: {error}"))?;

    let headers = response.headers().clone();
    let output = stream! {
        loop {
            let message = match tokio::time::timeout(
                std::time::Duration::from_secs(DEFAULT_PROXY_UPSTREAM_TIMEOUT_SECS),
                websocket.next(),
            )
            .await
            {
                Ok(Some(message)) => message,
                Ok(None) => break,
                Err(_) => {
                    yield Err("Codex WebSocket 上游读取超时".to_string());
                    break;
                }
            };
            match message {
                Ok(Message::Text(text)) => {
                    let text = text.to_string();
                    let event_name = websocket_event_type(&text);
                    let rewritten = rewrite_sse_event_data_models_for_client(&text);
                    yield Ok::<Bytes, String>(serialize_sse_event(event_name.as_deref(), &rewritten));
                    if websocket_event_is_terminal(&text) {
                        break;
                    }
                }
                Ok(Message::Binary(_)) => {
                    yield Err("Codex WebSocket 上游返回了非预期的二进制消息".to_string());
                    break;
                }
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(payload)) => {
                    let _ = websocket.send(Message::Pong(payload)).await;
                }
                Ok(Message::Pong(_)) => {}
                Ok(Message::Frame(_)) => {}
                Err(error) => {
                    yield Err(format!("Codex WebSocket 上游中断: {error}"));
                    break;
                }
            }
        }
    };

    Ok(CodexUpstreamResponse::WebSocket {
        headers,
        stream: Box::pin(output),
    })
}

async fn load_proxy_candidates(
    storage: &ProxyStorageContext,
) -> Result<Vec<ProxyCandidate>, String> {
    load_proxy_candidate_selection(storage)
        .await
        .map(|selection| selection.candidates)
}

async fn load_proxy_candidate_selection(
    storage: &ProxyStorageContext,
) -> Result<ProxyCandidateSelection, String> {
    let _guard = storage.store_lock.lock().await;
    let store = load_store_from_path(&account_store_path_from_data_dir(&storage.data_dir))?;
    let load_balance = ProxyLoadBalanceConfig::from_settings(&store.settings);
    let settings = store.settings.clone();

    let mut deduped: HashMap<String, ProxyCandidate> = HashMap::new();
    for candidate in store
        .accounts
        .into_iter()
        .filter_map(account_to_proxy_candidate)
    {
        match deduped.get(&candidate.account_key) {
            Some(existing) if !should_replace_proxy_candidate(existing, &candidate) => {}
            _ => {
                deduped.insert(candidate.account_key.clone(), candidate);
            }
        }
    }

    Ok(ProxyCandidateSelection {
        candidates: deduped.into_values().collect(),
        load_balance,
        settings,
        persisted_sequential_account_key: store.settings.api_proxy_sequential_account_key,
    })
}

async fn load_api_proxy_settings(storage: &ProxyStorageContext) -> Result<AppSettings, String> {
    let _guard = storage.store_lock.lock().await;
    let store = load_store_from_path(&account_store_path_from_data_dir(&storage.data_dir))?;
    Ok(store.settings)
}

fn normalize_api_proxy_config_model_name(model: &str) -> Option<String> {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return None;
    }

    let upstream = map_client_model_to_upstream(trimmed).unwrap_or_else(|_| trimmed.to_string());
    Some(normalize_model_for_client(&upstream))
}

fn api_proxy_disabled_model_set(settings: &AppSettings) -> HashSet<String> {
    settings
        .api_proxy_disabled_models
        .iter()
        .filter_map(|model| normalize_api_proxy_config_model_name(model))
        .filter(|model| MODELS.contains(&model.as_str()))
        .collect()
}

fn api_proxy_visible_models(settings: &AppSettings) -> Vec<&'static str> {
    let disabled = api_proxy_disabled_model_set(settings);
    MODELS
        .iter()
        .copied()
        .filter(|model| !disabled.contains(*model))
        .collect()
}

fn api_proxy_visible_models_for_key(
    settings: &AppSettings,
    key: &ApiProxyKey,
) -> Vec<&'static str> {
    let visible = api_proxy_visible_models(settings);
    if key.allowed_models.is_empty() {
        return visible;
    }

    let allowed = key
        .allowed_models
        .iter()
        .filter_map(|model| normalize_api_proxy_config_model_name(model))
        .collect::<HashSet<_>>();
    visible
        .into_iter()
        .filter(|model| allowed.contains(*model))
        .collect()
}

fn ensure_api_proxy_payload_models_enabled(
    payload: &Value,
    settings: &AppSettings,
) -> Result<(), String> {
    let disabled = api_proxy_disabled_model_set(settings);
    let blocked = api_proxy_requested_models_from_payload(payload)
        .into_iter()
        .filter(|model| disabled.contains(model))
        .collect::<Vec<_>>();
    if blocked.is_empty() {
        return Ok(());
    }

    Err(format!(
        "请求模型已在反代控制面板中禁用: {}",
        blocked.join(", ")
    ))
}

fn ensure_api_proxy_key_allows_payload(key: &ApiProxyKey, payload: &Value) -> Result<(), String> {
    let requested_models = api_proxy_requested_models_from_payload(payload);
    if !key.allowed_models.is_empty() {
        let allowed = key
            .allowed_models
            .iter()
            .filter_map(|model| normalize_api_proxy_config_model_name(model))
            .collect::<HashSet<_>>();
        let blocked = requested_models
            .iter()
            .filter(|model| !allowed.contains(model.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !blocked.is_empty() {
            return Err(format!(
                "平台 Key {} 未绑定请求模型: {}",
                key.label,
                blocked.join(", ")
            ));
        }
    }

    if !key.allowed_reasoning_efforts.is_empty() {
        let requested = api_proxy_reasoning_effort_from_payload(payload)
            .unwrap_or_else(|| DEFAULT_API_PROXY_REASONING_EFFORT.to_string());
        if !key.allowed_reasoning_efforts.contains(&requested) {
            return Err(format!(
                "平台 Key {} 未绑定推理等级: {}",
                key.label, requested
            ));
        }
    }

    if !key.allowed_service_tiers.is_empty() {
        let requested = api_proxy_service_tier_from_payload(payload)
            .unwrap_or_else(|| DEFAULT_API_PROXY_SERVICE_TIER.to_string());
        if !key.allowed_service_tiers.contains(&requested) {
            return Err(format!(
                "平台 Key {} 未绑定服务等级: {}",
                key.label, requested
            ));
        }
    }

    Ok(())
}

fn api_proxy_requested_models_from_payload(payload: &Value) -> Vec<String> {
    let image_models = api_proxy_requested_image_tool_models(payload);
    if !image_models.is_empty() {
        return image_models;
    }

    let mut models = Vec::new();
    push_unique_api_proxy_model(
        &mut models,
        payload
            .get("model")
            .and_then(Value::as_str)
            .and_then(normalize_api_proxy_config_model_name),
    );
    models
}

fn api_proxy_requested_image_tool_models(payload: &Value) -> Vec<String> {
    let mut models = Vec::new();
    if let Some(tools) = payload.get("tools").and_then(Value::as_array) {
        collect_api_proxy_requested_image_tool_models(tools, &mut models);
    }

    if let Some(input) = payload.get("input").and_then(Value::as_array) {
        for item in input {
            let Some(item) = item.as_object() else {
                continue;
            };
            if item.get("type").and_then(Value::as_str) != Some("additional_tools") {
                continue;
            }
            if let Some(tools) = item.get("tools").and_then(Value::as_array) {
                collect_api_proxy_requested_image_tool_models(tools, &mut models);
            }
        }
    }

    models
}

fn collect_api_proxy_requested_image_tool_models(tools: &[Value], models: &mut Vec<String>) {
    for tool in tools {
        let Some(object) = tool.as_object() else {
            continue;
        };
        if object.get("type").and_then(Value::as_str) != Some("image_generation") {
            continue;
        }

        push_unique_api_proxy_model(
            models,
            object
                .get("model")
                .and_then(Value::as_str)
                .and_then(normalize_api_proxy_config_model_name),
        );
    }
}

fn push_unique_api_proxy_model(models: &mut Vec<String>, next: Option<String>) {
    let Some(next) = next else {
        return;
    };
    if models.iter().any(|model| model == &next) {
        return;
    }
    models.push(next);
}

fn api_proxy_reasoning_effort_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("reasoning")
        .and_then(|value| value.get("effort"))
        .and_then(Value::as_str)
        .or_else(|| payload.get("reasoning_effort").and_then(Value::as_str))
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| API_PROXY_REASONING_EFFORTS.contains(&value.as_str()))
}

fn api_proxy_service_tier_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("service_tier")
        .and_then(Value::as_str)
        .and_then(normalize_api_proxy_service_tier_for_log)
}

fn normalize_api_proxy_service_tier_for_log(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Some("auto".to_string()),
        "" | "default" | "standard" => Some("default".to_string()),
        "fast" | "priority" => Some("fast".to_string()),
        "flex" => Some("flex".to_string()),
        _ => None,
    }
}

fn normalize_api_proxy_service_tier_for_upstream(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => Some(DEFAULT_UPSTREAM_SERVICE_TIER.to_string()),
        "default" | "standard" => Some("default".to_string()),
        "auto" => Some("auto".to_string()),
        "fast" | "priority" => Some("priority".to_string()),
        "flex" => Some("flex".to_string()),
        _ => None,
    }
}

fn api_proxy_service_tier_for_upstream(
    request_object: &Map<String, Value>,
) -> Result<String, String> {
    match request_object.get("service_tier") {
        None => Ok(DEFAULT_UPSTREAM_SERVICE_TIER.to_string()),
        Some(value) => {
            let value = value
                .as_str()
                .ok_or_else(|| "service_tier 必须是字符串".to_string())?;
            normalize_api_proxy_service_tier_for_upstream(value)
                .ok_or_else(|| format!("不支持的推理速度: {value}"))
        }
    }
}

// Requests that omit service_tier fall back to a configurable default instead
// of the compiled-in constant: the CODEX_TOOLS_PROXY_SERVICE_TIER environment
// variable (useful for headless proxyd). Explicit per-request service_tier
// always wins over this default.
fn resolved_proxy_default_service_tier(env_value: Option<&str>) -> String {
    env_value
        .and_then(normalize_api_proxy_service_tier_for_upstream)
        .unwrap_or_else(|| DEFAULT_UPSTREAM_SERVICE_TIER.to_string())
}

fn resolve_proxy_default_service_tier() -> String {
    resolved_proxy_default_service_tier(
        std::env::var(PROXY_DEFAULT_SERVICE_TIER_ENV_VAR)
            .ok()
            .as_deref(),
    )
}

fn apply_configured_default_service_tier(
    request_specifies_tier: bool,
    payload: &mut Value,
    tier: &str,
) {
    if request_specifies_tier {
        return;
    }
    if let Some(object) = payload.as_object_mut() {
        object.insert("service_tier".to_string(), Value::String(tier.to_string()));
    }
}

fn normalize_api_proxy_reasoning_effort_for_upstream(value: &str) -> Result<String, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => Ok(DEFAULT_API_PROXY_REASONING_EFFORT.to_string()),
        // Ultra is a Codex client mode. The inference request itself uses max.
        "ultra" => Ok("max".to_string()),
        normalized if API_PROXY_REASONING_EFFORTS.contains(&normalized) => {
            Ok(normalized.to_string())
        }
        _ => Err(format!("不支持的推理强度: {value}")),
    }
}

fn account_to_proxy_candidate(account: StoredAccount) -> Option<ProxyCandidate> {
    if !account.api_proxy_enabled {
        return None;
    }

    let account_key = account.account_key();
    let variant_key = account.variant_key();
    match account.source_kind.clone() {
        AccountSourceKind::Chatgpt => {
            let extracted = extract_auth(&account.auth_json).ok()?;
            Some(ProxyCandidate {
                id: account.id,
                label: account.label,
                source_kind: AccountSourceKind::Chatgpt,
                account_key,
                account_id: extracted.account_id,
                access_token: extracted.access_token,
                auth_json: account.auth_json,
                api_base_url: None,
                model_name: None,
                variant_key,
                plan_type: account
                    .usage
                    .as_ref()
                    .and_then(|usage| usage.plan_type.clone())
                    .or(account.plan_type)
                    .or(extracted.plan_type),
                usage: account.usage,
                auth_refresh_blocked: account.auth_refresh_blocked,
                auth_refresh_error: account.auth_refresh_error,
                updated_at: account.updated_at,
            })
        }
        AccountSourceKind::Relay => {
            let api_base_url = account
                .api_base_url
                .as_deref()
                .and_then(|value| profile_files::normalize_relay_base_url(value).ok())?;
            let access_token = account
                .api_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())?
                .to_string();
            let model_name = account
                .model_name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())?
                .to_string();
            Some(ProxyCandidate {
                id: account.id,
                label: account.label,
                source_kind: AccountSourceKind::Relay,
                account_key,
                account_id: account.account_id,
                access_token,
                auth_json: account.auth_json,
                api_base_url: Some(api_base_url),
                model_name: Some(model_name),
                variant_key,
                plan_type: account.plan_type,
                usage: account.usage,
                auth_refresh_blocked: account.auth_refresh_blocked,
                auth_refresh_error: account.auth_refresh_error,
                updated_at: account.updated_at,
            })
        }
    }
}

fn candidate_responses_url(
    context: &ProxyContext,
    candidate: &ProxyCandidate,
) -> Result<String, String> {
    candidate_upstream_url(context, candidate, CodexUpstreamEndpoint::Responses)
}

fn candidate_upstream_url(
    context: &ProxyContext,
    candidate: &ProxyCandidate,
    endpoint: CodexUpstreamEndpoint,
) -> Result<String, String> {
    let base_url = match &candidate.source_kind {
        AccountSourceKind::Chatgpt => context.upstream_base_url.as_str(),
        AccountSourceKind::Relay => candidate
            .api_base_url
            .as_deref()
            .ok_or_else(|| format!("{} 缺少 API Base URL", candidate.label))?,
    };
    let suffix = match endpoint {
        CodexUpstreamEndpoint::Responses => "responses",
        CodexUpstreamEndpoint::Compact => "responses/compact",
    };
    Ok(format!("{}/{suffix}", base_url.trim_end_matches('/')))
}

fn payload_for_candidate(candidate: &ProxyCandidate, payload: &Value) -> Value {
    if !matches!(&candidate.source_kind, AccountSourceKind::Relay) {
        return payload.clone();
    }
    let mut payload = payload.clone();
    if let (Some(model_name), Some(object)) = (&candidate.model_name, payload.as_object_mut()) {
        object.insert("model".to_string(), Value::String(model_name.clone()));
    }
    payload
}

fn should_replace_proxy_candidate(existing: &ProxyCandidate, candidate: &ProxyCandidate) -> bool {
    if candidate.auth_refresh_blocked != existing.auth_refresh_blocked {
        return !candidate.auth_refresh_blocked;
    }
    candidate.updated_at > existing.updated_at
}

fn log_proxy_request_route(route: &str) {
    log::info!("API proxy request route={route}");
}

fn log_proxy_response_route(route: &str, status: StatusCode) {
    log::info!(
        "API proxy response route={route} status={}",
        status.as_u16(),
    );
}

fn order_proxy_candidates_for_request(
    mut candidates: Vec<ProxyCandidate>,
    load_balance: ProxyLoadBalanceConfig,
    current_sequential_account_key: Option<&str>,
    session_bound_account_key: Option<&str>,
) -> Vec<ProxyCandidate> {
    candidates.sort_by(compare_proxy_candidates);

    if !matches!(load_balance.mode, ApiProxyLoadBalanceMode::Sequential) {
        // Usage-based ordering otherwise re-shuffles accounts mid-conversation;
        // keep a healthy session binding pinned to protect the upstream
        // prompt-cache shard, and fall back to usage order once it degrades.
        if let Some(key) = session_bound_account_key {
            if let Some(index) = candidates
                .iter()
                .position(|candidate| candidate.account_key == key)
            {
                if can_reuse_sequential_candidate(
                    &candidates[index],
                    load_balance.sequential_five_hour_limit_percent,
                ) {
                    let bound = candidates.remove(index);
                    candidates.insert(0, bound);
                }
            }
        }
        return candidates;
    }

    let Some(current_key) = current_sequential_account_key else {
        return candidates;
    };

    if let Some(current_index) = candidates
        .iter()
        .position(|candidate| candidate.account_key == current_key)
    {
        if can_reuse_sequential_candidate(
            &candidates[current_index],
            load_balance.sequential_five_hour_limit_percent,
        ) {
            let current = candidates.remove(current_index);
            candidates.insert(0, current);
            return candidates;
        }

        candidates.sort_by(|left, right| {
            compare_sequential_switch_candidates(
                left,
                right,
                current_key,
                load_balance.sequential_five_hour_limit_percent,
            )
        });
    }

    candidates
}

fn sequential_account_key_for_request(
    runtime_account_key: Option<String>,
    persisted_account_key: Option<String>,
) -> Option<String> {
    runtime_account_key.or(persisted_account_key)
}

fn can_reuse_sequential_candidate(candidate: &ProxyCandidate, limit_percent: f64) -> bool {
    !candidate.auth_refresh_blocked && is_under_sequential_limit(candidate, limit_percent)
}

fn compare_sequential_switch_candidates(
    left: &ProxyCandidate,
    right: &ProxyCandidate,
    current_key: &str,
    limit_percent: f64,
) -> Ordering {
    sequential_switch_rank(left, current_key, limit_percent)
        .cmp(&sequential_switch_rank(right, current_key, limit_percent))
        .then_with(|| compare_proxy_candidates(left, right))
}

fn sequential_switch_rank(candidate: &ProxyCandidate, current_key: &str, limit_percent: f64) -> u8 {
    if candidate.auth_refresh_blocked {
        return 3;
    }
    if candidate.account_key == current_key {
        return 2;
    }
    if is_under_sequential_limit(candidate, limit_percent) {
        0
    } else {
        1
    }
}

fn is_under_sequential_limit(candidate: &ProxyCandidate, limit_percent: f64) -> bool {
    five_hour_used_percent(candidate)
        .map(|used_percent| used_percent < limit_percent)
        .unwrap_or(true)
}

fn compare_proxy_candidates(left: &ProxyCandidate, right: &ProxyCandidate) -> Ordering {
    match left.auth_refresh_blocked.cmp(&right.auth_refresh_blocked) {
        Ordering::Equal => {}
        ordering => return ordering,
    }

    match is_free_plan(&right.plan_type).cmp(&is_free_plan(&left.plan_type)) {
        Ordering::Equal => {}
        ordering => return ordering,
    }

    match candidate_usage_window_used_percent(
        left,
        left.usage
            .as_ref()
            .and_then(|usage| usage.one_week.as_ref()),
    )
    .total_cmp(&candidate_usage_window_used_percent(
        right,
        right
            .usage
            .as_ref()
            .and_then(|usage| usage.one_week.as_ref()),
    )) {
        Ordering::Equal => {}
        ordering => return ordering,
    }

    match candidate_usage_window_used_percent(
        left,
        left.usage
            .as_ref()
            .and_then(|usage| usage.five_hour.as_ref()),
    )
    .total_cmp(&candidate_usage_window_used_percent(
        right,
        right
            .usage
            .as_ref()
            .and_then(|usage| usage.five_hour.as_ref()),
    )) {
        Ordering::Equal => {}
        ordering => return ordering,
    }

    left.label
        .cmp(&right.label)
        .then_with(|| left.account_key.cmp(&right.account_key))
}

fn is_free_plan(plan_type: &Option<String>) -> bool {
    plan_type
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("free"))
        .unwrap_or(false)
}

fn five_hour_used_percent(candidate: &ProxyCandidate) -> Option<f64> {
    candidate
        .usage
        .as_ref()
        .and_then(|usage| usage.five_hour.as_ref())
        .and_then(finite_used_percent)
}

fn usage_window_used_percent(window: Option<&UsageWindow>) -> f64 {
    window
        .and_then(finite_used_percent)
        .unwrap_or(f64::INFINITY)
}

fn candidate_usage_window_used_percent(
    candidate: &ProxyCandidate,
    window: Option<&UsageWindow>,
) -> f64 {
    if matches!(&candidate.source_kind, AccountSourceKind::Relay) && window.is_none() {
        0.0
    } else {
        usage_window_used_percent(window)
    }
}

fn finite_used_percent(window: &UsageWindow) -> Option<f64> {
    if window.used_percent.is_finite() {
        Some(window.used_percent.clamp(0.0, 100.0))
    } else {
        None
    }
}

async fn refresh_proxy_candidate_auth(
    storage: &ProxyStorageContext,
    candidate: &ProxyCandidate,
) -> Result<ProxyCandidate, String> {
    // 串行化“读取最新快照 -> 刷新 -> 写回”，防止并发请求用旧 refresh_token 误封账号。
    let _auth_guard = storage.auth_refresh_lock.lock().await;
    if let Some(latest_auth_json) =
        latest_proxy_account_auth_json(storage, &candidate.account_key).await
    {
        if auth_json_refresh_token(&latest_auth_json).is_some()
            && auth_json_refresh_token(&latest_auth_json)
                != auth_json_refresh_token(&candidate.auth_json)
        {
            return proxy_candidate_with_auth_json(candidate, latest_auth_json);
        }
    }

    let refreshed_auth_json = match refresh_chatgpt_auth_tokens(&candidate.auth_json).await {
        Ok(refreshed) => refreshed,
        Err(error) => {
            if let Some(latest_auth_json) =
                latest_proxy_account_auth_json(storage, &candidate.account_key).await
            {
                if auth_json_refresh_token(&latest_auth_json).is_some()
                    && auth_json_refresh_token(&latest_auth_json)
                        != auth_json_refresh_token(&candidate.auth_json)
                {
                    return proxy_candidate_with_auth_json(candidate, latest_auth_json);
                }
            }

            if should_suspend_proxy_refresh(&error) {
                let normalized = normalize_proxy_refresh_error(&error);
                persist_candidate_refresh_state(
                    storage,
                    &candidate.account_key,
                    None,
                    true,
                    Some(normalized.as_str()),
                )
                .await?;
                return Err(normalized);
            }
            return Err(error);
        }
    };
    persist_candidate_refresh_state(
        storage,
        &candidate.account_key,
        Some(&refreshed_auth_json),
        false,
        None,
    )
    .await?;

    proxy_candidate_with_auth_json(candidate, refreshed_auth_json)
}

async fn latest_proxy_account_auth_json(
    storage: &ProxyStorageContext,
    account_key: &str,
) -> Option<Value> {
    let _guard = storage.store_lock.lock().await;
    let store = load_store_from_path(&account_store_path_from_data_dir(&storage.data_dir)).ok()?;
    store
        .accounts
        .into_iter()
        .filter(|account| {
            !matches!(account.source_kind, AccountSourceKind::Relay)
                && account.account_key() == account_key
        })
        .max_by(|left, right| {
            (!left.auth_refresh_blocked)
                .cmp(&(!right.auth_refresh_blocked))
                .then(left.updated_at.cmp(&right.updated_at))
        })
        .map(|account| account.auth_json)
}

fn auth_json_refresh_token(auth_json: &Value) -> Option<&str> {
    auth_json
        .get("tokens")
        .and_then(Value::as_object)
        .and_then(|tokens| tokens.get("refresh_token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn proxy_candidate_with_auth_json(
    candidate: &ProxyCandidate,
    auth_json: Value,
) -> Result<ProxyCandidate, String> {
    let extracted =
        extract_auth(&auth_json).map_err(|error| format!("刷新后解析账号登录态失败: {error}"))?;

    Ok(ProxyCandidate {
        id: candidate.id.clone(),
        label: candidate.label.clone(),
        source_kind: AccountSourceKind::Chatgpt,
        account_key: candidate.account_key.clone(),
        account_id: extracted.account_id,
        access_token: extracted.access_token,
        auth_json,
        api_base_url: None,
        model_name: None,
        variant_key: candidate.variant_key.clone(),
        plan_type: candidate.plan_type.clone().or(extracted.plan_type),
        usage: candidate.usage.clone(),
        auth_refresh_blocked: false,
        auth_refresh_error: None,
        updated_at: now_unix_seconds(),
    })
}

async fn persist_candidate_refresh_state(
    storage: &ProxyStorageContext,
    account_key: &str,
    auth_json: Option<&Value>,
    auth_refresh_blocked: bool,
    auth_refresh_error: Option<&str>,
) -> Result<(), String> {
    let _guard = storage.store_lock.lock().await;
    let store_path = account_store_path_from_data_dir(&storage.data_dir);
    update_account_group_refresh_state_in_path(
        &store_path,
        account_key,
        auth_json,
        auth_refresh_blocked,
        auth_refresh_error,
        now_unix_seconds(),
        storage.sync_active_auth_on_refresh,
    )?;
    Ok(())
}

fn should_suspend_proxy_refresh(raw_error: &str) -> bool {
    let normalized = raw_error.to_ascii_lowercase();
    normalized.contains("refresh_token_reused")
        || is_invalid_refresh_grant(&normalized)
        || normalized.contains("provided authentication token is expired")
        || normalized
            .contains("your refresh token has already been used to generate a new access token")
        || normalized.contains("refresh token expired")
        || normalized.contains("refresh_token expired")
        || normalized.contains("expired refresh token")
        || normalized.contains("refresh token is expired")
        || normalized.contains("refresh token revoked")
        || normalized.contains("refresh_token_revoked")
        || normalized.contains("refresh token invalid")
        || normalized.contains("invalid refresh token")
        || normalized.contains("please try signing in again")
        || normalized.contains("token is expired")
        || normalized.contains("account has been deactivated")
        || normalized.contains("deactivated_user")
}

fn normalize_proxy_refresh_error(raw_error: &str) -> String {
    let normalized = raw_error.to_ascii_lowercase();
    if normalized.contains("account has been deactivated")
        || normalized.contains("deactivated_user")
    {
        return "账号被封禁，请检查邮箱".to_string();
    }
    if normalized.contains("refresh_token_reused")
        || is_invalid_refresh_grant(&normalized)
        || normalized.contains("provided authentication token is expired")
        || normalized
            .contains("your refresh token has already been used to generate a new access token")
        || normalized.contains("refresh token expired")
        || normalized.contains("refresh_token expired")
        || normalized.contains("expired refresh token")
        || normalized.contains("refresh token is expired")
        || normalized.contains("refresh token revoked")
        || normalized.contains("refresh_token_revoked")
        || normalized.contains("refresh token invalid")
        || normalized.contains("invalid refresh token")
        || normalized.contains("please try signing in again")
        || normalized.contains("token is expired")
    {
        return "工具保存的授权快照已失效，请重新登录授权。".to_string();
    }
    raw_error.to_string()
}

fn is_invalid_refresh_grant(normalized_error: &str) -> bool {
    normalized_error.contains("invalid_grant")
        && (normalized_error.contains("refresh")
            || normalized_error.contains("expired")
            || normalized_error.contains("revoked")
            || normalized_error.contains("invalid"))
}

fn should_retry_with_token_refresh(status: StatusCode, body: &Bytes) -> bool {
    if status == StatusCode::UNAUTHORIZED {
        return true;
    }

    let signals = extract_error_signals(body);
    signals.normalized.contains("token expired")
        || signals.normalized.contains("jwt expired")
        || signals.normalized.contains("invalid token")
        || signals.normalized.contains("invalid_token")
        || signals.normalized.contains("session expired")
        || signals.normalized.contains("login required")
}

fn classify_retriable_failure(status: StatusCode, body: &Bytes) -> Option<RetryFailureInfo> {
    let signals = extract_error_signals(body);

    if matches!(status, StatusCode::PAYMENT_REQUIRED) || contains_quota_signal(&signals.normalized)
    {
        return Some(RetryFailureInfo {
            category: RetryFailureCategory::QuotaExceeded,
            detail: format!("额度用完：{}", signals.brief),
        });
    }

    if contains_model_restriction_signal(&signals.normalized) {
        return Some(RetryFailureInfo {
            category: RetryFailureCategory::ModelRestricted,
            detail: format!("模型受限：{}", signals.brief),
        });
    }

    if status == StatusCode::TOO_MANY_REQUESTS || contains_rate_limit_signal(&signals.normalized) {
        return Some(RetryFailureInfo {
            category: RetryFailureCategory::RateLimited,
            detail: format!("频率限制：{}", signals.brief),
        });
    }

    if status == StatusCode::UNAUTHORIZED || contains_auth_signal(&signals.normalized) {
        return Some(RetryFailureInfo {
            category: RetryFailureCategory::Authentication,
            detail: format!("鉴权失败：{}", signals.brief),
        });
    }

    if status == StatusCode::FORBIDDEN || contains_permission_signal(&signals.normalized) {
        return Some(RetryFailureInfo {
            category: RetryFailureCategory::Permission,
            detail: format!("权限不足：{}", signals.brief),
        });
    }

    None
}

struct ErrorSignals {
    normalized: String,
    brief: String,
}

fn extract_error_signals(body: &Bytes) -> ErrorSignals {
    let raw_text = String::from_utf8_lossy(body).trim().to_string();
    let mut parts = Vec::new();

    if let Ok(value) = serde_json::from_slice::<Value>(body) {
        collect_error_parts(&value, &mut parts);
    }

    if parts.is_empty() && !raw_text.is_empty() {
        parts.push(raw_text.clone());
    }

    let joined = parts
        .into_iter()
        .filter(|item| !item.trim().is_empty())
        .fold(Vec::<String>::new(), |mut acc, item| {
            if !acc.iter().any(|existing| existing == &item) {
                acc.push(item);
            }
            acc
        })
        .join(" | ");
    let brief = if joined.is_empty() {
        "未返回具体错误信息".to_string()
    } else {
        truncate_for_error(&joined, 120)
    };

    ErrorSignals {
        normalized: format!("{} {}", joined, raw_text).to_ascii_lowercase(),
        brief,
    }
}

fn collect_error_parts(value: &Value, parts: &mut Vec<String>) {
    if let Some(error) = value.get("error") {
        if let Some(message) = error.get("message").and_then(Value::as_str) {
            parts.push(message.trim().to_string());
        }
        if let Some(code) = error.get("code").and_then(Value::as_str) {
            parts.push(code.trim().to_string());
        }
        if let Some(kind) = error.get("type").and_then(Value::as_str) {
            parts.push(kind.trim().to_string());
        }
    }

    if let Some(message) = value.get("message").and_then(Value::as_str) {
        parts.push(message.trim().to_string());
    }
}

fn contains_quota_signal(text: &str) -> bool {
    text.contains("insufficient_quota")
        || text.contains("quota exceeded")
        || text.contains("usage_limit")
        || text.contains("usage limit")
        || text.contains("credit balance")
        || text.contains("billing hard limit")
        || text.contains("exceeded your current quota")
        || text.contains("usage_limit_reached")
}

fn contains_rate_limit_signal(text: &str) -> bool {
    text.contains("rate limit")
        || text.contains("rate_limit")
        || text.contains("too many requests")
        || text.contains("requests per min")
        || text.contains("tokens per min")
        || text.contains("retry after")
        || text.contains("requests too quickly")
}

fn contains_model_restriction_signal(text: &str) -> bool {
    text.contains("model_not_found")
        || text.contains("does not have access to model")
        || text.contains("do not have access to model")
        || text.contains("access to model")
        || text.contains("unsupported model")
        || text.contains("model is not supported")
        || text.contains("not available on your account")
        || text.contains("model access")
}

fn contains_auth_signal(text: &str) -> bool {
    text.contains("invalid_api_key")
        || text.contains("invalid api key")
        || text.contains("authentication")
        || text.contains("unauthorized")
        || text.contains("token expired")
        || text.contains("account deactivated")
        || text.contains("invalid token")
}

fn contains_permission_signal(text: &str) -> bool {
    text.contains("permission")
        || text.contains("forbidden")
        || text.contains("not allowed")
        || text.contains("organization")
        || text.contains("access denied")
}

fn build_retriable_failure_summary(failures: &[RetryFailureInfo]) -> String {
    let mut quota = 0usize;
    let mut rate = 0usize;
    let mut model = 0usize;
    let mut auth = 0usize;
    let mut permission = 0usize;

    for failure in failures {
        match failure.category {
            RetryFailureCategory::QuotaExceeded => quota += 1,
            RetryFailureCategory::RateLimited => rate += 1,
            RetryFailureCategory::ModelRestricted => model += 1,
            RetryFailureCategory::Authentication => auth += 1,
            RetryFailureCategory::Permission => permission += 1,
        }
    }

    let mut parts = Vec::new();
    if quota > 0 {
        parts.push(format!("额度用完 {quota} 个"));
    }
    if rate > 0 {
        parts.push(format!("频率限制 {rate} 个"));
    }
    if model > 0 {
        parts.push(format!("模型受限 {model} 个"));
    }
    if auth > 0 {
        parts.push(format!("鉴权失败 {auth} 个"));
    }
    if permission > 0 {
        parts.push(format!("权限不足 {permission} 个"));
    }
    let sample = failures
        .iter()
        .map(|item| item.detail.as_str())
        .find(|detail| !detail.trim().is_empty());

    let mut message = format!(
        "本次尝试的 {} 个账号全部被上游拒绝：{}。",
        failures.len(),
        parts.join("，")
    );
    if let Some(sample) = sample {
        message.push_str(" 示例：");
        message.push_str(sample);
    }
    message
}

async fn read_persisted_api_proxy_key(
    storage: &ProxyStorageContext,
) -> Result<Option<String>, String> {
    ensure_persisted_api_proxy_keys(storage)
        .await
        .map(|keys| primary_api_proxy_key_value(&keys))
}

async fn read_persisted_api_proxy_port(storage: &ProxyStorageContext) -> Result<u16, String> {
    let _guard = storage.store_lock.lock().await;
    let store = load_store_from_path(&account_store_path_from_data_dir(&storage.data_dir))?;
    Ok(if store.settings.api_proxy_port == 0 {
        DEFAULT_PROXY_PORT
    } else {
        store.settings.api_proxy_port
    })
}

async fn ensure_persisted_api_proxy_keys(
    storage: &ProxyStorageContext,
) -> Result<Vec<ApiProxyKey>, String> {
    load_or_initialize_api_proxy_key_store(storage)
        .await
        .map(|store| store.keys)
}

async fn load_or_initialize_api_proxy_key_store(
    storage: &ProxyStorageContext,
) -> Result<ApiProxyKeyStore, String> {
    let _guard = storage.store_lock.lock().await;
    let path = api_proxy_keys_path(storage)?;
    let mut key_store = load_api_proxy_key_store_from_path(&path)?;
    if !key_store.keys.is_empty() {
        normalize_api_proxy_key_store(&mut key_store);
        return Ok(key_store);
    }

    let store_path = account_store_path_from_data_dir(&storage.data_dir);
    let account_store = load_store_from_path(&store_path)?;
    let legacy_key = read_api_proxy_key_file(storage)?.or_else(|| {
        account_store
            .settings
            .api_proxy_api_key
            .clone()
            .filter(|value| !value.trim().is_empty())
    });
    let now = now_unix_seconds();
    key_store.keys.push(default_api_proxy_key(now, legacy_key));
    key_store.updated_at = now;
    normalize_api_proxy_key_store(&mut key_store);
    save_api_proxy_key_store_to_path(&path, &key_store)?;

    if let Some(primary) = primary_api_proxy_key_value(&key_store.keys) {
        write_api_proxy_key_file(storage, &primary)?;
    }

    Ok(key_store)
}

async fn save_api_proxy_key_store(
    storage: &ProxyStorageContext,
    store: &ApiProxyKeyStore,
) -> Result<(), String> {
    let _guard = storage.store_lock.lock().await;
    let path = api_proxy_keys_path(storage)?;
    save_api_proxy_key_store_to_path(&path, store)?;
    if let Some(primary) = primary_api_proxy_key_value(&store.keys) {
        write_api_proxy_key_file(storage, &primary)?;
    }
    Ok(())
}

async fn refresh_runtime_api_proxy_keys(
    storage: &ProxyStorageContext,
    runtime_slot: &tokio::sync::Mutex<Option<ApiProxyRuntimeHandle>>,
) -> Result<(), String> {
    let keys = ensure_persisted_api_proxy_keys(storage).await?;
    let guard = runtime_slot.lock().await;
    if let Some(handle) = guard.as_ref() {
        if let Ok(mut key_guard) = handle.api_keys.write() {
            *key_guard = keys;
        }
    }
    Ok(())
}

fn generate_api_proxy_key() -> String {
    format!("sk-{}", uuid::Uuid::new_v4().simple())
}

fn read_api_proxy_key_file(storage: &ProxyStorageContext) -> Result<Option<String>, String> {
    let path = api_proxy_key_path(storage)?;
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("读取 API Key 存储失败 {}: {error}", path.display()))?;
    let value = raw.trim();
    if value.is_empty() {
        return Ok(None);
    }

    Ok(Some(value.to_string()))
}

fn write_api_proxy_key_file(storage: &ProxyStorageContext, api_key: &str) -> Result<(), String> {
    let path = api_proxy_key_path(storage)?;
    write_private_file_atomically(&path, api_key.as_bytes())
}

fn api_proxy_key_path(storage: &ProxyStorageContext) -> Result<PathBuf, String> {
    Ok(storage.data_dir.join("api-proxy.key"))
}

fn api_proxy_keys_path(storage: &ProxyStorageContext) -> Result<PathBuf, String> {
    Ok(storage.data_dir.join(API_PROXY_KEYS_FILE_NAME))
}

fn load_api_proxy_key_store_from_path(path: &Path) -> Result<ApiProxyKeyStore, String> {
    if !path.exists() {
        return Ok(ApiProxyKeyStore::default());
    }

    let raw = fs::read_to_string(path)
        .map_err(|error| format!("读取平台 Key 存储失败 {}: {error}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(ApiProxyKeyStore::default());
    }

    match serde_json::from_str::<ApiProxyKeyStore>(&raw) {
        Ok(mut store) => {
            store.version = API_PROXY_KEYS_STORE_VERSION;
            Ok(store)
        }
        Err(error) => {
            log::warn!(
                "平台 Key 存储格式无效，已重新初始化 {}: {}",
                path.display(),
                error
            );
            Ok(ApiProxyKeyStore::default())
        }
    }
}

fn save_api_proxy_key_store_to_path(path: &Path, store: &ApiProxyKeyStore) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(store)
        .map_err(|error| format!("序列化平台 Key 失败: {error}"))?;
    write_private_named_file_atomically(path, serialized.as_bytes(), "平台 Key")
}

fn normalize_api_proxy_key_store(store: &mut ApiProxyKeyStore) {
    let now = now_unix_seconds();
    let mut seen_ids = HashSet::new();
    let mut seen_keys = HashSet::new();
    let mut normalized = Vec::new();

    for mut key in std::mem::take(&mut store.keys) {
        if key.key.trim().is_empty() || seen_keys.contains(&key.key) {
            continue;
        }
        if key.id.trim().is_empty() || seen_ids.contains(&key.id) {
            key.id = uuid::Uuid::new_v4().to_string();
        }
        if key.label.trim().is_empty() {
            key.label = "Default".to_string();
        }
        if key.created_at <= 0 {
            key.created_at = now;
        }
        if key.updated_at <= 0 {
            key.updated_at = key.created_at;
        }
        key.allowed_models = sanitize_api_proxy_allowed_models(key.allowed_models);
        key.allowed_reasoning_efforts =
            sanitize_api_proxy_reasoning_efforts(key.allowed_reasoning_efforts);
        key.allowed_service_tiers = sanitize_api_proxy_service_tiers(key.allowed_service_tiers);

        seen_ids.insert(key.id.clone());
        seen_keys.insert(key.key.clone());
        normalized.push(key);
    }

    if normalized.is_empty() {
        normalized.push(default_api_proxy_key(now, None));
    }
    store.version = API_PROXY_KEYS_STORE_VERSION;
    store.keys = normalized;
}

fn default_api_proxy_key(now: i64, key: Option<String>) -> ApiProxyKey {
    ApiProxyKey {
        id: uuid::Uuid::new_v4().to_string(),
        label: "Default".to_string(),
        key: key.unwrap_or_else(generate_api_proxy_key),
        enabled: true,
        allowed_models: Vec::new(),
        allowed_reasoning_efforts: Vec::new(),
        allowed_service_tiers: Vec::new(),
        created_at: now,
        updated_at: now,
    }
}

fn normalized_api_proxy_key_label(label: &str) -> String {
    let value = label.trim();
    if value.is_empty() {
        "New key".to_string()
    } else {
        value.chars().take(80).collect()
    }
}

fn normalized_custom_api_proxy_key(value: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() < 12 {
        return Err("平台 Key 至少需要 12 个字符。".to_string());
    }
    Ok(Some(value.to_string()))
}

fn primary_api_proxy_key_value(keys: &[ApiProxyKey]) -> Option<String> {
    keys.iter()
        .find(|key| key.enabled)
        .or_else(|| keys.first())
        .map(|key| key.key.clone())
}

fn read_authorized_api_proxy_key(
    headers: &HeaderMap,
    api_keys: &Arc<RwLock<Vec<ApiProxyKey>>>,
) -> Option<ApiProxyKey> {
    let token = api_proxy_request_token(headers)?;
    let keys = api_keys.read().ok()?;
    keys.iter()
        // 平台 Key 属于认证凭据，统一使用常量时间比较，避免响应耗时泄漏前缀信息。
        .find(|key| {
            key.enabled && constant_time_eq::constant_time_eq(key.key.as_bytes(), token.as_bytes())
        })
        .cloned()
}

fn api_proxy_request_token(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_string());
    }

    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn sanitize_api_proxy_allowed_models(models: Vec<String>) -> Vec<String> {
    let selected = models
        .into_iter()
        .filter_map(|model| normalize_api_proxy_config_model_name(&model))
        .filter(|model| MODELS.contains(&model.as_str()))
        .collect::<HashSet<_>>();

    MODELS
        .iter()
        .filter(|model| selected.contains(**model))
        .map(|model| (*model).to_string())
        .collect()
}

fn sanitize_api_proxy_reasoning_efforts(values: Vec<String>) -> Vec<String> {
    sanitize_api_proxy_named_options(values, API_PROXY_REASONING_EFFORTS)
}

fn sanitize_api_proxy_service_tiers(values: Vec<String>) -> Vec<String> {
    let selected = values
        .into_iter()
        .filter_map(|value| normalize_api_proxy_service_tier_for_log(&value))
        .filter(|value| API_PROXY_SERVICE_TIERS.contains(&value.as_str()))
        .collect::<HashSet<_>>();

    API_PROXY_SERVICE_TIERS
        .iter()
        .filter(|value| selected.contains(**value))
        .map(|value| (*value).to_string())
        .collect()
}

fn sanitize_api_proxy_named_options(values: Vec<String>, allowed: &[&str]) -> Vec<String> {
    let selected = values
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| allowed.contains(&value.as_str()))
        .collect::<HashSet<_>>();

    allowed
        .iter()
        .filter(|value| selected.contains(**value))
        .map(|value| (*value).to_string())
        .collect()
}

fn api_proxy_usage_database_path(data_dir: &Path) -> PathBuf {
    data_dir.join(API_PROXY_USAGE_DATABASE_FILE_NAME)
}

async fn api_proxy_usage_sender(
    storage: &ProxyStorageContext,
) -> Result<mpsc::Sender<ApiProxyUsageCommand>, String> {
    let mut writer = storage.usage_writer.lock().await;
    if writer.is_none() {
        *writer = Some(ApiProxyUsageWriter::start(storage.data_dir.clone())?);
    }
    writer.as_ref().expect("usage writer initialized").sender()
}

async fn record_api_proxy_call_success(
    context: &ProxyContext,
    proxy_key: &ApiProxyKey,
    candidate: &ProxyCandidate,
    route: &str,
    payload: &Value,
) {
    let metadata = api_proxy_usage_metadata(proxy_key, candidate, route, payload);
    if let Err(error) = append_api_proxy_usage_event(
        &context.storage,
        api_proxy_usage_event(&metadata, 1, 0, now_unix_seconds()),
    )
    .await
    {
        log::warn!("记录 API 反代调用统计失败 route={route}: {error}");
    }
}

async fn record_api_proxy_tokens_from_response(
    storage: &ProxyStorageContext,
    metadata: &ApiProxyUsageMetadata,
    response: &Value,
) {
    let Some(usage) = api_proxy_usage_tokens_from_response(response) else {
        return;
    };

    if let Err(error) = append_api_proxy_usage_event(
        storage,
        api_proxy_usage_token_event(metadata, usage, now_unix_seconds()),
    )
    .await
    {
        log::warn!(
            "记录 API 反代 token 统计失败 route={} model={}: {}",
            metadata.route,
            metadata.model,
            error
        );
    }
}

async fn maybe_record_stream_usage_tokens(
    storage: &ProxyStorageContext,
    metadata: &ApiProxyUsageMetadata,
    event: &SseEvent,
    recorded_usage: &mut bool,
) {
    if *recorded_usage {
        return;
    }
    let Some(usage) = api_proxy_usage_tokens_from_sse_event(event) else {
        return;
    };

    *recorded_usage = true;
    // 终止事件对客户端可见前先完成轻量 mpsc 入队，使随后导出的 FIFO barrier 不会越过本次 token。
    if let Err(error) = append_api_proxy_usage_event(
        storage,
        api_proxy_usage_token_event(metadata, usage, now_unix_seconds()),
    )
    .await
    {
        log::warn!(
            "记录 API 反代流式 token 统计失败 route={} model={}: {}",
            metadata.route,
            metadata.model,
            error
        );
    }
}

fn api_proxy_usage_metadata(
    proxy_key: &ApiProxyKey,
    _candidate: &ProxyCandidate,
    route: &str,
    payload: &Value,
) -> ApiProxyUsageMetadata {
    ApiProxyUsageMetadata {
        key_id: Some(proxy_key.id.clone()),
        key_label: Some(proxy_key.label.clone()),
        model: api_proxy_usage_model_from_payload(payload),
        route: route.to_string(),
        reasoning_effort: api_proxy_reasoning_effort_from_payload(payload),
        service_tier: api_proxy_service_tier_from_payload(payload),
    }
}

fn api_proxy_usage_model_from_payload(payload: &Value) -> String {
    api_proxy_usage_image_tool_model_from_payload(payload)
        .or_else(|| {
            payload
                .get("model")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|model| !model.is_empty())
                .map(normalize_model_for_client)
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn api_proxy_usage_image_tool_model_from_payload(payload: &Value) -> Option<String> {
    api_proxy_requested_image_tool_models(payload)
        .into_iter()
        .next()
}

fn api_proxy_usage_event(
    metadata: &ApiProxyUsageMetadata,
    calls: i64,
    tokens: i64,
    timestamp: i64,
) -> ApiProxyUsageEvent {
    ApiProxyUsageEvent {
        timestamp,
        key_id: metadata.key_id.clone(),
        key_label: metadata.key_label.clone(),
        model: metadata.model.clone(),
        route: Some(metadata.route.clone()),
        reasoning_effort: metadata.reasoning_effort.clone(),
        service_tier: metadata.service_tier.clone(),
        calls,
        tokens,
        input_tokens: None,
        output_tokens: None,
        cached_input_tokens: None,
    }
}

fn api_proxy_usage_token_event(
    metadata: &ApiProxyUsageMetadata,
    usage: ApiProxyTokenUsageBreakdown,
    timestamp: i64,
) -> ApiProxyUsageEvent {
    ApiProxyUsageEvent {
        timestamp,
        key_id: metadata.key_id.clone(),
        key_label: metadata.key_label.clone(),
        model: metadata.model.clone(),
        route: Some(metadata.route.clone()),
        reasoning_effort: metadata.reasoning_effort.clone(),
        service_tier: metadata.service_tier.clone(),
        calls: 0,
        tokens: usage.total_tokens,
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cached_input_tokens: usage.cached_input_tokens,
    }
}

async fn append_api_proxy_usage_event(
    storage: &ProxyStorageContext,
    event: ApiProxyUsageEvent,
) -> Result<(), String> {
    let Some(event) = normalize_api_proxy_usage_event(event, now_unix_seconds()) else {
        return Ok(());
    };
    api_proxy_usage_sender(storage)
        .await?
        .send(ApiProxyUsageCommand::Append(event))
        .map_err(|_| "API proxy usage writer is unavailable".to_string())
}

fn normalize_api_proxy_usage_event(
    mut event: ApiProxyUsageEvent,
    now: i64,
) -> Option<ApiProxyUsageEvent> {
    event.calls = event.calls.max(0);
    event.tokens = event.tokens.max(0);
    event.input_tokens = event.input_tokens.map(|value| value.max(0));
    event.output_tokens = event.output_tokens.map(|value| value.max(0));
    event.cached_input_tokens = event.cached_input_tokens.map(|value| value.max(0));
    if event.calls == 0 && event.tokens == 0 {
        return None;
    }
    if event.model.trim().is_empty() {
        event.model = "unknown".to_string();
    }
    if event.timestamp <= 0 {
        event.timestamp = now;
    }
    Some(event)
}

fn load_api_proxy_usage_store_from_path(path: &Path) -> Result<ApiProxyUsageStore, String> {
    if !path.exists() {
        return Ok(ApiProxyUsageStore::default());
    }

    let raw = fs::read_to_string(path)
        .map_err(|error| format!("读取 API 反代统计存储失败 {}: {error}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(ApiProxyUsageStore::default());
    }

    match serde_json::from_str::<ApiProxyUsageStore>(&raw) {
        Ok(mut store) => {
            store.version = API_PROXY_USAGE_STORE_VERSION;
            Ok(store)
        }
        Err(error) => {
            log::warn!(
                "API 反代统计存储格式无效，已忽略 {}: {}",
                path.display(),
                error
            );
            Ok(ApiProxyUsageStore::default())
        }
    }
}

fn run_api_proxy_usage_writer(
    data_dir: PathBuf,
    receiver: mpsc::Receiver<ApiProxyUsageCommand>,
    ready: mpsc::SyncSender<Result<(), String>>,
) {
    let mut connection = match open_api_proxy_usage_database(&data_dir) {
        Ok(connection) => connection,
        Err(error) => {
            log::warn!("API proxy usage database unavailable: {error}");
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        return;
    }
    let mut pending = None;

    loop {
        let command = match pending.take().or_else(|| receiver.recv().ok()) {
            Some(command) => command,
            None => break,
        };

        match command {
            ApiProxyUsageCommand::Append(first) => {
                let mut events = vec![first];
                while events.len() < 256 {
                    match receiver.try_recv() {
                        Ok(ApiProxyUsageCommand::Append(event)) => events.push(event),
                        Ok(command) => {
                            pending = Some(command);
                            break;
                        }
                        Err(_) => break,
                    }
                }
                if let Err(error) = insert_api_proxy_usage_events(&mut connection, &events) {
                    log::warn!("Failed to persist API proxy usage: {error}");
                }
            }
            ApiProxyUsageCommand::QueryKeyLogs { limit, reply } => {
                let _ = reply.send(query_api_proxy_key_usage_logs(&connection, limit));
            }
            ApiProxyUsageCommand::QueryStats {
                now,
                range_seconds,
                reply,
            } => {
                let _ = reply.send(query_api_proxy_usage_stats(&connection, now, range_seconds));
            }
            ApiProxyUsageCommand::Barrier { reply } => {
                let _ = reply.send(());
            }
            ApiProxyUsageCommand::Clear { reply } => {
                let result = clear_api_proxy_usage_history(&mut connection, &data_dir);
                let _ = reply.send(result);
            }
        }
    }
}

fn clear_api_proxy_usage_history(
    connection: &mut Connection,
    data_dir: &Path,
) -> Result<(), String> {
    let transaction = connection
        .transaction()
        .map_err(|error| format!("Failed to start API proxy usage clear: {error}"))?;
    transaction
        .execute("DELETE FROM api_proxy_usage_events", [])
        .map_err(|error| format!("Failed to clear API proxy usage: {error}"))?;
    clear_legacy_api_proxy_usage_file(data_dir)?;
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit API proxy usage clear: {error}"))
}

fn clear_legacy_api_proxy_usage_file(data_dir: &Path) -> Result<(), String> {
    let legacy_path = data_dir.join(API_PROXY_USAGE_FILE_NAME);
    match fs::remove_file(&legacy_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Failed to remove legacy API proxy usage {}: {error}",
            legacy_path.display()
        )),
    }
}

fn open_api_proxy_usage_database(data_dir: &Path) -> Result<Connection, String> {
    fs::create_dir_all(data_dir).map_err(|error| {
        format!(
            "Failed to create API proxy usage directory {}: {error}",
            data_dir.display()
        )
    })?;
    let database_path = api_proxy_usage_database_path(data_dir);
    let mut connection = Connection::open(&database_path).map_err(|error| {
        format!(
            "Failed to open API proxy usage database {}: {error}",
            database_path.display()
        )
    })?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| format!("Failed to configure API proxy usage timeout: {error}"))?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|error| format!("Failed to enable API proxy usage WAL: {error}"))?;
    connection
        .pragma_update(None, "synchronous", "NORMAL")
        .map_err(|error| format!("Failed to configure API proxy usage sync mode: {error}"))?;
    connection
        .pragma_update(None, "temp_store", "MEMORY")
        .map_err(|error| format!("Failed to keep API proxy temporary data in memory: {error}"))?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS api_proxy_usage_events (
                id INTEGER PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                key_id TEXT,
                key_label TEXT,
                model TEXT NOT NULL,
                route TEXT,
                reasoning_effort TEXT,
                service_tier TEXT,
                calls INTEGER NOT NULL,
                tokens INTEGER NOT NULL,
                input_tokens INTEGER,
                output_tokens INTEGER,
                cached_input_tokens INTEGER
            );
            CREATE INDEX IF NOT EXISTS api_proxy_usage_timestamp_idx
                ON api_proxy_usage_events(timestamp);
            CREATE INDEX IF NOT EXISTS api_proxy_usage_key_timestamp_idx
                ON api_proxy_usage_events(key_id, timestamp);
            CREATE TABLE IF NOT EXISTS api_proxy_usage_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .map_err(|error| format!("Failed to initialize API proxy usage database: {error}"))?;
    ensure_api_proxy_usage_breakdown_columns(&mut connection)?;
    set_private_permissions(&database_path);
    migrate_legacy_api_proxy_usage(&mut connection, data_dir)?;
    Ok(connection)
}

fn ensure_api_proxy_usage_breakdown_columns(connection: &mut Connection) -> Result<(), String> {
    // IMMEDIATE 事务串行化多个 desktop/proxyd 进程的首次 schema 升级，避免重复 ADD COLUMN。
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Failed to start API proxy usage schema migration: {error}"))?;
    let mut statement = transaction
        .prepare("PRAGMA table_info(api_proxy_usage_events)")
        .map_err(|error| format!("Failed to inspect API proxy usage columns: {error}"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("Failed to query API proxy usage columns: {error}"))?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|error| format!("Failed to read API proxy usage columns: {error}"))?;
    drop(statement);

    // SQLite 为可空、无默认值列执行 ADD COLUMN 只改 schema，旧库升级为 O(1)，不会重写历史事件表。
    for (column, sql) in [
        (
            "input_tokens",
            "ALTER TABLE api_proxy_usage_events ADD COLUMN input_tokens INTEGER",
        ),
        (
            "output_tokens",
            "ALTER TABLE api_proxy_usage_events ADD COLUMN output_tokens INTEGER",
        ),
        (
            "cached_input_tokens",
            "ALTER TABLE api_proxy_usage_events ADD COLUMN cached_input_tokens INTEGER",
        ),
    ] {
        if !columns.contains(column) {
            transaction.execute_batch(sql).map_err(|error| {
                format!("Failed to add API proxy usage column {column}: {error}")
            })?;
        }
    }
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit API proxy usage schema migration: {error}"))
}

fn migrate_legacy_api_proxy_usage(
    connection: &mut Connection,
    data_dir: &Path,
) -> Result<(), String> {
    let migrated = connection
        .query_row(
            "SELECT value FROM api_proxy_usage_metadata WHERE key = ?1",
            [API_PROXY_USAGE_LEGACY_MIGRATION_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("Failed to inspect API proxy usage migration: {error}"))?;
    if migrated.is_some() {
        return Ok(());
    }

    let legacy_path = data_dir.join(API_PROXY_USAGE_FILE_NAME);
    let legacy_store = load_api_proxy_usage_store_from_path(&legacy_path)?;
    let now = now_unix_seconds();
    let events = legacy_store
        .events
        .into_iter()
        .filter_map(|event| normalize_api_proxy_usage_event(event, now))
        .collect::<Vec<_>>();
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Failed to start API proxy usage migration: {error}"))?;
    let migrated = transaction
        .query_row(
            "SELECT value FROM api_proxy_usage_metadata WHERE key = ?1",
            [API_PROXY_USAGE_LEGACY_MIGRATION_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("Failed to recheck API proxy usage migration: {error}"))?;
    if migrated.is_some() {
        return transaction
            .commit()
            .map_err(|error| format!("Failed to finish API proxy usage migration check: {error}"));
    }
    {
        let mut insert = transaction
            .prepare_cached(
                "INSERT INTO api_proxy_usage_events (
                    timestamp, key_id, key_label, model, route,
                    reasoning_effort, service_tier, calls, tokens,
                    input_tokens, output_tokens, cached_input_tokens
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )
            .map_err(|error| format!("Failed to prepare API proxy usage migration: {error}"))?;
        for event in &events {
            insert_api_proxy_usage_event(&mut insert, event)?;
        }
    }
    transaction
        .execute(
            "INSERT INTO api_proxy_usage_metadata (key, value) VALUES (?1, ?2)",
            params![
                API_PROXY_USAGE_LEGACY_MIGRATION_KEY,
                events.len().to_string()
            ],
        )
        .map_err(|error| format!("Failed to mark API proxy usage migration: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit API proxy usage migration: {error}"))?;
    Ok(())
}

fn insert_api_proxy_usage_events(
    connection: &mut Connection,
    events: &[ApiProxyUsageEvent],
) -> Result<(), String> {
    let transaction = connection
        .transaction()
        .map_err(|error| format!("Failed to start API proxy usage transaction: {error}"))?;
    {
        let mut insert = transaction
            .prepare_cached(
                "INSERT INTO api_proxy_usage_events (
                    timestamp, key_id, key_label, model, route,
                    reasoning_effort, service_tier, calls, tokens,
                    input_tokens, output_tokens, cached_input_tokens
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )
            .map_err(|error| format!("Failed to prepare API proxy usage insert: {error}"))?;
        for event in events {
            insert_api_proxy_usage_event(&mut insert, event)?;
        }
    }
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit API proxy usage: {error}"))
}

fn insert_api_proxy_usage_event(
    insert: &mut rusqlite::CachedStatement<'_>,
    event: &ApiProxyUsageEvent,
) -> Result<(), String> {
    insert
        .execute(params![
            event.timestamp,
            event.key_id.as_deref(),
            event.key_label.as_deref(),
            event.model.as_str(),
            event.route.as_deref(),
            event.reasoning_effort.as_deref(),
            event.service_tier.as_deref(),
            event.calls,
            event.tokens,
            event.input_tokens,
            event.output_tokens,
            event.cached_input_tokens,
        ])
        .map(|_| ())
        .map_err(|error| format!("Failed to insert API proxy usage: {error}"))
}

fn query_api_proxy_key_usage_logs(
    connection: &Connection,
    limit: usize,
) -> Result<Vec<ApiProxyKeyUsageLogEntry>, String> {
    let mut statement = connection
        .prepare_cached(
            "SELECT timestamp, key_id, key_label, model, route,
                    reasoning_effort, service_tier, calls, tokens
             FROM api_proxy_usage_events
             WHERE key_id IS NOT NULL OR key_label IS NOT NULL
             ORDER BY timestamp DESC, id DESC
             LIMIT ?1",
        )
        .map_err(|error| format!("Failed to prepare API proxy usage log query: {error}"))?;
    let entries = statement
        .query_map([limit as i64], |row| {
            Ok(ApiProxyKeyUsageLogEntry {
                timestamp: row.get(0)?,
                key_id: row.get(1)?,
                key_label: row.get(2)?,
                model: row.get(3)?,
                route: row.get(4)?,
                reasoning_effort: row.get(5)?,
                service_tier: row.get(6)?,
                calls: row.get::<_, i64>(7)?.max(0),
                tokens: row.get::<_, i64>(8)?.max(0),
            })
        })
        .map_err(|error| format!("Failed to query API proxy usage logs: {error}"))?;
    entries
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read API proxy usage logs: {error}"))
}

fn query_api_proxy_usage_stats(
    connection: &Connection,
    now: i64,
    range_seconds: i64,
) -> Result<ApiProxyUsageStats, String> {
    let range_seconds = normalize_api_proxy_usage_range_seconds(Some(range_seconds));
    let bucket_seconds = api_proxy_usage_bucket_seconds(range_seconds);
    let start = now.saturating_sub(range_seconds);
    let mut statement = connection
        .prepare_cached(
            "SELECT (timestamp / ?3) * ?3 AS bucket_timestamp,
                    key_id, key_label, TRIM(model) AS normalized_model,
                    SUM(CASE WHEN calls > 0 THEN calls ELSE 0 END) AS calls,
                    SUM(CASE WHEN tokens > 0 THEN tokens ELSE 0 END) AS tokens,
                    MAX(id) AS latest_id
             FROM api_proxy_usage_events
             WHERE timestamp >= ?1 AND timestamp <= ?2 AND TRIM(model) <> ''
             GROUP BY 1, 2, 4",
        )
        .map_err(|error| format!("Failed to prepare API proxy usage stats query: {error}"))?;
    let mut aggregated_events = statement
        .query_map(params![start, now, bucket_seconds], |row| {
            let latest_id = row.get::<_, i64>(6)?;
            let event = ApiProxyUsageEvent {
                timestamp: row.get(0)?,
                key_id: row.get(1)?,
                // SQLite 的单一 MAX(id) 规则会从最近一行取出同组 label，保持重命名语义。
                key_label: row.get(2)?,
                model: row.get(3)?,
                route: None,
                reasoning_effort: None,
                service_tier: None,
                calls: row.get(4)?,
                tokens: row.get(5)?,
                input_tokens: None,
                output_tokens: None,
                cached_input_tokens: None,
            };
            Ok((latest_id, event))
        })
        .map_err(|error| format!("Failed to query API proxy usage stats: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read API proxy usage stats: {error}"))?;
    // 聚合结果远小于原始事件；按最近事件 ID 排序后可沿用现有“最新 Key 名称覆盖旧名称”逻辑。
    aggregated_events.sort_by_key(|(latest_id, _)| *latest_id);
    let events = aggregated_events
        .into_iter()
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    Ok(build_api_proxy_usage_stats(&events, now, range_seconds))
}

struct ApiProxyUsageCsvSummary {
    key_label: String,
    calls: i64,
    tokens: i64,
    input_tokens: i64,
    output_tokens: i64,
    cached_input_tokens: i64,
    saw_token_event: bool,
    input_tokens_complete: bool,
    output_tokens_complete: bool,
    cached_input_tokens_complete: bool,
}

impl Default for ApiProxyUsageCsvSummary {
    fn default() -> Self {
        Self {
            key_label: String::new(),
            calls: 0,
            tokens: 0,
            input_tokens: 0,
            output_tokens: 0,
            cached_input_tokens: 0,
            saw_token_event: false,
            input_tokens_complete: true,
            output_tokens_complete: true,
            cached_input_tokens_complete: true,
        }
    }
}

fn export_api_proxy_usage_csv(
    database_path: &Path,
    export_path: &Path,
    now: i64,
    range_seconds: i64,
    key_id: Option<&str>,
) -> Result<(), String> {
    let mut connection = Connection::open_with_flags(
        database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| {
        format!(
            "Failed to open API proxy usage database {}: {error}",
            database_path.display()
        )
    })?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| format!("Failed to configure API proxy usage export timeout: {error}"))?;
    let start = now.saturating_sub(range_seconds);

    write_private_named_file_atomically_with(export_path, "API proxy usage export", |file| {
        let transaction = connection
            .transaction()
            .map_err(|error| format!("Failed to start API proxy usage export snapshot: {error}"))?;
        let mut writer = BufWriter::new(file);
        writer
            .write_all(b"\xEF\xBB\xBF")
            .map_err(|error| format!("Failed to write API proxy usage CSV BOM: {error}"))?;
        write_api_proxy_usage_csv_record(
            &mut writer,
            &[
                "row_type".to_string(),
                "period_start_utc".to_string(),
                "period_end_utc".to_string(),
                "timestamp_utc".to_string(),
                "timestamp_unix".to_string(),
                "event_type".to_string(),
                "key_id".to_string(),
                "key_label".to_string(),
                "model".to_string(),
                "route".to_string(),
                "reasoning_effort".to_string(),
                "service_tier".to_string(),
                "calls".to_string(),
                "input_tokens".to_string(),
                "output_tokens".to_string(),
                "cached_input_tokens".to_string(),
                "total_tokens".to_string(),
            ],
        )?;

        let mut summaries = BTreeMap::<(Option<String>, String), ApiProxyUsageCsvSummary>::new();
        let mut latest_key_labels = BTreeMap::<Option<String>, String>::new();
        if let Some(key_id) = key_id {
            let mut statement = transaction
                .prepare_cached(
                    "SELECT timestamp, key_id, key_label, model, route,
                            reasoning_effort, service_tier, calls, tokens,
                            input_tokens, output_tokens, cached_input_tokens
                     FROM api_proxy_usage_events
                     WHERE key_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3
                     ORDER BY timestamp ASC, id ASC",
                )
                .map_err(|error| format!("Failed to prepare filtered usage export: {error}"))?;
            let mut rows = statement
                .query(params![key_id, start, now])
                .map_err(|error| format!("Failed to query filtered usage export: {error}"))?;
            write_api_proxy_usage_csv_rows(
                &mut writer,
                &mut rows,
                &mut summaries,
                &mut latest_key_labels,
            )?;
        } else {
            let mut statement = transaction
                .prepare_cached(
                    "SELECT timestamp, key_id, key_label, model, route,
                            reasoning_effort, service_tier, calls, tokens,
                            input_tokens, output_tokens, cached_input_tokens
                     FROM api_proxy_usage_events
                     WHERE timestamp >= ?1 AND timestamp <= ?2
                     ORDER BY timestamp ASC, id ASC",
                )
                .map_err(|error| format!("Failed to prepare usage export: {error}"))?;
            let mut rows = statement
                .query(params![start, now])
                .map_err(|error| format!("Failed to query usage export: {error}"))?;
            write_api_proxy_usage_csv_rows(
                &mut writer,
                &mut rows,
                &mut summaries,
                &mut latest_key_labels,
            )?;
        }

        // 明细先流式落盘，内存中只保留 Key+模型的有限汇总维度。
        for ((key_id, model), summary) in summaries {
            let summary_key_label = latest_key_labels
                .get(&key_id)
                .cloned()
                .unwrap_or(summary.key_label);
            write_api_proxy_usage_csv_record(
                &mut writer,
                &[
                    "summary_range_key_model".to_string(),
                    format_api_proxy_usage_export_timestamp(start),
                    format_api_proxy_usage_export_timestamp(now),
                    String::new(),
                    String::new(),
                    String::new(),
                    key_id.unwrap_or_default(),
                    summary_key_label,
                    model,
                    String::new(),
                    String::new(),
                    String::new(),
                    summary.calls.to_string(),
                    complete_api_proxy_usage_token_component(
                        summary.saw_token_event,
                        summary.input_tokens_complete,
                        summary.input_tokens,
                    ),
                    complete_api_proxy_usage_token_component(
                        summary.saw_token_event,
                        summary.output_tokens_complete,
                        summary.output_tokens,
                    ),
                    complete_api_proxy_usage_token_component(
                        summary.saw_token_event,
                        summary.cached_input_tokens_complete,
                        summary.cached_input_tokens,
                    ),
                    summary.tokens.to_string(),
                ],
            )?;
        }
        writer
            .flush()
            .map_err(|error| format!("Failed to flush API proxy usage CSV: {error}"))?;
        drop(writer);
        transaction
            .commit()
            .map_err(|error| format!("Failed to finish API proxy usage export snapshot: {error}"))
    })
}

fn write_api_proxy_usage_csv_rows(
    writer: &mut impl Write,
    rows: &mut rusqlite::Rows<'_>,
    summaries: &mut BTreeMap<(Option<String>, String), ApiProxyUsageCsvSummary>,
    latest_key_labels: &mut BTreeMap<Option<String>, String>,
) -> Result<(), String> {
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("Failed to read API proxy usage export row: {error}"))?
    {
        let timestamp = row
            .get::<_, i64>(0)
            .map_err(|error| format!("Failed to read usage timestamp: {error}"))?;
        let key_id = row
            .get::<_, Option<String>>(1)
            .map_err(|error| format!("Failed to read usage key ID: {error}"))?;
        let key_label = row
            .get::<_, Option<String>>(2)
            .map_err(|error| format!("Failed to read usage key label: {error}"))?;
        let model = row
            .get::<_, String>(3)
            .map_err(|error| format!("Failed to read usage model: {error}"))?;
        let route = row
            .get::<_, Option<String>>(4)
            .map_err(|error| format!("Failed to read usage route: {error}"))?;
        let reasoning_effort = row
            .get::<_, Option<String>>(5)
            .map_err(|error| format!("Failed to read usage reasoning effort: {error}"))?;
        let service_tier = row
            .get::<_, Option<String>>(6)
            .map_err(|error| format!("Failed to read usage service tier: {error}"))?;
        let calls = row
            .get::<_, i64>(7)
            .map_err(|error| format!("Failed to read usage calls: {error}"))?
            .max(0);
        let tokens = row
            .get::<_, i64>(8)
            .map_err(|error| format!("Failed to read usage tokens: {error}"))?
            .max(0);
        let input_tokens = row
            .get::<_, Option<i64>>(9)
            .map_err(|error| format!("Failed to read usage input tokens: {error}"))?
            .map(|value| value.max(0));
        let output_tokens = row
            .get::<_, Option<i64>>(10)
            .map_err(|error| format!("Failed to read usage output tokens: {error}"))?
            .map(|value| value.max(0));
        let cached_input_tokens = row
            .get::<_, Option<i64>>(11)
            .map_err(|error| format!("Failed to read usage cached input tokens: {error}"))?
            .map(|value| value.max(0));

        write_api_proxy_usage_csv_record(
            writer,
            &[
                "detail".to_string(),
                String::new(),
                String::new(),
                format_api_proxy_usage_export_timestamp(timestamp),
                timestamp.to_string(),
                if calls > 0 && tokens > 0 {
                    "combined"
                } else if calls > 0 {
                    "call"
                } else {
                    "tokens"
                }
                .to_string(),
                key_id.clone().unwrap_or_default(),
                key_label.clone().unwrap_or_default(),
                model.clone(),
                route.unwrap_or_default(),
                reasoning_effort.unwrap_or_default(),
                service_tier.unwrap_or_default(),
                calls.to_string(),
                input_tokens
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                output_tokens
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                cached_input_tokens
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                tokens.to_string(),
            ],
        )?;

        if key_label
            .as_deref()
            .is_some_and(|label| !label.trim().is_empty())
        {
            // 查询按 timestamp/id 升序，覆盖后得到该 Key 在整个导出范围内统一的最新名称。
            latest_key_labels.insert(key_id.clone(), key_label.clone().unwrap_or_default());
        }
        let summary = summaries.entry((key_id, model)).or_default();
        summary.key_label = key_label.unwrap_or_default();
        summary.calls = summary.calls.saturating_add(calls);
        summary.tokens = summary.tokens.saturating_add(tokens);
        if tokens > 0 {
            summary.saw_token_event = true;
            match input_tokens {
                Some(value) => summary.input_tokens = summary.input_tokens.saturating_add(value),
                None => summary.input_tokens_complete = false,
            }
            match output_tokens {
                Some(value) => summary.output_tokens = summary.output_tokens.saturating_add(value),
                None => summary.output_tokens_complete = false,
            }
            match cached_input_tokens {
                Some(value) => {
                    summary.cached_input_tokens = summary.cached_input_tokens.saturating_add(value)
                }
                None => summary.cached_input_tokens_complete = false,
            }
        }
    }
    Ok(())
}

fn complete_api_proxy_usage_token_component(
    saw_token_event: bool,
    complete: bool,
    value: i64,
) -> String {
    // 历史行只有 total；只在范围内每个 token 事件都有分项时输出汇总，避免把部分数据伪装成完整值。
    if saw_token_event && complete {
        value.to_string()
    } else {
        String::new()
    }
}

fn write_api_proxy_usage_csv_record(
    writer: &mut impl Write,
    fields: &[String],
) -> Result<(), String> {
    for (index, value) in fields.iter().enumerate() {
        if index > 0 {
            writer
                .write_all(b",")
                .map_err(|error| format!("Failed to write API proxy usage CSV: {error}"))?;
        }
        let safe_value = if value
            .chars()
            .next()
            .is_some_and(|first| matches!(first, '=' | '+' | '-' | '@' | '\t' | '\r'))
        {
            format!("'{value}")
        } else {
            value.clone()
        };
        let escaped = safe_value.replace('"', "\"\"");
        write!(writer, "\"{escaped}\"")
            .map_err(|error| format!("Failed to write API proxy usage CSV: {error}"))?;
    }
    writer
        .write_all(b"\r\n")
        .map_err(|error| format!("Failed to finish API proxy usage CSV row: {error}"))
}

fn format_api_proxy_usage_export_timestamp(timestamp: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(timestamp)
        .ok()
        .and_then(|value| {
            value
                .format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_default()
}

fn build_api_proxy_usage_stats(
    events: &[ApiProxyUsageEvent],
    now: i64,
    range_seconds: i64,
) -> ApiProxyUsageStats {
    let range_seconds = normalize_api_proxy_usage_range_seconds(Some(range_seconds));
    let bucket_seconds = api_proxy_usage_bucket_seconds(range_seconds);
    let start = now.saturating_sub(range_seconds);
    let first_bucket = floor_timestamp_to_bucket(start, bucket_seconds);
    let last_bucket = floor_timestamp_to_bucket(now, bucket_seconds);
    let bucket_timestamps =
        api_proxy_usage_bucket_timestamps(first_bucket, last_bucket, bucket_seconds);

    let mut by_model = BTreeMap::<String, ApiProxyUsageSeriesAccumulator>::new();
    let mut by_key = BTreeMap::<String, (String, ApiProxyUsageSeriesAccumulator)>::new();
    for event in events {
        if event.timestamp < start || event.timestamp > now {
            continue;
        }
        let model = event.model.trim();
        if model.is_empty() {
            continue;
        }
        let calls = event.calls.max(0);
        let tokens = event.tokens.max(0);
        if calls == 0 && tokens == 0 {
            continue;
        }

        let bucket = floor_timestamp_to_bucket(event.timestamp, bucket_seconds);
        let accumulator = by_model.entry(model.to_string()).or_default();
        accumulator.total_calls += calls;
        accumulator.total_tokens += tokens;
        let point = accumulator.points.entry(bucket).or_insert((0, 0));
        point.0 += calls;
        point.1 += tokens;

        if let Some(key_id) = event
            .key_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let key_label = event
                .key_label
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Unnamed key");
            let (stored_label, key_accumulator) =
                by_key.entry(key_id.to_string()).or_insert_with(|| {
                    (
                        key_label.to_string(),
                        ApiProxyUsageSeriesAccumulator::default(),
                    )
                });
            // Key ID 是稳定身份；名称发生修改时采用最近事件中的名称，避免拆成两条曲线。
            *stored_label = key_label.to_string();
            key_accumulator.total_calls += calls;
            key_accumulator.total_tokens += tokens;
            let key_point = key_accumulator.points.entry(bucket).or_insert((0, 0));
            key_point.0 += calls;
            key_point.1 += tokens;
        }
    }

    let mut series = by_model
        .into_iter()
        .map(|(model, accumulator)| ApiProxyUsageSeries {
            model,
            total_calls: accumulator.total_calls,
            total_tokens: accumulator.total_tokens,
            points: bucket_timestamps
                .iter()
                .map(|timestamp| {
                    let (calls, tokens) = accumulator
                        .points
                        .get(timestamp)
                        .copied()
                        .unwrap_or_default();
                    ApiProxyUsagePoint {
                        timestamp: *timestamp,
                        calls,
                        tokens,
                    }
                })
                .collect(),
        })
        .collect::<Vec<_>>();

    series.sort_by(|left, right| {
        right
            .total_calls
            .cmp(&left.total_calls)
            .then_with(|| right.total_tokens.cmp(&left.total_tokens))
            .then_with(|| left.model.cmp(&right.model))
    });

    let mut key_series = by_key
        .into_iter()
        .map(
            |(key_id, (key_label, accumulator))| ApiProxyUsageKeySeries {
                key_id,
                key_label,
                total_calls: accumulator.total_calls,
                total_tokens: accumulator.total_tokens,
                points: bucket_timestamps
                    .iter()
                    .map(|timestamp| {
                        let (calls, tokens) = accumulator
                            .points
                            .get(timestamp)
                            .copied()
                            .unwrap_or_default();
                        ApiProxyUsagePoint {
                            timestamp: *timestamp,
                            calls,
                            tokens,
                        }
                    })
                    .collect(),
            },
        )
        .collect::<Vec<_>>();
    key_series.sort_by(|left, right| {
        right
            .total_calls
            .cmp(&left.total_calls)
            .then_with(|| right.total_tokens.cmp(&left.total_tokens))
            .then_with(|| left.key_label.cmp(&right.key_label))
            .then_with(|| left.key_id.cmp(&right.key_id))
    });

    ApiProxyUsageStats {
        updated_at: now,
        range_seconds,
        bucket_seconds,
        series,
        key_series,
    }
}

fn normalize_api_proxy_usage_range_seconds(range_seconds: Option<i64>) -> i64 {
    let requested = range_seconds
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_API_PROXY_USAGE_RANGE_SECONDS);

    if requested <= API_PROXY_USAGE_RANGE_1H_SECONDS {
        API_PROXY_USAGE_RANGE_1H_SECONDS
    } else if requested <= API_PROXY_USAGE_RANGE_24H_SECONDS {
        API_PROXY_USAGE_RANGE_24H_SECONDS
    } else if requested <= API_PROXY_USAGE_RANGE_7D_SECONDS {
        API_PROXY_USAGE_RANGE_7D_SECONDS
    } else if requested <= API_PROXY_USAGE_RANGE_14D_SECONDS {
        API_PROXY_USAGE_RANGE_14D_SECONDS
    } else {
        API_PROXY_USAGE_RANGE_30D_SECONDS
    }
}

fn api_proxy_usage_bucket_seconds(range_seconds: i64) -> i64 {
    match range_seconds {
        API_PROXY_USAGE_RANGE_1H_SECONDS => 60,
        API_PROXY_USAGE_RANGE_24H_SECONDS => 30 * 60,
        API_PROXY_USAGE_RANGE_7D_SECONDS => 6 * 60 * 60,
        API_PROXY_USAGE_RANGE_14D_SECONDS => 12 * 60 * 60,
        API_PROXY_USAGE_RANGE_30D_SECONDS => 24 * 60 * 60,
        _ => 30 * 60,
    }
}

fn api_proxy_usage_bucket_timestamps(first: i64, last: i64, bucket_seconds: i64) -> Vec<i64> {
    if bucket_seconds <= 0 || last < first {
        return Vec::new();
    }

    let mut timestamps = Vec::new();
    let mut current = first;
    while current <= last {
        timestamps.push(current);
        current = match current.checked_add(bucket_seconds) {
            Some(value) => value,
            None => break,
        };
    }
    timestamps
}

fn floor_timestamp_to_bucket(timestamp: i64, bucket_seconds: i64) -> i64 {
    if bucket_seconds <= 0 {
        return timestamp;
    }
    timestamp - timestamp.rem_euclid(bucket_seconds)
}

fn api_proxy_usage_tokens_from_response(response: &Value) -> Option<ApiProxyTokenUsageBreakdown> {
    response
        .get("usage")
        .and_then(api_proxy_usage_tokens_from_usage)
}

fn api_proxy_usage_tokens_from_sse_event(event: &SseEvent) -> Option<ApiProxyTokenUsageBreakdown> {
    let parsed = serde_json::from_str::<Value>(&event.data).ok()?;
    let event_type = parsed
        .get("type")
        .and_then(Value::as_str)
        .or(event.event.as_deref())?;
    if !matches!(event_type, "response.completed" | "response.done") {
        return None;
    }

    parsed
        .get("response")
        .and_then(api_proxy_usage_tokens_from_response)
        .or_else(|| {
            parsed
                .get("usage")
                .and_then(api_proxy_usage_tokens_from_usage)
        })
}

fn api_proxy_usage_tokens_from_usage(usage: &Value) -> Option<ApiProxyTokenUsageBreakdown> {
    let input_tokens = token_count_field(usage, "input_tokens")
        .or_else(|| token_count_field(usage, "prompt_tokens"));
    let output_tokens = token_count_field(usage, "output_tokens")
        .or_else(|| token_count_field(usage, "completion_tokens"));
    let total_tokens = token_count_field(usage, "total_tokens")
        .or_else(|| input_tokens?.checked_add(output_tokens?))?;
    let cached_input_tokens = usage
        .get("input_tokens_details")
        .and_then(|details| token_count_field(details, "cached_tokens"))
        .or_else(|| {
            usage
                .get("prompt_tokens_details")
                .and_then(|details| token_count_field(details, "cached_tokens"))
        })
        .or_else(|| token_count_field(usage, "cached_input_tokens"));

    Some(ApiProxyTokenUsageBreakdown {
        total_tokens,
        input_tokens,
        output_tokens,
        cached_input_tokens,
    })
}

fn token_count_field(usage: &Value, key: &str) -> Option<i64> {
    let value = usage.get(key)?;
    if let Some(value) = value.as_i64() {
        return (value >= 0).then_some(value);
    }
    if let Some(value) = value.as_u64() {
        return i64::try_from(value).ok();
    }
    value
        .as_str()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value >= 0)
}

#[cfg(feature = "desktop")]
fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app_paths::app_data_dir(app)
}

fn write_private_file_atomically(path: &Path, contents: &[u8]) -> Result<(), String> {
    write_private_named_file_atomically(path, contents, "API Key")
}

fn write_private_named_file_atomically(
    path: &Path,
    contents: &[u8],
    purpose: &str,
) -> Result<(), String> {
    write_private_named_file_atomically_with(path, purpose, |temp_file| {
        temp_file
            .write_all(contents)
            .map_err(|error| format!("写入 {purpose} 临时文件失败 {}: {error}", path.display()))
    })
}

fn write_private_named_file_atomically_with(
    path: &Path,
    purpose: &str,
    write_contents: impl FnOnce(&mut fs::File) -> Result<(), String>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析 {purpose} 存储目录 {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("创建 {purpose} 存储目录失败 {}: {error}", parent.display()))?;

    let temp_path = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("api-proxy.key"),
        uuid::Uuid::new_v4()
    ));

    let write_result = (|| -> Result<(), String> {
        let mut temp_file = private_create_new_options()
            .open(&temp_path)
            .map_err(|error| {
                format!(
                    "创建 {purpose} 临时文件失败 {}: {error}",
                    temp_path.display()
                )
            })?;
        write_contents(&mut temp_file)?;
        temp_file.sync_all().map_err(|error| {
            format!(
                "刷新 {purpose} 临时文件失败 {}: {error}",
                temp_path.display()
            )
        })?;
        drop(temp_file);
        set_private_permissions(&temp_path);

        #[cfg(target_family = "unix")]
        {
            fs::rename(&temp_path, path).map_err(|error| {
                format!(
                    "替换 {purpose} 存储文件失败 {} -> {}: {error}",
                    temp_path.display(),
                    path.display()
                )
            })?;

            let parent_dir = fs::File::open(parent).map_err(|error| {
                format!("打开 {purpose} 存储目录失败 {}: {error}", parent.display())
            })?;
            parent_dir.sync_all().map_err(|error| {
                format!("刷新 {purpose} 存储目录失败 {}: {error}", parent.display())
            })?;
        }

        #[cfg(not(target_family = "unix"))]
        {
            if path.exists() {
                fs::remove_file(path).map_err(|error| {
                    format!("移除旧 {purpose} 存储文件失败 {}: {error}", path.display())
                })?;
            }
            fs::rename(&temp_path, path).map_err(|error| {
                format!(
                    "替换 {purpose} 存储文件失败 {} -> {}: {error}",
                    temp_path.display(),
                    path.display()
                )
            })?;
        }

        set_private_permissions(path);
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}

fn read_current_api_key(shared: &Arc<RwLock<Vec<ApiProxyKey>>>) -> Option<String> {
    shared
        .read()
        .ok()
        .and_then(|keys| primary_api_proxy_key_value(&keys))
}

fn should_forward_response_header(name: &str) -> bool {
    !matches!(
        name.to_ascii_lowercase().as_str(),
        "content-length" | "connection" | "transfer-encoding" | "content-type"
    )
}

fn build_proxy_response(
    status: StatusCode,
    upstream_headers: &HeaderMap,
    body: Bytes,
) -> Response<Body> {
    let mut response = Response::builder().status(status);
    for (name, value) in upstream_headers {
        if should_forward_response_header(name.as_str()) {
            response = response.header(name, value);
        }
    }

    response
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| json_error_response(StatusCode::BAD_GATEWAY, "构建代理响应失败"))
}

fn build_json_proxy_response(
    status: StatusCode,
    upstream_headers: &HeaderMap,
    body: Bytes,
) -> Response<Body> {
    build_proxy_response(status, upstream_headers, body)
}

fn build_sse_bytes_response(
    status: StatusCode,
    upstream_headers: &HeaderMap,
    body: Bytes,
) -> Response<Body> {
    let mut response = Response::builder().status(status);
    for (name, value) in upstream_headers {
        if should_forward_response_header(name.as_str()) {
            response = response.header(name, value);
        }
    }
    response
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .header("connection", "keep-alive")
        .body(Body::from(body))
        .unwrap_or_else(|_| json_error_response(StatusCode::BAD_GATEWAY, "构建 SSE 响应失败"))
}

fn build_passthrough_sse_response(
    upstream: CodexUpstreamResponse,
    usage_storage: ProxyStorageContext,
    usage_metadata: ApiProxyUsageMetadata,
) -> Response<Body> {
    let (upstream_headers, mut upstream_stream) = upstream.into_stream();
    let output = stream! {
        let mut decoder = SseDecoder::default();
        let mut recorded_usage = false;

        while let Some(chunk) = upstream_stream.next().await {
            match chunk {
                Ok(chunk) => {
                    for event in decoder.push(&chunk) {
                        maybe_record_stream_usage_tokens(
                            &usage_storage,
                            &usage_metadata,
                            &event,
                            &mut recorded_usage,
                        )
                        .await;
                        yield Ok::<Bytes, Infallible>(serialize_sse_event(
                            event.event.as_deref(),
                            &rewrite_sse_event_data_models_for_client(&event.data),
                        ));
                    }
                }
                Err(_) => return,
            }
        }

        for event in decoder.finish() {
            maybe_record_stream_usage_tokens(
                &usage_storage,
                &usage_metadata,
                &event,
                &mut recorded_usage,
            )
            .await;
            yield Ok::<Bytes, Infallible>(serialize_sse_event(
                event.event.as_deref(),
                &rewrite_sse_event_data_models_for_client(&event.data),
            ));
        }
    };
    let mut response = Response::builder().status(StatusCode::OK);
    for (name, value) in &upstream_headers {
        if should_forward_response_header(name.as_str()) {
            response = response.header(name, value);
        }
    }

    response
        .header("content-type", "text/event-stream; charset=utf-8")
        .header("cache-control", "no-cache")
        .body(Body::from_stream(output))
        .unwrap_or_else(|_| json_error_response(StatusCode::BAD_GATEWAY, "构建流式代理响应失败"))
}

fn build_chat_streaming_response(
    upstream: CodexUpstreamResponse,
    usage_storage: ProxyStorageContext,
    usage_metadata: ApiProxyUsageMetadata,
) -> Response<Body> {
    let (upstream_headers, mut upstream_stream) = upstream.into_stream();
    let output = stream! {
        let mut decoder = SseDecoder::default();
        let mut state = ChatStreamState::default();
        let mut recorded_usage = false;

        while let Some(chunk) = upstream_stream.next().await {
            match chunk {
                Ok(chunk) => {
                    for event in decoder.push(&chunk) {
                        maybe_record_stream_usage_tokens(
                            &usage_storage,
                            &usage_metadata,
                            &event,
                            &mut recorded_usage,
                        )
                        .await;
                        for value in translate_sse_event_to_chat_chunk(&event, &mut state) {
                            yield Ok::<Bytes, Infallible>(sse_data_chunk(&value));
                        }
                    }
                }
                Err(error) => {
                    yield Ok::<Bytes, Infallible>(sse_data_chunk(&json!({
                        "error": {
                            "message": format!("上游流式响应中断: {error}")
                        }
                    })));
                    yield Ok::<Bytes, Infallible>(Bytes::from_static(SSE_DONE.as_bytes()));
                    return;
                }
            }
        }

        for event in decoder.finish() {
            maybe_record_stream_usage_tokens(
                &usage_storage,
                &usage_metadata,
                &event,
                &mut recorded_usage,
            )
            .await;
            for value in translate_sse_event_to_chat_chunk(&event, &mut state) {
                yield Ok::<Bytes, Infallible>(sse_data_chunk(&value));
            }
        }

        yield Ok::<Bytes, Infallible>(Bytes::from_static(SSE_DONE.as_bytes()));
    };

    let mut response = Response::builder().status(StatusCode::OK);
    for (name, value) in &upstream_headers {
        if should_forward_response_header(name.as_str()) {
            response = response.header(name, value);
        }
    }

    response
        .header("content-type", "text/event-stream; charset=utf-8")
        .header("cache-control", "no-cache")
        .body(Body::from_stream(output))
        .unwrap_or_else(|_| json_error_response(StatusCode::BAD_GATEWAY, "构建聊天流式响应失败"))
}

fn build_anthropic_streaming_response(
    upstream: CodexUpstreamResponse,
    usage_storage: ProxyStorageContext,
    usage_metadata: ApiProxyUsageMetadata,
) -> Response<Body> {
    let (upstream_headers, mut upstream_stream) = upstream.into_stream();
    let output = stream! {
        let mut decoder = SseDecoder::default();
        let mut state = AnthropicStreamState::default();
        let mut recorded_usage = false;

        while let Some(chunk) = upstream_stream.next().await {
            match chunk {
                Ok(chunk) => {
                    for event in decoder.push(&chunk) {
                        maybe_record_stream_usage_tokens(
                            &usage_storage,
                            &usage_metadata,
                            &event,
                            &mut recorded_usage,
                        )
                        .await;
                        for value in translate_sse_event_to_anthropic_event(&event, &mut state) {
                            yield Ok::<Bytes, Infallible>(anthropic_sse_chunk(&value));
                        }
                    }
                }
                Err(error) => {
                    yield Ok::<Bytes, Infallible>(anthropic_sse_chunk(&json!({
                        "type": "error",
                        "error": {
                            "type": "api_error",
                            "message": format!("上游 Anthropic 流式响应中断: {error}")
                        }
                    })));
                    return;
                }
            }
        }

        for event in decoder.finish() {
            maybe_record_stream_usage_tokens(
                &usage_storage,
                &usage_metadata,
                &event,
                &mut recorded_usage,
            )
            .await;
            for value in translate_sse_event_to_anthropic_event(&event, &mut state) {
                yield Ok::<Bytes, Infallible>(anthropic_sse_chunk(&value));
            }
        }
    };

    let mut response = Response::builder().status(StatusCode::OK);
    for (name, value) in &upstream_headers {
        if should_forward_response_header(name.as_str()) {
            response = response.header(name, value);
        }
    }

    response
        .header("content-type", "text/event-stream; charset=utf-8")
        .header("cache-control", "no-cache")
        .body(Body::from_stream(output))
        .unwrap_or_else(|_| {
            json_error_response(StatusCode::BAD_GATEWAY, "构建 Anthropic 流式响应失败")
        })
}

fn build_image_streaming_response(
    upstream: CodexUpstreamResponse,
    usage_storage: ProxyStorageContext,
    usage_metadata: ApiProxyUsageMetadata,
) -> Response<Body> {
    let (upstream_headers, mut upstream_stream) = upstream.into_stream();
    let output = stream! {
        let mut decoder = SseDecoder::default();
        let mut emitted_final_image = false;
        let mut recorded_usage = false;

        while let Some(chunk) = upstream_stream.next().await {
            match chunk {
                Ok(chunk) => {
                    for event in decoder.push(&chunk) {
                        maybe_record_stream_usage_tokens(
                            &usage_storage,
                            &usage_metadata,
                            &event,
                            &mut recorded_usage,
                        )
                        .await;
                        for value in translate_sse_event_to_image_chunk(&event, &mut emitted_final_image) {
                            yield Ok::<Bytes, Infallible>(sse_data_chunk(&value));
                        }
                    }
                }
                Err(error) => {
                    yield Ok::<Bytes, Infallible>(sse_data_chunk(&json!({
                        "error": {
                            "message": format!("上游图片流式响应中断: {error}")
                        }
                    })));
                    yield Ok::<Bytes, Infallible>(Bytes::from_static(SSE_DONE.as_bytes()));
                    return;
                }
            }
        }

        for event in decoder.finish() {
            maybe_record_stream_usage_tokens(
                &usage_storage,
                &usage_metadata,
                &event,
                &mut recorded_usage,
            )
            .await;
            for value in translate_sse_event_to_image_chunk(&event, &mut emitted_final_image) {
                yield Ok::<Bytes, Infallible>(sse_data_chunk(&value));
            }
        }

        yield Ok::<Bytes, Infallible>(Bytes::from_static(SSE_DONE.as_bytes()));
    };

    let mut response = Response::builder().status(StatusCode::OK);
    for (name, value) in &upstream_headers {
        if should_forward_response_header(name.as_str()) {
            response = response.header(name, value);
        }
    }

    response
        .header("content-type", "text/event-stream; charset=utf-8")
        .header("cache-control", "no-cache")
        .body(Body::from_stream(output))
        .unwrap_or_else(|_| json_error_response(StatusCode::BAD_GATEWAY, "构建图片流式响应失败"))
}

fn translate_sse_event_to_image_chunk(
    event: &SseEvent,
    emitted_final_image: &mut bool,
) -> Vec<Value> {
    let Ok(value) = serde_json::from_str::<Value>(&event.data) else {
        return Vec::new();
    };
    match value.get("type").and_then(Value::as_str) {
        Some("response.image_generation_call.partial_image") => {
            let Some(partial) = value.get("partial_image_b64").and_then(Value::as_str) else {
                return Vec::new();
            };
            vec![json!({
                "type": "image_generation.partial_image",
                "b64_json": partial,
            })]
        }
        Some("response.output_item.done") => {
            image_completed_chunk(value.get("item"), emitted_final_image)
        }
        Some("response.completed") => {
            image_completed_chunk(value.get("response"), emitted_final_image)
        }
        Some("response.failed") => vec![json!({
            "error": {
                "message": response_error_message_from_value(&value)
                    .unwrap_or_else(|| "图片生成失败".to_string())
            }
        })],
        _ => Vec::new(),
    }
}

fn image_completed_chunk(value: Option<&Value>, emitted_final_image: &mut bool) -> Vec<Value> {
    if *emitted_final_image {
        return Vec::new();
    }
    let Some(image) =
        value.and_then(|value| convert_responses_image_output_to_images_response(value).ok())
    else {
        return Vec::new();
    };
    *emitted_final_image = true;
    vec![json!({
        "type": "image_generation.completed",
        "image": image,
    })]
}

fn sse_data_chunk(value: &Value) -> Bytes {
    let serialized = serde_json::to_string(value).unwrap_or_else(|_| {
        "{\"error\":{\"message\":\"stream serialization failed\"}}".to_string()
    });
    Bytes::from(format!("data: {serialized}\n\n"))
}

fn anthropic_sse_chunk(value: &Value) -> Bytes {
    let event_type = value.get("type").and_then(Value::as_str);
    let serialized = serde_json::to_string(value).unwrap_or_else(|_| {
        "{\"type\":\"error\",\"error\":{\"type\":\"api_error\",\"message\":\"stream serialization failed\"}}".to_string()
    });
    serialize_sse_event(event_type, &serialized)
}

fn rewrite_sse_event_data_models_for_client(data: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(data) else {
        return data.to_string();
    };
    remap_model_fields_to_client(&mut value);
    serde_json::to_string(&value).unwrap_or_else(|_| data.to_string())
}

fn serialize_sse_event(event: Option<&str>, data: &str) -> Bytes {
    let mut serialized = String::new();
    if let Some(event) = event.filter(|value| !value.is_empty()) {
        serialized.push_str("event: ");
        serialized.push_str(event);
        serialized.push('\n');
    }
    if data.is_empty() {
        serialized.push_str("data:\n");
    } else {
        for line in data.lines() {
            serialized.push_str("data: ");
            serialized.push_str(line);
            serialized.push('\n');
        }
    }
    serialized.push('\n');
    Bytes::from(serialized)
}

fn extract_completed_response_from_sse(bytes: &[u8]) -> Result<Value, String> {
    if let Ok(value) = serde_json::from_slice::<Value>(bytes) {
        if value
            .get("type")
            .and_then(Value::as_str)
            .map(|value| value == "response.completed")
            .unwrap_or(false)
        {
            return value
                .get("response")
                .cloned()
                .map(|response| ensure_completed_response_output(response, ""))
                .ok_or_else(|| "Codex 响应缺少 response 字段".to_string());
        }
        if let Some(message) = response_error_message_from_value(&value) {
            return Err(message);
        }
    }

    let mut decoder = SseDecoder::default();
    let mut first_error = None::<String>;
    let mut output_text = String::new();
    let mut output_items = Vec::new();
    for event in decoder.push(bytes) {
        if let Ok(parsed) = serde_json::from_str::<Value>(&event.data) {
            collect_output_text_delta(&parsed, &mut output_text);
            collect_completed_output_item(&parsed, &mut output_items);
            if let Some(response) = response_completed_from_value(&parsed) {
                let response = ensure_completed_response_output_items(response, &output_items);
                return Ok(ensure_completed_response_output(response, &output_text));
            }
            if first_error.is_none() {
                first_error = response_error_message_from_value(&parsed);
            }
        }
    }
    for event in decoder.finish() {
        if let Ok(parsed) = serde_json::from_str::<Value>(&event.data) {
            collect_output_text_delta(&parsed, &mut output_text);
            collect_completed_output_item(&parsed, &mut output_items);
            if let Some(response) = response_completed_from_value(&parsed) {
                let response = ensure_completed_response_output_items(response, &output_items);
                return Ok(ensure_completed_response_output(response, &output_text));
            }
            if first_error.is_none() {
                first_error = response_error_message_from_value(&parsed);
            }
        }
    }

    if let Some(message) = first_error {
        return Err(message);
    }

    Err("未在 Codex SSE 中找到 response.completed 事件".to_string())
}

fn response_completed_from_value(parsed: &Value) -> Option<Value> {
    if parsed.get("type").and_then(Value::as_str) != Some("response.completed") {
        return None;
    }
    parsed.get("response").cloned()
}

fn collect_output_text_delta(value: &Value, output_text: &mut String) {
    if value.get("type").and_then(Value::as_str) == Some("response.output_text.delta") {
        if let Some(delta) = value.get("delta").and_then(Value::as_str) {
            output_text.push_str(delta);
        }
    }
}

fn collect_completed_output_item(value: &Value, output_items: &mut Vec<Value>) {
    if value.get("type").and_then(Value::as_str) != Some("response.output_item.done") {
        return;
    }
    let Some(item) = value.get("item").cloned() else {
        return;
    };
    output_items.push(item);
}

fn ensure_completed_response_output(mut response: Value, output_text: &str) -> Value {
    if output_text.is_empty() || response_has_text_output(&response) {
        return response;
    }

    if let Some(object) = response.as_object_mut() {
        object.insert(
            "output".to_string(),
            Value::Array(vec![json!({
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": output_text,
                    "annotations": []
                }]
            })]),
        );
    }
    response
}

fn ensure_completed_response_output_items(mut response: Value, output_items: &[Value]) -> Value {
    if output_items.is_empty() || !response_output_is_empty(&response) {
        return response;
    }

    if let Some(object) = response.as_object_mut() {
        object.insert("output".to_string(), Value::Array(output_items.to_vec()));
    }
    response
}

fn response_has_text_output(response: &Value) -> bool {
    response
        .get("output")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter().any(|item| {
                item.get("content")
                    .and_then(Value::as_array)
                    .map(|content| {
                        content.iter().any(|part| {
                            part.get("type").and_then(Value::as_str) == Some("output_text")
                                && part
                                    .get("text")
                                    .and_then(Value::as_str)
                                    .map(|text| !text.is_empty())
                                    .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn response_output_is_empty(response: &Value) -> bool {
    response
        .get("output")
        .and_then(Value::as_array)
        .map(|items| items.is_empty())
        .unwrap_or(true)
}

fn response_error_message_from_value(value: &Value) -> Option<String> {
    match value.get("type").and_then(Value::as_str) {
        Some("error") => value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        Some("response.failed") => value
            .get("response")
            .and_then(|response| response.get("error"))
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        _ => None,
    }
}

fn convert_completed_response_to_chat_completion(response: &Value) -> Value {
    let response_object = response.as_object().cloned().unwrap_or_default();
    let mut message = Map::new();
    message.insert("role".to_string(), Value::String("assistant".to_string()));

    let mut reasoning_content = None::<String>;
    let mut text_content = None::<String>;
    let mut tool_calls = Vec::new();

    if let Some(output) = response_object.get("output").and_then(Value::as_array) {
        for item in output {
            let Some(item_object) = item.as_object() else {
                continue;
            };
            match item_object
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
            {
                "reasoning" => {
                    if let Some(summary) = item_object.get("summary").and_then(Value::as_array) {
                        for summary_item in summary {
                            if summary_item.get("type").and_then(Value::as_str)
                                == Some("summary_text")
                            {
                                if let Some(text) = summary_item.get("text").and_then(Value::as_str)
                                {
                                    if !text.trim().is_empty() {
                                        reasoning_content = Some(text.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                "message" => {
                    if let Some(content) = item_object.get("content").and_then(Value::as_array) {
                        let mut collected = Vec::new();
                        for content_item in content {
                            let Some(content_object) = content_item.as_object() else {
                                continue;
                            };
                            if content_object.get("type").and_then(Value::as_str)
                                == Some("output_text")
                            {
                                if let Some(text) =
                                    content_object.get("text").and_then(Value::as_str)
                                {
                                    if !text.is_empty() {
                                        collected.push(text.to_string());
                                    }
                                }
                            }
                        }
                        if !collected.is_empty() {
                            text_content = Some(collected.join(""));
                        }
                    }
                }
                "function_call" => {
                    tool_calls.push(json!({
                        "id": item_object.get("call_id").and_then(Value::as_str).unwrap_or_default(),
                        "type": "function",
                        "function": {
                            "name": item_object.get("name").and_then(Value::as_str).unwrap_or_default(),
                            "arguments": item_object.get("arguments").and_then(Value::as_str).unwrap_or_default(),
                        }
                    }));
                }
                _ => {}
            }
        }
    }

    message.insert(
        "content".to_string(),
        text_content.map(Value::String).unwrap_or(Value::Null),
    );
    if let Some(reasoning) = reasoning_content {
        message.insert("reasoning_content".to_string(), Value::String(reasoning));
    }
    if !tool_calls.is_empty() {
        message.insert("tool_calls".to_string(), Value::Array(tool_calls.clone()));
    }

    let finish_reason = if tool_calls.is_empty() {
        "stop"
    } else {
        "tool_calls"
    };
    let mut root = Map::new();
    root.insert(
        "id".to_string(),
        response_object
            .get("id")
            .cloned()
            .unwrap_or(Value::String(String::new())),
    );
    root.insert(
        "object".to_string(),
        Value::String("chat.completion".to_string()),
    );
    root.insert(
        "created".to_string(),
        response_object
            .get("created_at")
            .cloned()
            .unwrap_or(Value::Number(serde_json::Number::from(0))),
    );
    root.insert(
        "model".to_string(),
        response_object
            .get("model")
            .and_then(Value::as_str)
            .map(|model| Value::String(normalize_model_for_client(model)))
            .unwrap_or(Value::String(String::new())),
    );
    root.insert(
        "choices".to_string(),
        Value::Array(vec![json!({
            "index": 0,
            "message": Value::Object(message),
            "finish_reason": finish_reason,
            "native_finish_reason": finish_reason,
        })]),
    );

    if let Some(usage) = response_object.get("usage") {
        root.insert("usage".to_string(), build_openai_usage(usage));
    }

    Value::Object(root)
}

fn build_openai_usage(usage: &Value) -> Value {
    let mut root = Map::new();
    if let Some(input_tokens) = usage.get("input_tokens") {
        root.insert("prompt_tokens".to_string(), input_tokens.clone());
    }
    if let Some(output_tokens) = usage.get("output_tokens") {
        root.insert("completion_tokens".to_string(), output_tokens.clone());
    }
    if let Some(total_tokens) = usage.get("total_tokens") {
        root.insert("total_tokens".to_string(), total_tokens.clone());
    }
    if let Some(cached_tokens) = usage
        .get("input_tokens_details")
        .and_then(|value| value.get("cached_tokens"))
    {
        root.insert(
            "prompt_tokens_details".to_string(),
            json!({ "cached_tokens": cached_tokens }),
        );
    }
    if let Some(reasoning_tokens) = usage
        .get("output_tokens_details")
        .and_then(|value| value.get("reasoning_tokens"))
    {
        root.insert(
            "completion_tokens_details".to_string(),
            json!({ "reasoning_tokens": reasoning_tokens }),
        );
    }
    Value::Object(root)
}

fn convert_completed_response_to_anthropic_message(response: &Value) -> Value {
    let response_object = response.as_object().cloned().unwrap_or_default();
    let mut content = Vec::new();
    let mut saw_tool_use = false;

    if let Some(output) = response_object.get("output").and_then(Value::as_array) {
        for item in output {
            let Some(item_object) = item.as_object() else {
                continue;
            };
            match item_object
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
            {
                "message" => {
                    if let Some(parts) = item_object.get("content").and_then(Value::as_array) {
                        for part in parts {
                            let Some(part_object) = part.as_object() else {
                                continue;
                            };
                            if part_object.get("type").and_then(Value::as_str)
                                != Some("output_text")
                            {
                                continue;
                            }
                            if let Some(text) = part_object.get("text").and_then(Value::as_str) {
                                if !text.is_empty() {
                                    content.push(json!({
                                        "type": "text",
                                        "text": text,
                                    }));
                                }
                            }
                        }
                    }
                }
                "function_call" => {
                    saw_tool_use = true;
                    content.push(json!({
                        "type": "tool_use",
                        "id": item_object.get("call_id").and_then(Value::as_str).unwrap_or_default(),
                        "name": item_object.get("name").and_then(Value::as_str).unwrap_or_default(),
                        "input": parse_tool_arguments_json(
                            item_object
                                .get("arguments")
                                .and_then(Value::as_str)
                                .unwrap_or_default(),
                        ),
                    }));
                }
                _ => {}
            }
        }
    }

    json!({
        "id": response_object
            .get("id")
            .cloned()
            .unwrap_or(Value::String(String::new())),
        "type": "message",
        "role": "assistant",
        "model": response_object
            .get("model")
            .and_then(Value::as_str)
            .map(normalize_model_for_client)
            .unwrap_or_default(),
        "content": content,
        "stop_reason": anthropic_stop_reason_from_response(response, saw_tool_use),
        "stop_sequence": Value::Null,
        "usage": build_anthropic_usage(response_object.get("usage")),
    })
}

fn parse_tool_arguments_json(arguments: &str) -> Value {
    if arguments.trim().is_empty() {
        return json!({});
    }
    serde_json::from_str::<Value>(arguments).unwrap_or_else(|_| json!({}))
}

fn anthropic_stop_reason_from_response(response: &Value, saw_tool_use: bool) -> &'static str {
    if saw_tool_use {
        return "tool_use";
    }
    let is_max_tokens = response
        .get("incomplete_details")
        .and_then(|value| value.get("reason"))
        .and_then(Value::as_str)
        == Some("max_output_tokens");
    if is_max_tokens || response.get("status").and_then(Value::as_str) == Some("incomplete") {
        "max_tokens"
    } else {
        "end_turn"
    }
}

fn build_anthropic_usage(usage: Option<&Value>) -> Value {
    let mut root = Map::new();
    let input_tokens = usage
        .and_then(|value| value.get("input_tokens"))
        .and_then(integer_field_value)
        .unwrap_or(0);
    let output_tokens = usage
        .and_then(|value| value.get("output_tokens"))
        .and_then(integer_field_value)
        .unwrap_or(0);
    root.insert(
        "input_tokens".to_string(),
        Value::Number(serde_json::Number::from(input_tokens)),
    );
    root.insert(
        "output_tokens".to_string(),
        Value::Number(serde_json::Number::from(output_tokens)),
    );

    if let Some(cached_tokens) = usage
        .and_then(|value| value.get("input_tokens_details"))
        .and_then(|value| value.get("cached_tokens"))
        .and_then(integer_field_value)
    {
        root.insert(
            "cache_read_input_tokens".to_string(),
            Value::Number(serde_json::Number::from(cached_tokens)),
        );
    }
    Value::Object(root)
}

fn translate_sse_event_to_anthropic_event(
    event: &SseEvent,
    state: &mut AnthropicStreamState,
) -> Vec<Value> {
    let Ok(parsed) = serde_json::from_str::<Value>(&event.data) else {
        return Vec::new();
    };
    let Some(kind) = parsed.get("type").and_then(Value::as_str) else {
        return Vec::new();
    };

    match kind {
        "response.created" => {
            let response = parsed.get("response");
            state.response_id = response
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            state.model = response
                .and_then(|value| value.get("model"))
                .and_then(Value::as_str)
                .map(normalize_model_for_client)
                .unwrap_or_default();
            vec![json!({
                "type": "message_start",
                "message": {
                    "id": state.response_id,
                    "type": "message",
                    "role": "assistant",
                    "model": state.model,
                    "content": [],
                    "stop_reason": Value::Null,
                    "stop_sequence": Value::Null,
                    "usage": build_anthropic_usage(response.and_then(|value| value.get("usage"))),
                }
            })]
        }
        "response.output_text.delta" => {
            let Some(delta) = parsed.get("delta").and_then(Value::as_str) else {
                return Vec::new();
            };
            let mut events = ensure_anthropic_text_block(state);
            if let Some(index) = state.open_text_index {
                events.push(json!({
                    "type": "content_block_delta",
                    "index": index,
                    "delta": {
                        "type": "text_delta",
                        "text": delta,
                    }
                }));
            }
            events
        }
        "response.output_text.done" => stop_anthropic_text_block(state),
        "response.output_item.added" => {
            let Some(item) = parsed.get("item").and_then(Value::as_object) else {
                return Vec::new();
            };
            if item.get("type").and_then(Value::as_str) != Some("function_call") {
                return Vec::new();
            }
            let mut events = stop_anthropic_text_block(state);
            if let Some(call_id) = item
                .get("call_id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                state.announced_tool_call_ids.insert(call_id.to_string());
            }
            events.extend(start_anthropic_tool_block(state, item, None));
            events
        }
        "response.function_call_arguments.delta" => {
            state.has_received_tool_arguments_delta = true;
            state
                .open_tool_index
                .map(|index| {
                    vec![json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": parsed.get("delta").and_then(Value::as_str).unwrap_or_default(),
                        }
                    })]
                })
                .unwrap_or_default()
        }
        "response.function_call_arguments.done" => {
            let mut events = Vec::new();
            if !state.has_received_tool_arguments_delta {
                if let Some(index) = state.open_tool_index {
                    events.push(json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": parsed.get("arguments").and_then(Value::as_str).unwrap_or_default(),
                        }
                    }));
                }
            }
            events.extend(stop_anthropic_tool_block(state));
            events
        }
        "response.output_item.done" => {
            let Some(item) = parsed.get("item").and_then(Value::as_object) else {
                return Vec::new();
            };
            if item.get("type").and_then(Value::as_str) != Some("function_call") {
                return Vec::new();
            }
            let call_id = item
                .get("call_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if state.open_tool_index.is_some() {
                state.announced_tool_call_ids.remove(call_id);
                return stop_anthropic_tool_block(state);
            }
            if state.announced_tool_call_ids.remove(call_id) {
                // The tool block was already streamed and closed by
                // response.function_call_arguments.done, which arrives
                // before response.output_item.done. Re-emitting it here
                // would send a second tool_use block with the same id and
                // Anthropic clients reject the message ("Invalid tool
                // parameters" in Claude Code).
                return Vec::new();
            }
            let mut events = stop_anthropic_text_block(state);
            let arguments = item
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or_default();
            events.extend(start_anthropic_tool_block(
                state,
                item,
                Some(parse_tool_arguments_json(arguments)),
            ));
            events.extend(stop_anthropic_tool_block(state));
            events
        }
        "response.completed" => {
            let response = parsed.get("response");
            let mut events = stop_anthropic_text_block(state);
            events.extend(stop_anthropic_tool_block(state));
            events.push(json!({
                "type": "message_delta",
                "delta": {
                    "stop_reason": anthropic_stop_reason_from_response(
                        response.unwrap_or(&Value::Null),
                        state.saw_tool_use,
                    ),
                    "stop_sequence": Value::Null,
                },
                "usage": build_anthropic_usage(response.and_then(|value| value.get("usage"))),
            }));
            events.push(json!({
                "type": "message_stop",
            }));
            events
        }
        "response.failed" => vec![json!({
            "type": "error",
            "error": {
                "type": "api_error",
                "message": response_error_message_from_value(&parsed)
                    .unwrap_or_else(|| "Anthropic Messages 请求失败".to_string()),
            }
        })],
        _ => {
            let _ = &event.event;
            Vec::new()
        }
    }
}

fn ensure_anthropic_text_block(state: &mut AnthropicStreamState) -> Vec<Value> {
    if state.open_text_index.is_some() {
        return Vec::new();
    }
    let mut events = stop_anthropic_tool_block(state);
    state.content_index += 1;
    let index = state.content_index;
    state.open_text_index = Some(index);
    events.push(json!({
        "type": "content_block_start",
        "index": index,
        "content_block": {
            "type": "text",
            "text": "",
        }
    }));
    events
}

fn stop_anthropic_text_block(state: &mut AnthropicStreamState) -> Vec<Value> {
    state
        .open_text_index
        .take()
        .map(|index| {
            vec![json!({
                "type": "content_block_stop",
                "index": index,
            })]
        })
        .unwrap_or_default()
}

fn start_anthropic_tool_block(
    state: &mut AnthropicStreamState,
    item: &Map<String, Value>,
    complete_input: Option<Value>,
) -> Vec<Value> {
    let mut events = stop_anthropic_tool_block(state);
    state.content_index += 1;
    let index = state.content_index;
    state.open_tool_index = Some(index);
    state.saw_tool_use = true;
    state.has_received_tool_arguments_delta = false;
    events.push(json!({
        "type": "content_block_start",
        "index": index,
        "content_block": {
            "type": "tool_use",
            "id": item.get("call_id").and_then(Value::as_str).unwrap_or_default(),
            "name": item.get("name").and_then(Value::as_str).unwrap_or_default(),
            "input": complete_input.unwrap_or_else(|| json!({})),
        }
    }));
    events
}

fn stop_anthropic_tool_block(state: &mut AnthropicStreamState) -> Vec<Value> {
    state
        .open_tool_index
        .take()
        .map(|index| {
            vec![json!({
                "type": "content_block_stop",
                "index": index,
            })]
        })
        .unwrap_or_default()
}

fn translate_sse_event_to_chat_chunk(event: &SseEvent, state: &mut ChatStreamState) -> Vec<Value> {
    let Ok(parsed) = serde_json::from_str::<Value>(&event.data) else {
        return Vec::new();
    };
    let Some(kind) = parsed.get("type").and_then(Value::as_str) else {
        return Vec::new();
    };

    match kind {
        "response.created" => {
            state.response_id = parsed
                .get("response")
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            state.created_at = parsed
                .get("response")
                .and_then(|value| value.get("created_at"))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            state.model = parsed
                .get("response")
                .and_then(|value| value.get("model"))
                .and_then(Value::as_str)
                .map(normalize_model_for_client)
                .unwrap_or_default();
            Vec::new()
        }
        "response.reasoning_summary_text.delta" => parsed
            .get("delta")
            .and_then(Value::as_str)
            .map(|delta| {
                vec![build_chat_chunk(
                    state,
                    json!({
                        "role": "assistant",
                        "reasoning_content": delta,
                    }),
                    None,
                    parsed.get("response").and_then(|value| value.get("usage")),
                )]
            })
            .unwrap_or_default(),
        "response.reasoning_summary_text.done" => vec![build_chat_chunk(
            state,
            json!({
                "role": "assistant",
                "reasoning_content": "\n\n",
            }),
            None,
            parsed.get("response").and_then(|value| value.get("usage")),
        )],
        "response.output_text.delta" => parsed
            .get("delta")
            .and_then(Value::as_str)
            .map(|delta| {
                vec![build_chat_chunk(
                    state,
                    json!({
                        "role": "assistant",
                        "content": delta,
                    }),
                    None,
                    parsed.get("response").and_then(|value| value.get("usage")),
                )]
            })
            .unwrap_or_default(),
        "response.output_item.added" => {
            let Some(item) = parsed.get("item").and_then(Value::as_object) else {
                return Vec::new();
            };
            if item.get("type").and_then(Value::as_str) != Some("function_call") {
                return Vec::new();
            }

            state.function_call_index += 1;
            state.has_received_arguments_delta = false;
            state.has_tool_call_announced = true;

            vec![build_chat_chunk(
                state,
                json!({
                    "role": "assistant",
                    "tool_calls": [{
                        "index": state.function_call_index,
                        "id": item.get("call_id").and_then(Value::as_str).unwrap_or_default(),
                        "type": "function",
                        "function": {
                            "name": item.get("name").and_then(Value::as_str).unwrap_or_default(),
                            "arguments": "",
                        }
                    }]
                }),
                None,
                parsed.get("response").and_then(|value| value.get("usage")),
            )]
        }
        "response.function_call_arguments.delta" => {
            state.has_received_arguments_delta = true;
            vec![build_chat_chunk(
                state,
                json!({
                    "tool_calls": [{
                        "index": state.function_call_index,
                        "function": {
                            "arguments": parsed.get("delta").and_then(Value::as_str).unwrap_or_default(),
                        }
                    }]
                }),
                None,
                parsed.get("response").and_then(|value| value.get("usage")),
            )]
        }
        "response.function_call_arguments.done" => {
            if state.has_received_arguments_delta {
                return Vec::new();
            }

            vec![build_chat_chunk(
                state,
                json!({
                    "tool_calls": [{
                        "index": state.function_call_index,
                        "function": {
                            "arguments": parsed.get("arguments").and_then(Value::as_str).unwrap_or_default(),
                        }
                    }]
                }),
                None,
                parsed.get("response").and_then(|value| value.get("usage")),
            )]
        }
        "response.output_item.done" => {
            let Some(item) = parsed.get("item").and_then(Value::as_object) else {
                return Vec::new();
            };
            if item.get("type").and_then(Value::as_str) != Some("function_call") {
                return Vec::new();
            }
            if state.has_tool_call_announced {
                state.has_tool_call_announced = false;
                return Vec::new();
            }

            state.function_call_index += 1;
            vec![build_chat_chunk(
                state,
                json!({
                    "role": "assistant",
                    "tool_calls": [{
                        "index": state.function_call_index,
                        "id": item.get("call_id").and_then(Value::as_str).unwrap_or_default(),
                        "type": "function",
                        "function": {
                            "name": item.get("name").and_then(Value::as_str).unwrap_or_default(),
                            "arguments": item.get("arguments").and_then(Value::as_str).unwrap_or_default(),
                        }
                    }]
                }),
                None,
                parsed.get("response").and_then(|value| value.get("usage")),
            )]
        }
        "response.completed" => {
            let finish_reason = if state.function_call_index >= 0 {
                "tool_calls"
            } else {
                "stop"
            };
            vec![build_chat_chunk(
                state,
                json!({}),
                Some(finish_reason),
                parsed.get("response").and_then(|value| value.get("usage")),
            )]
        }
        _ => {
            let _ = &event.event;
            Vec::new()
        }
    }
}

fn build_chat_chunk(
    state: &ChatStreamState,
    delta: Value,
    finish_reason: Option<&str>,
    usage: Option<&Value>,
) -> Value {
    let mut root = Map::new();
    root.insert("id".to_string(), Value::String(state.response_id.clone()));
    root.insert(
        "object".to_string(),
        Value::String("chat.completion.chunk".to_string()),
    );
    root.insert(
        "created".to_string(),
        Value::Number(serde_json::Number::from(state.created_at.max(0))),
    );
    root.insert("model".to_string(), Value::String(state.model.clone()));

    let mut choice = Map::new();
    choice.insert(
        "index".to_string(),
        Value::Number(serde_json::Number::from(0)),
    );
    choice.insert("delta".to_string(), delta);
    choice.insert(
        "finish_reason".to_string(),
        finish_reason
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
    );
    choice.insert(
        "native_finish_reason".to_string(),
        finish_reason
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
    );

    root.insert(
        "choices".to_string(),
        Value::Array(vec![Value::Object(choice)]),
    );
    if let Some(usage) = usage {
        root.insert("usage".to_string(), build_openai_usage(usage));
    }
    Value::Object(root)
}

impl SseDecoder {
    fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(chunk);
        self.take_ready_events()
    }

    fn finish(&mut self) -> Vec<SseEvent> {
        let mut events = self.take_ready_events();
        if !self.buffer.is_empty() {
            if let Some(event) = parse_sse_event(&self.buffer) {
                events.push(event);
            }
            self.buffer.clear();
        }
        events
    }

    fn take_ready_events(&mut self) -> Vec<SseEvent> {
        let mut events = Vec::new();
        while let Some(boundary) = find_sse_boundary(&self.buffer) {
            let block = self.buffer.drain(..boundary).collect::<Vec<_>>();
            let delimiter = if self.buffer.starts_with(b"\r\n\r\n") {
                4
            } else {
                2
            };
            self.buffer.drain(..delimiter);
            if let Some(event) = parse_sse_event(&block) {
                events.push(event);
            }
        }
        events
    }
}

fn find_sse_boundary(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .or_else(|| buffer.windows(2).position(|window| window == b"\n\n"))
}

fn parse_sse_event(block: &[u8]) -> Option<SseEvent> {
    let text = String::from_utf8_lossy(block);
    let mut event = None;
    let mut data_lines = Vec::new();

    for raw_line in text.lines() {
        let line = raw_line.trim_end_matches('\r');
        if let Some(value) = line.strip_prefix("event:") {
            event = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("data:") {
            data_lines.push(value.trim_start().to_string());
        }
    }

    if data_lines.is_empty() {
        return None;
    }

    Some(SseEvent {
        event,
        data: data_lines.join("\n"),
    })
}

fn json_error_response(status: StatusCode, message: &str) -> Response<Body> {
    let mut response = Json(json!({
        "error": {
            "message": message,
        }
    }))
    .into_response();
    *response.status_mut() = status;
    response
}

async fn update_proxy_target(context: &ProxyContext, candidate: &ProxyCandidate) {
    {
        let mut snapshot = context.shared.lock().await;
        snapshot.active_account_key = Some(candidate.account_key.clone());
        snapshot.active_account_id = Some(candidate.account_id.clone());
        snapshot.active_account_label = Some(match &candidate.source_kind {
            AccountSourceKind::Chatgpt => format!("ChatGPT · {}", candidate.label),
            AccountSourceKind::Relay => format!("API · {}", candidate.label),
        });
        snapshot.sequential_account_key = Some(candidate.account_key.clone());
    }

    if let Err(error) =
        persist_sequential_proxy_account_key_if_enabled(&context.storage, &candidate.account_key)
            .await
    {
        log::warn!("持久化 API 反代逐个模式账号失败: {error}");
    }
}

async fn persist_sequential_proxy_account_key_if_enabled(
    storage: &ProxyStorageContext,
    account_key: &str,
) -> Result<(), String> {
    let _guard = storage.store_lock.lock().await;
    let path = account_store_path_from_data_dir(&storage.data_dir);
    let mut store = load_store_from_path(&path)?;

    if !matches!(
        store.settings.api_proxy_load_balance_mode,
        ApiProxyLoadBalanceMode::Sequential
    ) {
        return Ok(());
    }

    if store.settings.api_proxy_sequential_account_key.as_deref() == Some(account_key) {
        return Ok(());
    }

    store.settings.api_proxy_sequential_account_key = Some(account_key.to_string());
    save_store_to_path(&path, &store)
}

async fn current_sequential_proxy_account_key(
    context: &ProxyContext,
    session_affinity_key: Option<&str>,
) -> Option<String> {
    let snapshot = context.shared.lock().await;
    sequential_runtime_account_key(&snapshot, session_affinity_key)
}

fn sequential_runtime_account_key(
    snapshot: &ApiProxyRuntimeSnapshot,
    session_affinity_key: Option<&str>,
) -> Option<String> {
    match session_affinity_key {
        Some(key) => {
            let now = now_unix_seconds();
            snapshot
                .sequential_session_affinity
                .get(key)
                .filter(|entry| now - entry.updated_at <= PROXY_SESSION_AFFINITY_TTL_SECS)
                .map(|entry| entry.account_key.clone())
        }
        None => snapshot.sequential_account_key.clone(),
    }
}

// Release a session binding only when it still points at the account that
// just failed (compare-and-delete semantics), so a request-scoped error
// cannot drop another account's binding.
async fn unbind_session_proxy_account(
    context: &ProxyContext,
    session_affinity_key: Option<&str>,
    account_key: &str,
) {
    let Some(session_key) = session_affinity_key else {
        return;
    };

    let mut snapshot = context.shared.lock().await;
    if snapshot
        .sequential_session_affinity
        .get(session_key)
        .map(|entry| entry.account_key.as_str())
        == Some(account_key)
    {
        snapshot.sequential_session_affinity.remove(session_key);
    }
}

async fn bind_sequential_session_proxy_account(
    context: &ProxyContext,
    session_affinity_key: Option<&str>,
    account_key: &str,
) {
    let Some(session_key) = session_affinity_key else {
        return;
    };

    let mut snapshot = context.shared.lock().await;
    if !snapshot
        .sequential_session_affinity
        .contains_key(session_key)
    {
        prune_proxy_session_affinity_if_needed(&mut snapshot);
    }
    snapshot.sequential_session_affinity.insert(
        session_key.to_string(),
        ApiProxySessionAffinity {
            account_key: account_key.to_string(),
            updated_at: now_unix_seconds(),
        },
    );
}

fn prune_proxy_session_affinity_if_needed(snapshot: &mut ApiProxyRuntimeSnapshot) {
    if snapshot.sequential_session_affinity.len() < MAX_PROXY_SESSION_AFFINITY_ENTRIES {
        return;
    }

    if let Some(oldest_key) = snapshot
        .sequential_session_affinity
        .iter()
        .min_by_key(|(_, entry)| entry.updated_at)
        .map(|(key, _)| key.clone())
    {
        snapshot.sequential_session_affinity.remove(&oldest_key);
    }
}

async fn update_proxy_error(context: &ProxyContext, error: Option<String>) {
    let mut snapshot = context.shared.lock().await;
    snapshot.last_error = error;
}

fn snapshot_handle_state(handle: &ApiProxyRuntimeHandle) -> ApiProxyHandleState {
    ApiProxyHandleState {
        port: handle.port,
        api_keys: handle.api_keys.clone(),
        task_finished: handle.task.is_finished(),
        shared: handle.shared.clone(),
    }
}

async fn status_from_handle_state(handle: ApiProxyHandleState) -> ApiProxyStatus {
    let snapshot = handle.shared.lock().await.clone();
    if handle.task_finished {
        enrich_codex_proxy_status(ApiProxyStatus {
            running: false,
            port: None,
            api_key: read_current_api_key(&handle.api_keys),
            base_url: None,
            lan_base_url: None,
            codex_proxy_bound: false,
            codex_proxy_restore_available: false,
            codex_proxy_base_url: None,
            codex_proxy_config_path: None,
            active_account_key: snapshot.active_account_key,
            active_account_id: snapshot.active_account_id,
            active_account_label: snapshot.active_account_label,
            last_error: snapshot.last_error,
        })
    } else {
        enrich_codex_proxy_status(ApiProxyStatus {
            running: true,
            port: Some(handle.port),
            api_key: read_current_api_key(&handle.api_keys),
            base_url: Some(proxy_base_url(handle.port)),
            lan_base_url: proxy_lan_base_url(handle.port),
            codex_proxy_bound: false,
            codex_proxy_restore_available: false,
            codex_proxy_base_url: None,
            codex_proxy_config_path: None,
            active_account_key: snapshot.active_account_key,
            active_account_id: snapshot.active_account_id,
            active_account_label: snapshot.active_account_label,
            last_error: snapshot.last_error,
        })
    }
}

fn stopped_status(api_key: Option<String>, last_error: Option<String>) -> ApiProxyStatus {
    enrich_codex_proxy_status(ApiProxyStatus {
        running: false,
        port: None,
        api_key,
        base_url: None,
        lan_base_url: None,
        codex_proxy_bound: false,
        codex_proxy_restore_available: false,
        codex_proxy_base_url: None,
        codex_proxy_config_path: None,
        active_account_key: None,
        active_account_id: None,
        active_account_label: None,
        last_error,
    })
}

fn enrich_codex_proxy_status(mut status: ApiProxyStatus) -> ApiProxyStatus {
    if let Ok(binding) = profile_files::codex_proxy_binding_state(status.base_url.as_deref()) {
        status.codex_proxy_bound = binding.bound;
        status.codex_proxy_restore_available = binding.restore_available;
        status.codex_proxy_base_url = binding.current_base_url;
        status.codex_proxy_config_path = binding.config_path;
    }
    status
}

fn resolve_codex_upstream_base_url() -> String {
    format!(
        "{}/backend-api/codex",
        resolve_chatgpt_base_origin().trim_end_matches('/')
    )
}

fn minimal_account_warmup_payload() -> Result<Value, String> {
    normalize_openai_responses_request(json!({
        "model": DEFAULT_API_PROXY_MODEL,
        "input": "hello",
        "stream": true,
        "store": false,
        "reasoning": {"effort": "none", "summary": "auto"},
        "text": {"verbosity": "low"},
        "service_tier": "default"
    }))
    .map(|(payload, _)| payload)
}

/// Sends the smallest supported real inference request for intentionally
/// activating an account's short usage window. The fixed prompt contains no
/// user data, disables storage and reasoning, and never retries by itself.
pub(crate) async fn send_minimal_account_warmup_request(auth_json: &Value) -> Result<(), String> {
    let auth = extract_auth(auth_json)?;
    let payload = payload_for_upstream(&minimal_account_warmup_payload()?);
    let body =
        serde_json::to_vec(&payload).map_err(|error| format!("序列化账号预热请求失败: {error}"))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(ACCOUNT_WARMUP_TIMEOUT_SECS))
        .build()
        .map_err(|error| format!("创建账号预热客户端失败: {error}"))?;
    let url = format!(
        "{}/responses",
        resolve_codex_upstream_base_url().trim_end_matches('/')
    );
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .header("ChatGPT-Account-Id", auth.account_id)
        .header("Accept", "text/event-stream")
        .header("Content-Type", "application/json")
        .header("User-Agent", CODEX_USER_AGENT)
        .header("Originator", "codex_cli_rs")
        .header("Version", CODEX_CLIENT_VERSION)
        .header("session-id", uuid::Uuid::new_v4().to_string())
        .header(RESPONSES_LITE_HEADER, "true")
        .body(body)
        .send()
        .await
        .map_err(|error| format!("发送账号预热请求失败: {error}"))?;
    let status = response.status();
    let response_body = read_account_warmup_response_limited(response).await?;
    let response_text = String::from_utf8_lossy(&response_body);

    if !status.is_success() {
        return Err(format!(
            "账号预热请求被上游拒绝 HTTP {}: {}",
            status.as_u16(),
            truncate_for_error(response_text.trim(), 240)
        ));
    }
    if response_text.contains("response.completed") {
        return Ok(());
    }
    if response_text.contains("response.failed") {
        return Err(format!(
            "账号预热推理失败: {}",
            truncate_for_error(response_text.trim(), 240)
        ));
    }
    Err("账号预热响应未包含完成事件".to_string())
}

async fn read_account_warmup_response_limited(
    response: reqwest::Response,
) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("读取账号预热响应失败: {error}"))?;
        if body.len().saturating_add(chunk.len()) > MAX_ACCOUNT_WARMUP_RESPONSE_BYTES {
            return Err("账号预热响应超过 1 MiB 安全上限".to_string());
        }
        body.extend_from_slice(&chunk);
        if body
            .windows(b"response.completed".len())
            .any(|window| window == b"response.completed")
            || body
                .windows(b"response.failed".len())
                .any(|window| window == b"response.failed")
        {
            break;
        }
    }
    Ok(body)
}

fn proxy_base_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/v1")
}

fn proxy_lan_base_url(port: u16) -> Option<String> {
    detect_preferred_lan_ip().map(|ip| format!("http://{ip}:{port}/v1"))
}

fn detect_preferred_lan_ip() -> Option<Ipv4Addr> {
    let interfaces = if_addrs::get_if_addrs().ok()?;
    let mut fallback_private = None;

    for interface in interfaces {
        let ip = match interface.addr {
            IfAddr::V4(addr) if !addr.ip.is_loopback() => addr.ip,
            _ => continue,
        };

        if is_preferred_lan_ip(ip) {
            return Some(ip);
        }

        if fallback_private.is_none() && is_private_ipv4(ip) {
            fallback_private = Some(ip);
        }
    }

    fallback_private
}

fn is_preferred_lan_ip(ip: Ipv4Addr) -> bool {
    let [first, second, ..] = ip.octets();
    first == 192 && second == 168
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    let [first, second, ..] = ip.octets();
    first == 10 || (first == 172 && (16..=31).contains(&second)) || is_preferred_lan_ip(ip)
}

fn resolve_proxy_request_body_limit_bytes() -> usize {
    resolve_proxy_request_body_limit_bytes_from_mib_value(
        std::env::var(PROXY_REQUEST_BODY_LIMIT_MIB_ENV_VAR)
            .ok()
            .as_deref(),
    )
}

fn resolve_proxy_request_body_limit_bytes_from_mib_value(value: Option<&str>) -> usize {
    parse_proxy_request_body_limit_mib(value)
        .and_then(|mib| mib.checked_mul(1024 * 1024))
        .unwrap_or(DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES)
}

fn parse_proxy_request_body_limit_mib(value: Option<&str>) -> Option<usize> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }

    match raw.parse::<usize>() {
        Ok(parsed) if parsed > 0 => Some(parsed),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::account_to_proxy_candidate;
    use super::anthropic_upstream_session_id;
    use super::api_proxy_disabled_model_set;
    use super::api_proxy_requested_models_from_payload;
    use super::api_proxy_usage_bucket_seconds;
    use super::api_proxy_usage_model_from_payload;
    use super::api_proxy_visible_models;
    use super::api_proxy_visible_models_for_key;
    use super::append_api_proxy_usage_event;
    use super::apply_anthropic_upstream_session_id;
    use super::apply_configured_default_service_tier;
    use super::build_api_proxy_usage_stats;
    use super::build_compact_sse_failure;
    use super::build_compact_sse_response;
    use super::candidate_upstream_url;
    use super::clear_api_proxy_usage_stats_with_storage;
    use super::convert_anthropic_messages_request_to_codex;
    use super::convert_completed_response_to_anthropic_message;
    use super::convert_completed_response_to_chat_completion;
    use super::convert_openai_chat_request_to_codex;
    use super::convert_openai_image_edit_request_to_codex;
    use super::convert_openai_image_generation_request_to_codex;
    use super::convert_responses_image_output_to_images_response;
    use super::create_api_proxy_key_with_runtime;
    use super::decode_proxy_request_body;
    use super::delete_api_proxy_key_with_runtime;
    use super::detect_content_encoding_by_magic;
    use super::ensure_api_proxy_key_allows_payload;
    use super::ensure_api_proxy_payload_models_enabled;
    use super::export_api_proxy_usage_with_storage;
    use super::extract_completed_response_from_sse;
    use super::find_http_header_end;
    use super::get_api_proxy_key_usage_logs_with_storage;
    use super::get_api_proxy_usage_stats_with_storage;
    use super::host_matches_no_proxy;
    use super::is_responses_terminal_event;
    use super::list_api_proxy_keys_with_storage;
    use super::minimal_account_warmup_payload;
    use super::normalize_codex_compact_reasoning_effort;
    use super::normalize_openai_compact_request;
    use super::normalize_openai_responses_request;
    use super::normalize_responses_websocket_create;
    use super::now_unix_seconds;
    use super::order_proxy_candidates_for_request;
    use super::parse_compact_upstream_response;
    use super::parse_http_proxy_config;
    use super::parse_json_request;
    use super::parse_proxy_request_body_limit_mib;
    use super::payload_for_candidate;
    use super::regenerate_api_proxy_key_with_runtime;
    use super::request_session_affinity_key;
    use super::resolve_proxy_request_body_limit_bytes_from_mib_value;
    use super::resolved_proxy_default_service_tier;
    use super::rewrite_response_models_for_client;
    use super::rewrite_sse_event_data_models_for_client;
    use super::sanitize_api_proxy_disabled_models_for_settings;
    use super::sequential_account_key_for_request;
    use super::sequential_runtime_account_key;
    use super::should_use_responses_websocket;
    use super::translate_sse_event_to_anthropic_event;
    use super::translate_sse_event_to_chat_chunk;
    use super::translate_sse_event_to_image_chunk;
    use super::update_api_proxy_key_with_runtime;
    use super::websocket_target_host_port;
    use super::AnthropicStreamState;
    use super::ApiProxyUsageEvent;
    use super::ChatStreamState;
    use super::CodexUpstreamEndpoint;
    use super::ImageMultipartRequest;
    use super::ProxyCandidate;
    use super::ProxyContext;
    use super::ProxyLoadBalanceConfig;
    use super::ProxyStorageContext;
    use super::SseEvent;
    use super::API_PROXY_USAGE_DATABASE_FILE_NAME;
    use super::API_PROXY_USAGE_FILE_NAME;
    use super::API_PROXY_USAGE_RANGE_1H_SECONDS;
    use super::API_PROXY_USAGE_RANGE_30D_SECONDS;
    use super::COMPACT_SSE_KEEPALIVE;
    use super::COMPACT_SSE_KEEPALIVE_INTERVAL_SECS;
    use super::DEFAULT_API_PROXY_REASONING_EFFORT;
    use super::DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES;
    use super::DEFAULT_UPSTREAM_SERVICE_TIER;
    use crate::models::AccountSourceKind;
    use crate::models::ApiProxyKey;
    use crate::models::ApiProxyLoadBalanceMode;
    use crate::models::AppSettings;
    use crate::models::CreateApiProxyKeyInput;
    use crate::models::StoredAccount;
    use crate::models::UpdateApiProxyKeyInput;
    use crate::models::UsageSnapshot;
    use crate::models::UsageWindow;
    use crate::state::ApiProxyRuntimeSnapshot;
    use crate::state::ApiProxySessionAffinity;
    use axum::body::Bytes;
    use axum::http::HeaderMap;
    use axum::http::HeaderValue;
    use axum::http::StatusCode;
    use serde_json::json;
    use serde_json::Value;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::RwLock;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    #[test]
    fn minimal_account_warmup_uses_fixed_low_cost_payload() {
        let payload = minimal_account_warmup_payload().expect("warmup payload should normalize");
        assert_eq!(
            payload.get("model").and_then(Value::as_str),
            Some("gpt-5.6-sol")
        );
        assert_eq!(payload.get("store").and_then(Value::as_bool), Some(false));
        assert_eq!(payload.get("stream").and_then(Value::as_bool), Some(true));
        assert_eq!(
            payload.pointer("/reasoning/effort").and_then(Value::as_str),
            Some("none")
        );
        assert_eq!(
            payload.pointer("/text/verbosity").and_then(Value::as_str),
            Some("low")
        );
        assert!(serde_json::to_string(&payload)
            .expect("serialize warmup payload")
            .contains("hello"));
        assert!(payload.get("max_output_tokens").is_none());
    }

    fn proxy_candidate(
        label: &str,
        account_key: &str,
        one_week_percent: Option<f64>,
        five_hour_percent: Option<f64>,
        auth_refresh_blocked: bool,
    ) -> ProxyCandidate {
        proxy_candidate_with_plan(
            label,
            account_key,
            one_week_percent,
            five_hour_percent,
            auth_refresh_blocked,
            "team",
        )
    }

    fn proxy_candidate_with_plan(
        label: &str,
        account_key: &str,
        one_week_percent: Option<f64>,
        five_hour_percent: Option<f64>,
        auth_refresh_blocked: bool,
        plan_type: &str,
    ) -> ProxyCandidate {
        ProxyCandidate {
            id: account_key.to_string(),
            label: label.to_string(),
            source_kind: AccountSourceKind::Chatgpt,
            account_key: account_key.to_string(),
            account_id: account_key.to_string(),
            access_token: "token".to_string(),
            auth_json: json!({}),
            api_base_url: None,
            model_name: None,
            variant_key: account_key.to_string(),
            plan_type: Some(plan_type.to_string()),
            usage: Some(UsageSnapshot {
                fetched_at: 1,
                plan_type: Some(plan_type.to_string()),
                five_hour: five_hour_percent.map(|used_percent| UsageWindow {
                    used_percent,
                    window_seconds: 18_000,
                    reset_at: None,
                }),
                one_week: one_week_percent.map(|used_percent| UsageWindow {
                    used_percent,
                    window_seconds: 604_800,
                    reset_at: None,
                }),
                credits: None,
                reset_credits: None,
            }),
            auth_refresh_blocked,
            auth_refresh_error: None,
            updated_at: 1,
        }
    }

    fn load_balance_config(
        mode: ApiProxyLoadBalanceMode,
        sequential_limit: f64,
    ) -> ProxyLoadBalanceConfig {
        ProxyLoadBalanceConfig {
            mode,
            sequential_five_hour_limit_percent: sequential_limit,
        }
    }

    fn candidate_labels(candidates: &[ProxyCandidate]) -> Vec<&str> {
        candidates
            .iter()
            .map(|candidate| candidate.label.as_str())
            .collect()
    }

    fn usage_event(timestamp: i64, model: &str, calls: i64, tokens: i64) -> ApiProxyUsageEvent {
        ApiProxyUsageEvent {
            timestamp,
            key_id: None,
            key_label: None,
            model: model.to_string(),
            route: None,
            reasoning_effort: None,
            service_tier: None,
            calls,
            tokens,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
        }
    }

    fn api_proxy_key(
        allowed_models: Vec<&str>,
        reasoning: Vec<&str>,
        service_tiers: Vec<&str>,
    ) -> ApiProxyKey {
        ApiProxyKey {
            id: "key-1".to_string(),
            label: "tenant".to_string(),
            key: "sk-test".to_string(),
            enabled: true,
            allowed_models: allowed_models.into_iter().map(str::to_string).collect(),
            allowed_reasoning_efforts: reasoning.into_iter().map(str::to_string).collect(),
            allowed_service_tiers: service_tiers.into_iter().map(str::to_string).collect(),
            created_at: 1,
            updated_at: 1,
        }
    }

    fn temp_proxy_storage_context(test_name: &str) -> (ProxyStorageContext, PathBuf) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let data_dir = std::env::temp_dir().join(format!("codex-tools-{test_name}-{nonce}"));
        std::fs::create_dir_all(&data_dir).expect("temporary proxy storage should be writable");
        (
            ProxyStorageContext {
                data_dir: data_dir.clone(),
                store_lock: Arc::new(tokio::sync::Mutex::new(())),
                auth_refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
                usage_writer: Arc::new(tokio::sync::Mutex::new(None)),
                sync_active_auth_on_refresh: false,
            },
            data_dir,
        )
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_proxy_binds_lan_interfaces_but_keeps_loopback_base_url() {
        assert_eq!(super::DESKTOP_API_PROXY_BIND_HOST, "0.0.0.0");
        assert_eq!(super::proxy_base_url(8787), "http://127.0.0.1:8787/v1");
    }

    #[test]
    fn disabled_account_is_not_proxy_candidate() {
        let account = StoredAccount {
            id: "disabled".to_string(),
            label: "disabled".to_string(),
            source_kind: Default::default(),
            principal_id: Some("disabled@example.com".to_string()),
            email: Some("disabled@example.com".to_string()),
            account_id: "account-1".to_string(),
            plan_type: Some("team".to_string()),
            auth_json: json!({}),
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
            api_proxy_enabled: false,
        };

        assert!(account_to_proxy_candidate(account).is_none());
    }

    #[test]
    fn relay_account_is_proxy_candidate_with_its_api_settings() {
        let account = StoredAccount {
            id: "relay-1".to_string(),
            label: "relay".to_string(),
            source_kind: AccountSourceKind::Relay,
            principal_id: None,
            email: None,
            account_id: "relay:relay-1".to_string(),
            plan_type: None,
            auth_json: json!({}),
            api_base_url: Some("https://relay.example.com/v1/".to_string()),
            api_key: Some("sk-relay".to_string()),
            model_name: Some("relay-model".to_string()),
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
            api_proxy_enabled: true,
        };

        let candidate = account_to_proxy_candidate(account).expect("relay should proxy");
        assert!(matches!(&candidate.source_kind, AccountSourceKind::Relay));
        assert_eq!(
            candidate.api_base_url.as_deref(),
            Some("https://relay.example.com/v1")
        );
        assert_eq!(candidate.access_token, "sk-relay");
        assert_eq!(candidate.model_name.as_deref(), Some("relay-model"));

        let payload = payload_for_candidate(
            &candidate,
            &json!({
                "model": "gpt-5.4",
                "input": "hello"
            }),
        );
        assert_eq!(
            payload.get("model").and_then(Value::as_str),
            Some("relay-model")
        );

        let chatgpt_candidate =
            proxy_candidate("chatgpt", "chatgpt-account", Some(10.0), Some(10.0), false);
        let ordered = order_proxy_candidates_for_request(
            vec![chatgpt_candidate, candidate],
            load_balance_config(ApiProxyLoadBalanceMode::Average, 80.0),
            None,
            None,
        );
        assert_eq!(candidate_labels(&ordered), vec!["relay", "chatgpt"]);
    }

    #[test]
    fn normalizes_compact_request_and_preserves_supported_request_fields() {
        let payload = normalize_openai_compact_request(json!({
            "model": "gpt-5.4",
            "input": [{"type": "message", "role": "user", "content": "hi"}],
            "instructions": "be brief",
            "stream": true,
            "store": true,
            "prompt_cache_key": "cache-key",
            "service_tier": "fast",
            "reasoning": {"effort": "high"}
        }))
        .expect("compact request should normalize");

        assert_eq!(
            payload.get("model").and_then(Value::as_str),
            Some("gpt-5.4")
        );
        assert!(payload.get("stream").is_none());
        assert!(payload.get("store").is_none());
        assert_eq!(
            payload.get("prompt_cache_key").and_then(Value::as_str),
            Some("cache-key")
        );
        assert_eq!(
            payload.get("service_tier").and_then(Value::as_str),
            Some("priority")
        );
        assert_eq!(
            payload.pointer("/reasoning/effort").and_then(Value::as_str),
            Some("high")
        );

        let default_tier = normalize_openai_compact_request(json!({
            "model": "gpt-5.4",
            "input": []
        }))
        .expect("compact request should apply the configured default tier");
        assert_eq!(
            default_tier.get("service_tier").and_then(Value::as_str),
            Some("default")
        );
        assert!(normalize_openai_compact_request(json!({
            "model": "gpt-5.4",
            "input": [],
            "service_tier": "turbo"
        }))
        .is_err());
        assert!(normalize_openai_compact_request(json!({
            "model": "gpt-5.4",
            "input": [],
            "prompt_cache_key": 7
        }))
        .is_err());
    }

    #[test]
    fn builds_compact_upstream_urls_and_sse_bridge() {
        let context = ProxyContext {
            storage: ProxyStorageContext {
                data_dir: PathBuf::from("/tmp"),
                store_lock: Arc::new(tokio::sync::Mutex::new(())),
                auth_refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
                usage_writer: Arc::new(tokio::sync::Mutex::new(None)),
                sync_active_auth_on_refresh: false,
            },
            api_keys: Arc::new(RwLock::new(Vec::new())),
            upstream_base_url: "https://chatgpt.com/backend-api/codex".to_string(),
            client: reqwest::Client::new(),
            shared: Arc::new(tokio::sync::Mutex::new(ApiProxyRuntimeSnapshot::default())),
            request_body_limit_bytes: DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES,
        };
        let chatgpt = ProxyCandidate {
            id: "c1".to_string(),
            label: "chatgpt".to_string(),
            source_kind: AccountSourceKind::Chatgpt,
            account_key: "k1".to_string(),
            account_id: "acc".to_string(),
            access_token: "token".to_string(),
            auth_json: json!({}),
            api_base_url: None,
            model_name: None,
            variant_key: "v".to_string(),
            plan_type: None,
            usage: None,
            auth_refresh_blocked: false,
            auth_refresh_error: None,
            updated_at: 1,
        };
        let relay = ProxyCandidate {
            id: "r1".to_string(),
            label: "relay".to_string(),
            source_kind: AccountSourceKind::Relay,
            account_key: "k2".to_string(),
            account_id: "relay".to_string(),
            access_token: "sk".to_string(),
            auth_json: json!({}),
            api_base_url: Some("https://relay.example.com/v1".to_string()),
            model_name: Some("gpt-5.4".to_string()),
            variant_key: "v".to_string(),
            plan_type: None,
            usage: None,
            auth_refresh_blocked: false,
            auth_refresh_error: None,
            updated_at: 1,
        };

        assert_eq!(
            candidate_upstream_url(&context, &chatgpt, CodexUpstreamEndpoint::Compact).unwrap(),
            "https://chatgpt.com/backend-api/codex/responses/compact"
        );
        assert_eq!(
            candidate_upstream_url(&context, &relay, CodexUpstreamEndpoint::Compact).unwrap(),
            "https://relay.example.com/v1/responses/compact"
        );

        let response = json!({
            "id": "resp_1",
            "output": [{"type": "compaction", "summary": "done"}],
            "usage": {"input_tokens": 1, "output_tokens": 2, "total_tokens": 3}
        });
        let sse =
            String::from_utf8(build_compact_sse_response(&response).unwrap().to_vec()).unwrap();
        assert!(sse.contains("event: response.output_item.done"));
        assert!(sse.contains("event: response.completed"));
        assert!(sse.contains("\"type\":\"compaction\""));

        let failed = String::from_utf8(build_compact_sse_failure("timeout").to_vec()).unwrap();
        assert!(failed.contains("event: response.failed"));
        assert!(failed.contains("timeout"));
        assert!(COMPACT_SSE_KEEPALIVE.contains("event: response.in_progress"));
        assert_eq!(COMPACT_SSE_KEEPALIVE_INTERVAL_SECS, 10);

        let raw = br#"{"model":"gpt-5.4","input":"hi"}"#;
        let zstd_body = zstd::stream::encode_all(&raw[..], 0).expect("zstd encode");
        assert_eq!(
            detect_content_encoding_by_magic(&zstd_body).as_deref(),
            Some("zstd")
        );
        let decoded = decode_proxy_request_body(
            &HeaderMap::new(),
            Bytes::from(zstd_body.clone()),
            DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES,
        )
        .expect("decode zstd body by magic");
        assert_eq!(decoded.as_ref(), raw);
        let parsed = parse_json_request(
            &HeaderMap::new(),
            decoded,
            DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES,
        )
        .expect("parse decoded json");
        assert_eq!(parsed.get("model").and_then(Value::as_str), Some("gpt-5.4"));
        let oversized =
            decode_proxy_request_body(&HeaderMap::new(), Bytes::from(zstd_body), raw.len() - 1)
                .expect_err("decoded body limit must be enforced");
        assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let expansion_payload = vec![b'x'; super::MIN_PROXY_DECOMPRESSED_BODY_BUDGET_BYTES * 2];
        let expansion_body =
            zstd::stream::encode_all(&expansion_payload[..], 0).expect("zstd expansion encode");
        let expansion_error = decode_proxy_request_body(
            &HeaderMap::new(),
            Bytes::from(expansion_body),
            DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES,
        )
        .expect_err("high-ratio compressed bodies must be rejected");
        assert_eq!(expansion_error.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let mut gzip_body = Vec::new();
        {
            use flate2::write::GzEncoder;
            use flate2::Compression;
            use std::io::Write as _;
            let mut encoder = GzEncoder::new(&mut gzip_body, Compression::default());
            encoder.write_all(raw).unwrap();
            encoder.finish().unwrap();
        }
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", HeaderValue::from_static("gzip"));
        let decoded = decode_proxy_request_body(
            &headers,
            Bytes::from(gzip_body),
            DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES,
        )
        .expect("decode gzip body");
        assert_eq!(decoded.as_ref(), raw);

        let mut deflate_body = Vec::new();
        {
            use flate2::write::ZlibEncoder;
            use flate2::Compression;
            use std::io::Write as _;
            let mut encoder = ZlibEncoder::new(&mut deflate_body, Compression::default());
            encoder.write_all(raw).unwrap();
            encoder.finish().unwrap();
        }
        headers.insert("content-encoding", HeaderValue::from_static("deflate"));
        let decoded = decode_proxy_request_body(
            &headers,
            Bytes::from(deflate_body),
            DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES,
        )
        .expect("decode standard zlib-wrapped HTTP deflate body");
        assert_eq!(decoded.as_ref(), raw);

        let parsed =
            parse_compact_upstream_response(br#"{"id":"resp_x","output":[{"type":"compaction"}]}"#)
                .unwrap();
        assert_eq!(parsed.get("id").and_then(Value::as_str), Some("resp_x"));

        let downgraded = normalize_codex_compact_reasoning_effort(json!({
            "model": "gpt-5.6",
            "reasoning": {"effort": "max"}
        }));
        assert_eq!(
            downgraded
                .pointer("/reasoning/effort")
                .and_then(Value::as_str),
            Some("xhigh")
        );
    }

    #[test]
    fn api_proxy_usage_stats_bucket_calls_and_tokens_per_model() {
        let now = 3_600;
        let bucket_seconds = api_proxy_usage_bucket_seconds(API_PROXY_USAGE_RANGE_1H_SECONDS);
        let call_bucket = now - 120;
        let token_bucket = now - 60;
        let events = vec![
            usage_event(call_bucket, "gpt-5.4", 1, 0),
            usage_event(token_bucket, "gpt-5.4", 0, 42),
            usage_event(now - API_PROXY_USAGE_RANGE_1H_SECONDS - 1, "gpt-5.4", 1, 99),
            usage_event(now - 30, "gpt-5.5", 1, 12),
        ];

        let stats = build_api_proxy_usage_stats(&events, now, API_PROXY_USAGE_RANGE_1H_SECONDS);

        assert_eq!(stats.range_seconds, API_PROXY_USAGE_RANGE_1H_SECONDS);
        assert_eq!(stats.bucket_seconds, bucket_seconds);
        assert_eq!(stats.series.len(), 2);

        let gpt5 = stats
            .series
            .iter()
            .find(|series| series.model == "gpt-5.4")
            .expect("gpt-5.4 series");
        assert_eq!(gpt5.total_calls, 1);
        assert_eq!(gpt5.total_tokens, 42);
        assert!(gpt5
            .points
            .iter()
            .all(|point| point.timestamp % bucket_seconds == 0));
        assert_eq!(
            gpt5.points
                .iter()
                .find(|point| point.timestamp == call_bucket)
                .map(|point| point.calls),
            Some(1)
        );
        assert_eq!(
            gpt5.points
                .iter()
                .find(|point| point.timestamp == token_bucket)
                .map(|point| point.tokens),
            Some(42)
        );

        for series in &stats.series {
            assert_eq!(series.points.len(), gpt5.points.len());
        }
    }

    #[test]
    fn api_proxy_usage_prefers_image_tool_model_over_controller_model() {
        let payload = json!({
            "model": "gpt-5.5",
            "tools": [{
                "type": "image_generation",
                "model": "gpt-image-2"
            }]
        });

        let model = api_proxy_usage_model_from_payload(&payload);

        assert_eq!(model, "gpt-image-2");
    }

    #[test]
    fn api_proxy_usage_finds_image_tool_model_in_responses_lite_input() {
        let payload = json!({
            "model": "gpt-5.6-sol",
            "input": [{
                "type": "additional_tools",
                "role": "developer",
                "tools": [{
                    "type": "image_generation",
                    "model": "gpt-image-2"
                }]
            }]
        });

        assert_eq!(api_proxy_usage_model_from_payload(&payload), "gpt-image-2");
        assert_eq!(
            api_proxy_requested_models_from_payload(&payload),
            vec!["gpt-image-2"]
        );
    }

    #[test]
    fn sanitize_api_proxy_disabled_models_filters_unknown_and_orders_results() {
        let disabled = sanitize_api_proxy_disabled_models_for_settings(vec![
            "gpt-image-2".to_string(),
            "gpt-5-4".to_string(),
            "unknown-model".to_string(),
            "gpt5.4".to_string(),
        ]);

        assert_eq!(disabled, vec!["gpt-5.4", "gpt-image-2"]);
    }

    #[test]
    fn api_proxy_supported_models_only_exposes_current_allowlist() {
        assert_eq!(
            super::get_api_proxy_supported_models_internal(),
            vec![
                "gpt-5.6-sol",
                "gpt-5.6-terra",
                "gpt-5.6-luna",
                "gpt-5.5",
                "gpt-5.4",
                "gpt-image-2",
            ]
        );
    }

    #[test]
    fn api_proxy_sanitizers_accept_all_supported_reasoning_and_speed_options() {
        assert_eq!(
            super::sanitize_api_proxy_reasoning_efforts(vec![
                "max".to_string(),
                "none".to_string(),
                "xhigh".to_string(),
                "minimal".to_string(),
                "low".to_string(),
                "medium".to_string(),
                "high".to_string(),
                "invalid".to_string(),
            ]),
            vec!["none", "minimal", "low", "medium", "high", "xhigh", "max"]
        );
        assert_eq!(
            super::sanitize_api_proxy_service_tiers(vec![
                "priority".to_string(),
                "default".to_string(),
                "auto".to_string(),
                "flex".to_string(),
            ]),
            vec!["auto", "default", "fast", "flex"]
        );
    }

    #[test]
    fn api_proxy_visible_models_hide_disabled_entries() {
        let settings = AppSettings {
            api_proxy_disabled_models: vec!["gpt-5.4".to_string(), "gpt-image-2".to_string()],
            ..AppSettings::default()
        };

        let visible = api_proxy_visible_models(&settings);
        let disabled = api_proxy_disabled_model_set(&settings);

        assert!(disabled.contains("gpt-5.4"));
        assert!(disabled.contains("gpt-image-2"));
        assert!(!visible.contains(&"gpt-5.4"));
        assert!(!visible.contains(&"gpt-image-2"));
        assert!(visible.contains(&"gpt-5.5"));
    }

    #[test]
    fn api_proxy_visible_models_are_intersected_with_key_binding() {
        let settings = AppSettings {
            api_proxy_disabled_models: vec!["gpt-5.4".to_string()],
            ..AppSettings::default()
        };
        let key = api_proxy_key(vec!["gpt-5.4", "gpt-5.5"], vec![], vec![]);

        let visible = api_proxy_visible_models_for_key(&settings, &key);

        assert_eq!(visible, vec!["gpt-5.5"]);
    }

    #[tokio::test]
    async fn api_proxy_key_store_creates_updates_regenerates_and_deletes_keys() {
        let (storage, data_dir) = temp_proxy_storage_context("api-key-store-crud");
        let runtime_slot = tokio::sync::Mutex::new(None);

        let initial = list_api_proxy_keys_with_storage(&storage)
            .await
            .expect("default platform key should be initialized");
        assert_eq!(initial.len(), 1);
        assert!(initial[0].enabled);

        let created = create_api_proxy_key_with_runtime(
            &storage,
            &runtime_slot,
            CreateApiProxyKeyInput {
                label: "Tenant A".to_string(),
                key: Some("sk-tenant-a-fixed".to_string()),
                allowed_models: vec!["gpt-5.5".to_string(), "unknown-model".to_string()],
                allowed_reasoning_efforts: vec!["high".to_string(), "invalid".to_string()],
                allowed_service_tiers: vec!["priority".to_string(), "invalid".to_string()],
            },
        )
        .await
        .expect("custom platform key should be created");
        assert_eq!(created.len(), 2);

        let tenant_id = created
            .iter()
            .find(|key| key.key == "sk-tenant-a-fixed")
            .expect("created tenant key should be returned")
            .id
            .clone();
        let tenant = created
            .iter()
            .find(|key| key.id == tenant_id)
            .expect("created tenant key should be found by id");
        assert_eq!(tenant.allowed_models, vec!["gpt-5.5"]);
        assert_eq!(tenant.allowed_reasoning_efforts, vec!["high"]);
        assert_eq!(tenant.allowed_service_tiers, vec!["fast"]);

        let updated = update_api_proxy_key_with_runtime(
            &storage,
            &runtime_slot,
            UpdateApiProxyKeyInput {
                id: tenant_id.clone(),
                label: Some("Tenant A disabled".to_string()),
                enabled: Some(false),
                allowed_models: Some(vec!["gpt-5.4".to_string()]),
                allowed_reasoning_efforts: Some(vec!["low".to_string()]),
                allowed_service_tiers: Some(vec!["flex".to_string()]),
            },
        )
        .await
        .expect("created tenant key should be updateable");
        let updated_tenant = updated
            .iter()
            .find(|key| key.id == tenant_id)
            .expect("updated tenant key should still exist");
        assert_eq!(updated_tenant.label, "Tenant A disabled");
        assert!(!updated_tenant.enabled);
        assert_eq!(updated_tenant.allowed_models, vec!["gpt-5.4"]);
        assert_eq!(updated_tenant.allowed_reasoning_efforts, vec!["low"]);
        assert_eq!(updated_tenant.allowed_service_tiers, vec!["flex"]);

        let previous_key_value = updated_tenant.key.clone();
        let regenerated =
            regenerate_api_proxy_key_with_runtime(&storage, &runtime_slot, Some(&tenant_id))
                .await
                .expect("created tenant key should be regeneratable");
        let regenerated_tenant = regenerated
            .iter()
            .find(|key| key.id == tenant_id)
            .expect("regenerated tenant key should still exist");
        assert_ne!(regenerated_tenant.key, previous_key_value);

        let after_delete = delete_api_proxy_key_with_runtime(&storage, &runtime_slot, &tenant_id)
            .await
            .expect("created tenant key should be deleteable");
        assert_eq!(after_delete.len(), 1);
        assert_ne!(after_delete[0].id, tenant_id);

        let fallback =
            delete_api_proxy_key_with_runtime(&storage, &runtime_slot, after_delete[0].id.as_str())
                .await
                .expect("deleting the last key should create a fallback key");
        assert_eq!(fallback.len(), 1);
        assert!(fallback[0].enabled);
        assert_ne!(fallback[0].id, after_delete[0].id);

        std::fs::remove_dir_all(data_dir).expect("temporary proxy storage should be removable");
    }

    #[tokio::test]
    async fn api_proxy_key_usage_logs_preserve_key_route_reasoning_and_service_tier() {
        let (storage, data_dir) = temp_proxy_storage_context("api-key-usage-logs");
        let now = now_unix_seconds();
        append_api_proxy_usage_event(
            &storage,
            ApiProxyUsageEvent {
                timestamp: now,
                key_id: Some("key-a".to_string()),
                key_label: Some("Tenant A".to_string()),
                model: "gpt-5.5".to_string(),
                route: Some("/v1/responses".to_string()),
                reasoning_effort: Some("high".to_string()),
                service_tier: Some("fast".to_string()),
                calls: 1,
                tokens: 99,
                input_tokens: Some(60),
                output_tokens: Some(39),
                cached_input_tokens: Some(10),
            },
        )
        .await
        .expect("usage event should be appended");
        append_api_proxy_usage_event(&storage, usage_event(now - 1, "gpt-5.4", 1, 12))
            .await
            .expect("legacy usage event should be appended");

        let logs = get_api_proxy_key_usage_logs_with_storage(&storage, Some(10))
            .await
            .expect("per-key usage logs should be readable");

        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].key_id.as_deref(), Some("key-a"));
        assert_eq!(logs[0].key_label.as_deref(), Some("Tenant A"));
        assert_eq!(logs[0].model, "gpt-5.5");
        assert_eq!(logs[0].route.as_deref(), Some("/v1/responses"));
        assert_eq!(logs[0].reasoning_effort.as_deref(), Some("high"));
        assert_eq!(logs[0].service_tier.as_deref(), Some("fast"));
        assert_eq!(logs[0].calls, 1);
        assert_eq!(logs[0].tokens, 99);

        drop(storage);
        std::fs::remove_dir_all(data_dir).expect("temporary proxy storage should be removable");
    }

    #[test]
    fn api_proxy_payload_model_validation_rejects_disabled_chat_model() {
        let settings = AppSettings {
            api_proxy_disabled_models: vec!["gpt-5.4".to_string()],
            ..AppSettings::default()
        };
        let payload = json!({ "model": "gpt-5.4" });

        let error = ensure_api_proxy_payload_models_enabled(&payload, &settings)
            .expect_err("disabled chat model should be rejected");

        assert!(error.contains("gpt-5.4"));
    }

    #[test]
    fn api_proxy_payload_model_validation_prefers_image_tool_model() {
        let settings = AppSettings {
            api_proxy_disabled_models: vec!["gpt-image-2".to_string()],
            ..AppSettings::default()
        };
        let payload = json!({
            "model": "gpt-5.5",
            "tools": [{
                "type": "image_generation",
                "model": "gpt-image-2"
            }]
        });

        let models = api_proxy_requested_models_from_payload(&payload);
        let error = ensure_api_proxy_payload_models_enabled(&payload, &settings)
            .expect_err("disabled image model should be rejected");

        assert_eq!(models, vec!["gpt-image-2"]);
        assert!(error.contains("gpt-image-2"));
    }

    #[test]
    fn api_proxy_key_validation_rejects_unbound_model() {
        let key = api_proxy_key(vec!["gpt-5.5"], vec![], vec![]);
        let payload = json!({ "model": "gpt-5.4" });

        let error = ensure_api_proxy_key_allows_payload(&key, &payload)
            .expect_err("unbound model should be rejected");

        assert!(error.contains("gpt-5.4"));
    }

    #[test]
    fn api_proxy_key_validation_accepts_fast_service_tier_alias() {
        let key = api_proxy_key(vec![], vec![], vec!["fast"]);
        let payload = json!({
            "model": "gpt-5.5",
            "reasoning": { "effort": "medium" },
            "service_tier": "priority"
        });

        assert!(ensure_api_proxy_key_allows_payload(&key, &payload).is_ok());
    }

    #[test]
    fn api_proxy_key_validation_rejects_unbound_reasoning_effort() {
        let key = api_proxy_key(vec![], vec!["low"], vec![]);
        let payload = json!({
            "model": "gpt-5.5",
            "reasoning": { "effort": "high" }
        });

        let error = ensure_api_proxy_key_allows_payload(&key, &payload)
            .expect_err("unbound reasoning effort should be rejected");

        assert!(error.contains("high"));
    }

    #[tokio::test]
    async fn api_proxy_usage_migrates_once_keeps_history_and_only_clears_manually() {
        let (storage, data_dir) = temp_proxy_storage_context("api-usage-migration");
        let now = now_unix_seconds();
        let old_timestamp = now - 60 * 24 * 60 * 60;
        let database_path = data_dir.join(API_PROXY_USAGE_DATABASE_FILE_NAME);
        {
            let legacy_database = rusqlite::Connection::open(&database_path)
                .expect("legacy usage database should be creatable");
            legacy_database
                .execute_batch(
                    "CREATE TABLE api_proxy_usage_events (
                        id INTEGER PRIMARY KEY,
                        timestamp INTEGER NOT NULL,
                        key_id TEXT,
                        key_label TEXT,
                        model TEXT NOT NULL,
                        route TEXT,
                        reasoning_effort TEXT,
                        service_tier TEXT,
                        calls INTEGER NOT NULL,
                        tokens INTEGER NOT NULL
                    );",
                )
                .expect("pre-breakdown usage schema should be creatable");
        }
        let legacy = serde_json::to_string_pretty(&json!({
            "version": 1,
            "updatedAt": now,
            "events": [
                {
                    "timestamp": old_timestamp,
                    "keyId": "key-old",
                    "keyLabel": "Old tenant",
                    "accountKey": "must-not-migrate",
                    "model": "gpt-5.4",
                    "route": "/v1/responses",
                    "calls": 1,
                    "tokens": 0
                },
                {
                    "timestamp": now,
                    "keyId": "key-new",
                    "keyLabel": "New tenant",
                    "model": "gpt-5.4",
                    "route": "/v1/responses",
                    "calls": 1,
                    "tokens": 12
                }
            ]
        }))
        .expect("legacy usage should serialize");
        let legacy_path = data_dir.join(API_PROXY_USAGE_FILE_NAME);
        std::fs::write(&legacy_path, &legacy).expect("legacy usage should be writable");

        let stats = get_api_proxy_usage_stats_with_storage(
            &storage,
            Some(API_PROXY_USAGE_RANGE_30D_SECONDS),
        )
        .await
        .expect("migrated usage should be queryable");
        assert_eq!(
            stats
                .series
                .iter()
                .map(|series| series.total_calls)
                .sum::<i64>(),
            1
        );

        let logs = get_api_proxy_key_usage_logs_with_storage(&storage, Some(10))
            .await
            .expect("migrated key logs should be queryable");
        assert_eq!(logs.len(), 2, "old usage is retained until manual clear");
        assert_eq!(
            std::fs::read_to_string(&legacy_path).expect("legacy file remains readable"),
            legacy,
            "migration must not overwrite the legacy JSON"
        );

        let connection =
            rusqlite::Connection::open(&database_path).expect("usage database should be readable");
        let migrated_columns = {
            let mut statement = connection
                .prepare("PRAGMA table_info(api_proxy_usage_events)")
                .expect("usage columns should be inspectable");
            statement
                .query_map([], |row| row.get::<_, String>(1))
                .expect("usage columns should be queryable")
                .collect::<Result<std::collections::HashSet<_>, _>>()
                .expect("usage columns should be readable")
        };
        assert!(migrated_columns.contains("input_tokens"));
        assert!(migrated_columns.contains("output_tokens"));
        assert!(migrated_columns.contains("cached_input_tokens"));
        let count = || {
            connection
                .query_row("SELECT COUNT(*) FROM api_proxy_usage_events", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("usage rows should be countable")
        };
        assert_eq!(count(), 2);
        get_api_proxy_usage_stats_with_storage(&storage, None)
            .await
            .expect("reopening usage must not duplicate migration");
        assert_eq!(count(), 2);

        append_api_proxy_usage_event(
            &storage,
            ApiProxyUsageEvent {
                timestamp: now,
                key_id: Some("key-new".to_string()),
                key_label: Some("=Renamed, \"tenant\"\nrow".to_string()),
                model: "gpt-5.5".to_string(),
                route: Some("/v1/responses".to_string()),
                reasoning_effort: Some("high".to_string()),
                service_tier: Some("fast".to_string()),
                calls: 0,
                tokens: 5,
                input_tokens: Some(3),
                output_tokens: Some(2),
                cached_input_tokens: Some(1),
            },
        )
        .await
        .expect("latest usage should be queued before export");
        append_api_proxy_usage_event(
            &storage,
            ApiProxyUsageEvent {
                timestamp: now,
                key_id: Some("key-other".to_string()),
                key_label: Some("Other tenant".to_string()),
                model: "gpt-5.4".to_string(),
                route: Some("/v1/responses".to_string()),
                reasoning_effort: None,
                service_tier: None,
                calls: 1,
                tokens: 0,
                input_tokens: None,
                output_tokens: None,
                cached_input_tokens: None,
            },
        )
        .await
        .expect("second usage should be queued before export");

        let all_export_path = data_dir.join("all-usage.csv");
        export_api_proxy_usage_with_storage(
            &storage,
            Some(API_PROXY_USAGE_RANGE_30D_SECONDS),
            None,
            all_export_path.clone(),
        )
        .await
        .expect("all-key CSV export should succeed");
        let all_csv = std::fs::read_to_string(&all_export_path)
            .expect("all-key CSV export should be readable");
        assert!(all_csv.starts_with('\u{feff}'));
        assert!(all_csv.contains("\"event_type\""));
        assert!(all_csv.contains(
            "\"input_tokens\",\"output_tokens\",\"cached_input_tokens\",\"total_tokens\""
        ));
        assert!(all_csv.contains("\"high\",\"fast\",\"0\",\"3\",\"2\",\"1\",\"5\"\r\n"));
        assert!(all_csv.contains("\"key-other\""));
        assert!(all_csv.contains("\"summary_range_key_model\",\"20"));
        assert_eq!(
            all_csv
                .matches("\"'=Renamed, \"\"tenant\"\"\nrow\"")
                .count(),
            3,
            "detail and both model summaries must use the latest Key label"
        );

        let filtered_export_path = data_dir.join("filtered-usage.csv");
        export_api_proxy_usage_with_storage(
            &storage,
            Some(API_PROXY_USAGE_RANGE_30D_SECONDS),
            Some("key-new".to_string()),
            filtered_export_path.clone(),
        )
        .await
        .expect("per-key CSV export should succeed");
        let filtered_csv = std::fs::read_to_string(&filtered_export_path)
            .expect("per-key CSV export should be readable");
        assert!(filtered_csv.contains("\"key-new\""));
        assert!(!filtered_csv.contains("\"key-other\""));

        let empty_export_path = data_dir.join("empty-usage.csv");
        export_api_proxy_usage_with_storage(
            &storage,
            Some(API_PROXY_USAGE_RANGE_30D_SECONDS),
            Some("missing-key".to_string()),
            empty_export_path.clone(),
        )
        .await
        .expect("empty CSV export should still succeed");
        let empty_csv = std::fs::read_to_string(&empty_export_path)
            .expect("empty CSV export should be readable");
        assert_eq!(empty_csv.lines().count(), 1);

        clear_api_proxy_usage_stats_with_storage(&storage)
            .await
            .expect("manual clear should succeed");
        assert_eq!(count(), 0);
        assert!(
            !legacy_path.exists(),
            "successful manual clear must remove the legacy JSON copy"
        );

        drop(connection);
        drop(storage);
        let reopened = super::open_api_proxy_usage_database(&data_dir)
            .expect("cleared usage database should reopen without reimporting history");
        let reopened_count = reopened
            .query_row("SELECT COUNT(*) FROM api_proxy_usage_events", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("reopened usage rows should be countable");
        assert_eq!(reopened_count, 0);
        drop(reopened);
        std::fs::remove_dir_all(data_dir).expect("temporary proxy storage should be removable");
    }

    #[test]
    fn average_load_balance_preserves_free_plan_then_usage_order() {
        let candidates = vec![
            proxy_candidate_with_plan("free weekly 95", "e", Some(95.0), Some(95.0), false, "free"),
            proxy_candidate("weekly 20", "a", Some(20.0), Some(5.0), false),
            proxy_candidate("weekly 10 five 90", "b", Some(10.0), Some(90.0), false),
            proxy_candidate("weekly 10 five 5", "c", Some(10.0), Some(5.0), false),
            proxy_candidate("blocked low usage", "d", Some(0.0), Some(0.0), true),
        ];

        let ordered = order_proxy_candidates_for_request(
            candidates,
            load_balance_config(ApiProxyLoadBalanceMode::Average, 80.0),
            None,
            None,
        );

        assert_eq!(
            candidate_labels(&ordered),
            vec![
                "free weekly 95",
                "weekly 10 five 5",
                "weekly 10 five 90",
                "weekly 20",
                "blocked low usage",
            ]
        );
    }

    #[test]
    fn sequential_load_balance_reuses_current_candidate_under_limit() {
        let candidates = vec![
            proxy_candidate("smart best", "a", Some(5.0), Some(10.0), false),
            proxy_candidate("current", "b", Some(50.0), Some(70.0), false),
        ];

        let ordered = order_proxy_candidates_for_request(
            candidates,
            load_balance_config(ApiProxyLoadBalanceMode::Sequential, 80.0),
            Some("b"),
            None,
        );

        assert_eq!(candidate_labels(&ordered), vec!["current", "smart best"]);
    }

    #[test]
    fn sequential_load_balance_reuses_persisted_candidate_after_restart() {
        let candidates = vec![
            proxy_candidate("smart best", "a", Some(5.0), Some(10.0), false),
            proxy_candidate("persisted current", "b", Some(50.0), Some(70.0), false),
        ];
        let current_key = sequential_account_key_for_request(None, Some("b".to_string()));

        let ordered = order_proxy_candidates_for_request(
            candidates,
            load_balance_config(ApiProxyLoadBalanceMode::Sequential, 80.0),
            current_key.as_deref(),
            None,
        );

        assert_eq!(
            candidate_labels(&ordered),
            vec!["persisted current", "smart best"]
        );
    }

    #[test]
    fn sequential_load_balance_runtime_candidate_overrides_persisted_candidate() {
        let current_key = sequential_account_key_for_request(
            Some("runtime".to_string()),
            Some("persisted".to_string()),
        );

        assert_eq!(current_key.as_deref(), Some("runtime"));
    }

    #[test]
    fn sequential_runtime_affinity_keeps_sessions_independent() {
        let mut snapshot = ApiProxyRuntimeSnapshot {
            sequential_account_key: Some("global".to_string()),
            ..ApiProxyRuntimeSnapshot::default()
        };
        snapshot.sequential_session_affinity.insert(
            "header:x-codex-tools-session:session-a".to_string(),
            ApiProxySessionAffinity {
                account_key: "account-a".to_string(),
                updated_at: super::now_unix_seconds(),
            },
        );
        snapshot.sequential_session_affinity.insert(
            "header:x-codex-tools-session:session-b".to_string(),
            ApiProxySessionAffinity {
                account_key: "account-b".to_string(),
                updated_at: super::now_unix_seconds(),
            },
        );

        assert_eq!(
            sequential_runtime_account_key(
                &snapshot,
                Some("header:x-codex-tools-session:session-a")
            )
            .as_deref(),
            Some("account-a")
        );
        assert_eq!(
            sequential_runtime_account_key(
                &snapshot,
                Some("header:x-codex-tools-session:session-b")
            )
            .as_deref(),
            Some("account-b")
        );
        assert_eq!(
            sequential_runtime_account_key(&snapshot, Some("missing-session")),
            None
        );
        assert_eq!(
            sequential_runtime_account_key(&snapshot, None).as_deref(),
            Some("global")
        );
    }

    #[test]
    fn sequential_runtime_account_key_expires_stale_bindings() {
        let mut snapshot = ApiProxyRuntimeSnapshot::default();
        snapshot.sequential_session_affinity.insert(
            "header:x-codex-tools-session:stale".to_string(),
            ApiProxySessionAffinity {
                account_key: "account-stale".to_string(),
                updated_at: super::now_unix_seconds() - super::PROXY_SESSION_AFFINITY_TTL_SECS - 1,
            },
        );
        snapshot.sequential_session_affinity.insert(
            "header:x-codex-tools-session:fresh".to_string(),
            ApiProxySessionAffinity {
                account_key: "account-fresh".to_string(),
                updated_at: super::now_unix_seconds(),
            },
        );

        assert_eq!(
            sequential_runtime_account_key(&snapshot, Some("header:x-codex-tools-session:stale")),
            None
        );
        assert_eq!(
            sequential_runtime_account_key(&snapshot, Some("header:x-codex-tools-session:fresh"))
                .as_deref(),
            Some("account-fresh")
        );
    }

    #[test]
    fn average_mode_prefers_healthy_session_binding_over_usage_order() {
        let candidates = vec![
            proxy_candidate("least used", "a", Some(5.0), Some(5.0), false),
            proxy_candidate("bound", "b", Some(50.0), Some(60.0), false),
            proxy_candidate("most used", "c", Some(90.0), Some(90.0), false),
        ];

        let ordered = order_proxy_candidates_for_request(
            candidates,
            load_balance_config(ApiProxyLoadBalanceMode::Average, 80.0),
            None,
            Some("b"),
        );

        assert_eq!(
            candidate_labels(&ordered),
            vec!["bound", "least used", "most used"]
        );
    }

    #[test]
    fn average_mode_falls_back_to_usage_order_when_binding_degrades() {
        let candidates = vec![
            proxy_candidate("least used", "a", Some(5.0), Some(5.0), false),
            proxy_candidate("bound blocked", "b", Some(10.0), Some(10.0), true),
        ];

        let ordered = order_proxy_candidates_for_request(
            candidates,
            load_balance_config(ApiProxyLoadBalanceMode::Average, 80.0),
            None,
            Some("b"),
        );

        assert_eq!(
            candidate_labels(&ordered),
            vec!["least used", "bound blocked"]
        );
    }

    #[test]
    fn average_mode_without_binding_keeps_usage_order() {
        let candidates = vec![
            proxy_candidate("least used", "a", Some(5.0), Some(5.0), false),
            proxy_candidate("more used", "b", Some(50.0), Some(60.0), false),
        ];

        let ordered = order_proxy_candidates_for_request(
            candidates,
            load_balance_config(ApiProxyLoadBalanceMode::Average, 80.0),
            None,
            None,
        );

        assert_eq!(candidate_labels(&ordered), vec!["least used", "more used"]);
    }

    #[test]
    fn request_session_affinity_key_reads_claude_code_identity() {
        let headers = HeaderMap::new();
        let from_header = {
            let mut headers = headers.clone();
            headers.insert(
                "x-claude-code-session-id",
                HeaderValue::from_static(" cc-session-1 "),
            );
            request_session_affinity_key(&headers, &json!({}))
        };
        assert_eq!(
            from_header.as_deref(),
            Some("header:x-claude-code-session-id:cc-session-1")
        );

        let from_metadata = request_session_affinity_key(
            &headers,
            &json!({
                "metadata": {
                    "user_id": "{\"session_id\":\"cc-session-2\"}"
                }
            }),
        );
        assert_eq!(
            from_metadata.as_deref(),
            Some("metadata:user_id:cc-session-2")
        );
    }

    #[test]
    fn request_session_affinity_key_prefers_header_over_payload() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-codex-tools-session",
            HeaderValue::from_static(" header-session "),
        );
        let payload = json!({
            "sessionId": "payload-session",
        });

        let key = request_session_affinity_key(&headers, &payload);

        assert_eq!(
            key.as_deref(),
            Some("header:x-codex-tools-session:header-session")
        );
    }

    #[test]
    fn request_session_affinity_key_uses_payload_when_header_is_missing() {
        let headers = HeaderMap::new();
        let payload = json!({
            "conversationId": "conversation-1",
        });

        let key = request_session_affinity_key(&headers, &payload);

        assert_eq!(
            key.as_deref(),
            Some("payload:conversationId:conversation-1")
        );
    }

    #[test]
    fn sequential_load_balance_reuses_current_candidate_with_missing_five_hour_usage() {
        let candidates = vec![
            proxy_candidate("smart best", "a", Some(5.0), Some(10.0), false),
            proxy_candidate("current missing 5h", "b", Some(50.0), None, false),
        ];

        let ordered = order_proxy_candidates_for_request(
            candidates,
            load_balance_config(ApiProxyLoadBalanceMode::Sequential, 80.0),
            Some("b"),
            None,
        );

        assert_eq!(
            candidate_labels(&ordered),
            vec!["current missing 5h", "smart best"]
        );
    }

    #[test]
    fn sequential_load_balance_keeps_current_first_when_all_five_hour_usage_is_missing() {
        let candidates = vec![
            proxy_candidate("missing a", "a", Some(5.0), None, false),
            proxy_candidate("current missing", "b", Some(50.0), None, false),
            proxy_candidate("missing c", "c", Some(10.0), None, false),
        ];

        let ordered = order_proxy_candidates_for_request(
            candidates,
            load_balance_config(ApiProxyLoadBalanceMode::Sequential, 80.0),
            Some("b"),
            None,
        );

        assert_eq!(candidate_labels(&ordered)[0], "current missing");
    }

    #[test]
    fn sequential_load_balance_switches_when_current_reaches_limit() {
        let candidates = vec![
            proxy_candidate("current at limit", "a", Some(1.0), Some(80.0), false),
            proxy_candidate("next under limit", "b", Some(20.0), Some(10.0), false),
            proxy_candidate("blocked", "c", Some(0.0), Some(0.0), true),
        ];

        let ordered = order_proxy_candidates_for_request(
            candidates,
            load_balance_config(ApiProxyLoadBalanceMode::Sequential, 80.0),
            Some("a"),
            None,
        );

        assert_eq!(
            candidate_labels(&ordered),
            vec!["next under limit", "current at limit", "blocked"]
        );
    }

    #[test]
    fn converts_chat_request_to_codex_payload() {
        let request = json!({
            "model": "gpt-5.4",
            "stream": false,
            "messages": [
                { "role": "system", "content": "You are terse." },
                { "role": "user", "content": "1+1 等于几？" }
            ]
        });

        let (payload, downstream_stream) =
            convert_openai_chat_request_to_codex(&request).expect("payload should convert");

        assert!(!downstream_stream);
        assert_eq!(
            payload.get("model").and_then(|value| value.as_str()),
            Some("gpt-5.4")
        );
        assert_eq!(
            payload.get("stream").and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            payload.get("store").and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            payload
                .get("input")
                .and_then(|value| value.as_array())
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(
            payload
                .get("input")
                .and_then(|value| value.get(0))
                .and_then(|value| value.get("role"))
                .and_then(|value| value.as_str()),
            Some("developer")
        );
    }

    #[test]
    fn defaults_openai_requests_to_gpt_5_6_sol_xhigh_standard_responses_lite() {
        let request = json!({
            "messages": [{ "role": "user", "content": "hello" }],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "lookup",
                    "parameters": { "type": "object" }
                }
            }]
        });

        let (payload, _) =
            convert_openai_chat_request_to_codex(&request).expect("request should convert");

        assert_eq!(
            payload.get("model").and_then(Value::as_str),
            Some("gpt-5.6-sol")
        );
        assert_eq!(
            payload
                .get("reasoning")
                .and_then(|value| value.get("effort"))
                .and_then(Value::as_str),
            Some("xhigh")
        );
        assert_eq!(
            payload.get("service_tier").and_then(Value::as_str),
            Some("default")
        );
        assert_eq!(
            payload.get("parallel_tool_calls").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            payload.get("tool_choice").and_then(Value::as_str),
            Some("auto")
        );
        assert!(payload.get("instructions").is_none());
        assert!(payload.get("tools").is_none());
        assert_eq!(
            payload
                .get("input")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("type"))
                .and_then(Value::as_str),
            Some("additional_tools")
        );
        assert_eq!(
            payload
                .get("reasoning")
                .and_then(|value| value.get("context"))
                .and_then(Value::as_str),
            Some("all_turns")
        );

        let (responses_payload, _) = normalize_openai_responses_request(json!({
            "input": "hello"
        }))
        .expect("Responses request should use defaults");
        assert_eq!(
            responses_payload.get("model").and_then(Value::as_str),
            Some("gpt-5.6-sol")
        );
        assert_eq!(
            responses_payload
                .get("reasoning")
                .and_then(|value| value.get("effort"))
                .and_then(Value::as_str),
            Some("xhigh")
        );
        assert_eq!(
            responses_payload
                .get("service_tier")
                .and_then(Value::as_str),
            Some("default")
        );
    }

    #[test]
    fn maps_all_gpt_5_6_model_names_and_aliases() {
        let cases = [
            ("gpt-5.6", "gpt-5.6-sol"),
            ("gpt5.6", "gpt-5.6-sol"),
            ("gpt-5-6", "gpt-5.6-sol"),
            ("gpt-5.6-sol", "gpt-5.6-sol"),
            ("gpt5.6-sol", "gpt-5.6-sol"),
            ("gpt-5-6-sol", "gpt-5.6-sol"),
            ("gpt-5.6-terra", "gpt-5.6-terra"),
            ("gpt5.6-terra", "gpt-5.6-terra"),
            ("gpt-5-6-terra", "gpt-5.6-terra"),
            ("gpt-5.6-luna", "gpt-5.6-luna"),
            ("gpt5.6-luna", "gpt-5.6-luna"),
            ("gpt-5-6-luna", "gpt-5.6-luna"),
            ("gpt5.6-sol-2026-07-01", "gpt-5.6-sol-2026-07-01"),
            ("gpt-5-6-terra-2026-07-01", "gpt-5.6-terra-2026-07-01"),
            ("gpt-5.6-luna-2026-07-01", "gpt-5.6-luna-2026-07-01"),
        ];

        for (requested, expected) in cases {
            let request = json!({
                "model": requested,
                "input": "hello"
            });
            let (payload, _) = normalize_openai_responses_request(request)
                .expect("GPT-5.6 request should normalize");
            assert_eq!(
                payload.get("model").and_then(Value::as_str),
                Some(expected),
                "alias {requested}"
            );
        }
    }

    #[test]
    fn forwards_every_reasoning_effort_and_maps_ultra_to_max() {
        for (requested, expected) in [
            ("none", "none"),
            ("low", "low"),
            ("medium", "medium"),
            ("high", "high"),
            ("xhigh", "xhigh"),
            ("max", "max"),
            ("ultra", "max"),
        ] {
            let request = json!({
                "model": "gpt-5.6-sol",
                "input": "hello",
                "reasoning": { "effort": requested }
            });
            let (payload, _) = normalize_openai_responses_request(request)
                .expect("reasoning effort should normalize");
            assert_eq!(
                payload
                    .get("reasoning")
                    .and_then(|value| value.get("effort"))
                    .and_then(Value::as_str),
                Some(expected),
                "effort {requested}"
            );
        }

        let minimal = json!({
            "model": "gpt-5.6-sol",
            "input": "hello",
            "reasoning": { "effort": "minimal" }
        });
        assert!(normalize_openai_responses_request(minimal)
            .expect_err("GPT-5.6 minimal should be rejected")
            .contains("minimal"));
    }

    #[test]
    fn anthropic_output_config_effort_maps_to_upstream() {
        for (requested, expected) in [
            ("low", "low"),
            ("medium", "medium"),
            ("high", "high"),
            ("xhigh", "xhigh"),
            ("max", "max"),
        ] {
            let request = json!({
                "model": "gpt-5.6-terra",
                "output_config": { "effort": requested },
                "thinking": { "type": "adaptive" },
                "messages": [{ "role": "user", "content": "hello" }]
            });
            let (payload, _) = convert_anthropic_messages_request_to_codex(&request)
                .expect("Anthropic request should convert");
            assert_eq!(
                payload
                    .get("reasoning")
                    .and_then(|value| value.get("effort"))
                    .and_then(Value::as_str),
                Some(expected),
                "output_config effort {requested}"
            );
        }
    }

    #[test]
    fn anthropic_explicit_reasoning_overrides_output_config() {
        let request = json!({
            "model": "gpt-5.4",
            "output_config": { "effort": "low" },
            "reasoning": { "effort": "high" },
            "messages": [{ "role": "user", "content": "hello" }]
        });
        let (payload, _) = convert_anthropic_messages_request_to_codex(&request)
            .expect("Anthropic request should convert");
        assert_eq!(
            payload
                .get("reasoning")
                .and_then(|value| value.get("effort"))
                .and_then(Value::as_str),
            Some("high")
        );
    }

    #[test]
    fn anthropic_output_config_invalid_effort_falls_back_to_default() {
        let request = json!({
            "model": "gpt-5.4",
            "output_config": { "effort": "turbo" },
            "messages": [{ "role": "user", "content": "hello" }]
        });
        let (payload, _) = convert_anthropic_messages_request_to_codex(&request)
            .expect("Anthropic request should convert");
        assert_eq!(
            payload
                .get("reasoning")
                .and_then(|value| value.get("effort"))
                .and_then(Value::as_str),
            Some(DEFAULT_API_PROXY_REASONING_EFFORT)
        );
    }

    #[test]
    fn resolved_proxy_default_service_tier_uses_env_else_compiled_default() {
        let cases = [
            (Some("fast"), "priority"),
            (Some("default"), DEFAULT_UPSTREAM_SERVICE_TIER),
            (Some("bogus"), DEFAULT_UPSTREAM_SERVICE_TIER),
            (None, DEFAULT_UPSTREAM_SERVICE_TIER),
        ];
        for (env_value, expected) in cases {
            assert_eq!(
                resolved_proxy_default_service_tier(env_value),
                expected,
                "env={env_value:?}"
            );
        }
    }

    #[test]
    fn apply_configured_default_service_tier_respects_explicit_request() {
        let default_tier = "priority";

        // Request without an explicit tier receives the configured default.
        let (mut payload, _) = convert_anthropic_messages_request_to_codex(&json!({
            "model": "gpt-5.4",
            "messages": [{ "role": "user", "content": "hello" }]
        }))
        .expect("Anthropic request should convert");
        apply_configured_default_service_tier(false, &mut payload, default_tier);
        assert_eq!(
            payload.get("service_tier").and_then(Value::as_str),
            Some(default_tier)
        );

        // Explicit tier (already normalized, e.g. fast -> priority) is kept.
        let (mut payload, _) = convert_anthropic_messages_request_to_codex(&json!({
            "model": "gpt-5.4",
            "service_tier": "fast",
            "messages": [{ "role": "user", "content": "hello" }]
        }))
        .expect("Anthropic request should convert");
        apply_configured_default_service_tier(true, &mut payload, default_tier);
        assert_eq!(
            payload.get("service_tier").and_then(Value::as_str),
            Some("priority")
        );
    }

    #[test]
    fn forwards_every_service_tier_and_alias() {
        for (requested, payload_tier, policy_tier, upstream_tier) in [
            ("", "default", "default", None),
            ("auto", "auto", "auto", Some("auto")),
            ("default", "default", "default", None),
            ("standard", "default", "default", None),
            ("fast", "priority", "fast", Some("priority")),
            ("priority", "priority", "fast", Some("priority")),
            ("flex", "flex", "flex", Some("flex")),
        ] {
            let request = json!({
                "model": "gpt-5.6-sol",
                "input": "hello",
                "service_tier": requested
            });
            let (payload, _) =
                normalize_openai_responses_request(request).expect("service tier should normalize");
            assert_eq!(
                payload.get("service_tier").and_then(Value::as_str),
                Some(payload_tier),
                "payload tier {requested}"
            );
            assert_eq!(
                super::api_proxy_service_tier_from_payload(&payload).as_deref(),
                Some(policy_tier),
                "policy tier {requested}"
            );
            assert_eq!(
                super::payload_for_upstream(&payload)
                    .get("service_tier")
                    .and_then(Value::as_str),
                upstream_tier,
                "upstream tier {requested}"
            );
        }
    }

    #[test]
    fn converts_gpt_5_6_responses_payload_to_lite_contract() {
        let request = json!({
            "model": "gpt-5.6-terra",
            "instructions": "Be exact.",
            "input": "hello",
            "tools": [{ "type": "function", "name": "lookup", "parameters": {} }],
            "parallel_tool_calls": true,
            "reasoning": { "effort": "max" },
            "service_tier": "fast"
        });

        let (payload, _) =
            normalize_openai_responses_request(request).expect("request should normalize");
        let input = payload
            .get("input")
            .and_then(Value::as_array)
            .expect("lite input should be an array");

        assert_eq!(
            input[0].get("type").and_then(Value::as_str),
            Some("additional_tools")
        );
        assert_eq!(
            input[0].get("role").and_then(Value::as_str),
            Some("developer")
        );
        assert_eq!(
            input[1].get("role").and_then(Value::as_str),
            Some("developer")
        );
        assert_eq!(input[2].get("role").and_then(Value::as_str), Some("user"));
        assert!(payload.get("instructions").is_none());
        assert!(payload.get("tools").is_none());
        assert_eq!(
            payload.get("parallel_tool_calls").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            payload.get("service_tier").and_then(Value::as_str),
            Some("priority")
        );
        assert_eq!(
            payload
                .get("reasoning")
                .and_then(|value| value.get("context"))
                .and_then(Value::as_str),
            Some("all_turns")
        );
    }

    #[test]
    fn preserves_existing_responses_lite_prefix_without_duplication() {
        let request = json!({
            "model": "gpt-5.6-sol",
            "input": [
                { "type": "additional_tools", "role": "developer", "tools": [] },
                { "type": "message", "role": "user", "content": [] }
            ]
        });

        let (payload, _) =
            normalize_openai_responses_request(request).expect("request should normalize");
        let input = payload
            .get("input")
            .and_then(Value::as_array)
            .expect("lite input should be an array");
        assert_eq!(
            input
                .iter()
                .filter(|item| item.get("type").and_then(Value::as_str) == Some("additional_tools"))
                .count(),
            1
        );
    }

    #[test]
    fn responses_lite_removes_image_detail_from_messages_and_tool_outputs() {
        let request = json!({
            "model": "gpt-5.6-sol",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_image",
                        "image_url": "data:image/png;base64,AA==",
                        "detail": "original"
                    }]
                },
                {
                    "type": "custom_tool_call_output",
                    "call_id": "call-1",
                    "output": [{
                        "type": "input_image",
                        "image_url": "data:image/png;base64,AA==",
                        "detail": "high"
                    }]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_image",
                        "image_url": "HTTPS://example.com/image.png",
                        "detail": "high"
                    }]
                },
                {
                    "type": "function_call_output",
                    "call_id": "call-2",
                    "output": [{
                        "type": "input_image",
                        "image_url": "DATA:image/png;base64,AA==",
                        "detail": "LOW"
                    }]
                }
            ]
        });

        let (payload, _) =
            normalize_openai_responses_request(request).expect("request should normalize");
        let serialized = serde_json::to_string(&payload).expect("payload should serialize");

        assert!(!serialized.contains("\"detail\""));
        assert_eq!(serialized.matches("input_image").count(), 2);
        assert!(!serialized.contains("example.com/image.png"));
        assert!(serialized
            .contains("image content omitted because remote image URLs are not supported"));
        assert!(serialized.contains("image content omitted because detail 'low' is not supported"));
    }

    #[test]
    fn responses_lite_websocket_payload_uses_client_metadata_marker() {
        let payload = json!({
            "model": "gpt-5.6-terra",
            "input": []
        });

        let payload = super::websocket_response_create_payload(&payload, Some("sticky-turn"));

        assert_eq!(
            payload
                .get("client_metadata")
                .and_then(|value| {
                    value.get("ws_request_header_x_openai_internal_codex_responses_lite")
                })
                .and_then(Value::as_str),
            Some("true")
        );
        assert_eq!(
            payload
                .get("client_metadata")
                .and_then(|value| value.get(super::CODEX_TURN_STATE_HEADER))
                .and_then(Value::as_str),
            Some("sticky-turn")
        );
    }

    #[test]
    fn anthropic_and_websocket_entries_use_responses_lite_defaults() {
        let anthropic = json!({
            "system": "Be exact.",
            "messages": [{ "role": "user", "content": "hello" }],
            "tools": [{
                "name": "lookup",
                "input_schema": { "type": "object" }
            }]
        });
        let (anthropic_payload, _) = convert_anthropic_messages_request_to_codex(&anthropic)
            .expect("Anthropic request should convert");
        assert_eq!(
            anthropic_payload.get("model").and_then(Value::as_str),
            Some("gpt-5.6-sol")
        );
        assert_eq!(
            anthropic_payload
                .get("service_tier")
                .and_then(Value::as_str),
            Some("default")
        );
        assert_eq!(
            anthropic_payload
                .get("reasoning")
                .and_then(|value| value.get("effort"))
                .and_then(Value::as_str),
            Some("xhigh")
        );
        let anthropic_input = anthropic_payload
            .get("input")
            .and_then(Value::as_array)
            .expect("Lite input should be an array");
        assert_eq!(
            anthropic_input[0].get("type").and_then(Value::as_str),
            Some("additional_tools")
        );
        assert_eq!(
            anthropic_input[1].get("role").and_then(Value::as_str),
            Some("developer")
        );

        let (websocket_payload, _) = normalize_responses_websocket_create(
            br#"{"type":"response.create","model":"gpt-5.6-luna","input":"hello"}"#,
        )
        .expect("WebSocket payload should normalize");
        assert_eq!(
            websocket_payload
                .get("input")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("type"))
                .and_then(Value::as_str),
            Some("additional_tools")
        );
        assert_eq!(
            websocket_payload
                .get("parallel_tool_calls")
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn responses_websocket_v2_requests_use_http_fallback() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "openai-beta",
            HeaderValue::from_static("other=true, responses_websockets=2026-02-06"),
        );

        assert!(super::responses_websocket_requires_http_fallback(&headers));
        assert!(!super::responses_websocket_requires_http_fallback(
            &HeaderMap::new()
        ));
    }

    #[test]
    fn responses_lite_forces_codex_0144_identity_while_classic_preserves_client_identity() {
        let mut headers = HeaderMap::new();
        headers.insert("version", HeaderValue::from_static("0.125.0"));
        headers.insert(
            "user-agent",
            HeaderValue::from_static("codex_cli_rs/0.125.0"),
        );

        assert_eq!(
            super::upstream_codex_client_identity(&headers, true),
            ("0.144.0", "codex_cli_rs/0.144.0")
        );
        assert_eq!(
            super::upstream_codex_client_identity(&headers, false),
            ("0.125.0", "codex_cli_rs/0.125.0")
        );
        assert!(super::payload_uses_responses_lite(
            &json!({ "model": "gpt-5.6-terra" })
        ));
        assert!(!super::payload_uses_responses_lite(
            &json!({ "model": "gpt-5.5" })
        ));
    }

    #[test]
    fn explicit_standard_speed_is_preserved_for_policy_and_omitted_upstream() {
        let request = json!({
            "model": "gpt-5.6-luna",
            "input": "hello",
            "service_tier": "default"
        });
        let (payload, _) =
            normalize_openai_responses_request(request).expect("request should normalize");

        assert_eq!(
            payload.get("service_tier").and_then(Value::as_str),
            Some("default")
        );
        assert!(super::payload_for_upstream(&payload)
            .get("service_tier")
            .is_none());
    }

    #[test]
    fn rejects_unknown_reasoning_effort_and_service_tier() {
        let invalid_effort = json!({
            "model": "gpt-5.6-sol",
            "input": "hello",
            "reasoning": { "effort": "future" }
        });
        let invalid_tier = json!({
            "model": "gpt-5.6-sol",
            "input": "hello",
            "service_tier": "turbo"
        });

        assert!(normalize_openai_responses_request(invalid_effort)
            .expect_err("unknown effort should fail")
            .contains("future"));
        assert!(normalize_openai_responses_request(invalid_tier)
            .expect_err("unknown tier should fail")
            .contains("turbo"));
    }

    #[test]
    fn maps_ultra_to_max_and_rejects_malformed_model_or_reasoning() {
        let ultra = json!({
            "model": "gpt-5.6-sol",
            "input": "hello",
            "reasoning": { "effort": "ultra" }
        });
        let (payload, _) =
            normalize_openai_responses_request(ultra).expect("ultra should map to max");
        assert_eq!(
            payload
                .get("reasoning")
                .and_then(|value| value.get("effort"))
                .and_then(Value::as_str),
            Some("max")
        );

        for invalid in [
            json!({ "model": 56, "input": "hello" }),
            json!({ "model": "gpt-5.6-sol", "input": "hello", "reasoning": "high" }),
            json!({ "model": "gpt-5.6-sol", "input": "hello", "reasoning": { "effort": 9 } }),
            json!({ "model": "gpt-5.6-sol", "input": {}, "reasoning": { "effort": "high" } }),
        ] {
            assert!(normalize_openai_responses_request(invalid).is_err());
        }
    }

    #[test]
    fn non_lite_models_keep_classic_responses_fields() {
        let request = json!({
            "model": "gpt-5.5",
            "instructions": "Be exact.",
            "input": "hello",
            "tools": [{ "type": "function", "name": "lookup", "parameters": {} }],
            "parallel_tool_calls": true
        });

        let (payload, _) =
            normalize_openai_responses_request(request).expect("classic request should normalize");
        assert_eq!(
            payload.get("instructions").and_then(Value::as_str),
            Some("Be exact.")
        );
        assert!(payload.get("tools").is_some_and(Value::is_array));
        assert_eq!(
            payload.get("parallel_tool_calls").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn rejects_hosted_tools_that_responses_lite_cannot_advertise() {
        for tool_type in ["web_search", "image_generation"] {
            let request = json!({
                "model": "gpt-5.6-sol",
                "input": "hello",
                "tools": [{ "type": tool_type }]
            });

            let error = normalize_openai_responses_request(request)
                .expect_err("hosted Responses tool should fail clearly");
            assert!(error.contains(tool_type));
        }
    }

    #[test]
    fn maps_chat_request_model_alias_to_upstream() {
        let request = json!({
            "model": "gpt-5-4",
            "messages": [
                { "role": "user", "content": "hello" }
            ]
        });

        let (payload, _) =
            convert_openai_chat_request_to_codex(&request).expect("payload should convert");

        assert_eq!(
            payload.get("model").and_then(|value| value.as_str()),
            Some("gpt-5.4")
        );
    }

    #[test]
    fn converts_anthropic_messages_request_to_codex_payload() {
        let request = json!({
            "model": "gpt-5-4",
            "system": "You are terse.",
            "max_tokens": 128,
            "stream": false,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        { "type": "text", "text": "look" },
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": "aGVsbG8="
                            }
                        }
                    ]
                },
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "call_123",
                            "name": "lookup",
                            "input": { "q": "codex" }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "call_123",
                            "content": "ok"
                        }
                    ]
                }
            ],
            "tools": [
                {
                    "name": "lookup",
                    "description": "search",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "q": { "type": "string" }
                        }
                    }
                }
            ],
            "tool_choice": { "type": "tool", "name": "lookup" }
        });

        let (payload, downstream_stream) =
            convert_anthropic_messages_request_to_codex(&request).expect("payload should convert");

        assert!(!downstream_stream);
        assert_eq!(
            payload.get("model").and_then(Value::as_str),
            Some("gpt-5.4")
        );
        assert_eq!(
            payload.get("instructions").and_then(Value::as_str),
            Some("You are terse.")
        );
        assert!(
            payload.get("max_output_tokens").is_none(),
            "max_tokens must not be forwarded: the Codex upstream rejects max_output_tokens"
        );
        assert_eq!(
            payload.get("input").and_then(Value::as_array).map(Vec::len),
            Some(3)
        );
        assert_eq!(
            payload
                .get("input")
                .and_then(|value| value.get(1))
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("function_call")
        );
        assert_eq!(
            payload
                .get("input")
                .and_then(|value| value.get(2))
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("function_call_output")
        );
        assert_eq!(
            payload
                .get("tools")
                .and_then(|value| value.get(0))
                .and_then(|value| value.get("parameters"))
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("object")
        );
        assert_eq!(
            payload
                .get("tool_choice")
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str),
            Some("lookup")
        );
    }

    #[test]
    fn accepts_responses_style_input_on_chat_completions_route() {
        let request = json!({
            "model": "gpt-5-4",
            "input": "hello"
        });

        let (payload, downstream_stream) =
            convert_openai_chat_request_to_codex(&request).expect("payload should convert");

        assert!(!downstream_stream);
        assert_eq!(
            payload.get("model").and_then(|value| value.as_str()),
            Some("gpt-5.4")
        );
        assert_eq!(
            payload.get("input").and_then(|value| value.as_str()),
            Some("hello")
        );
        assert_eq!(
            payload.get("stream").and_then(|value| value.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn maps_responses_request_model_alias_to_upstream() {
        let request = json!({
            "model": "gpt-5-4",
            "input": "hello"
        });

        let (payload, downstream_stream) =
            normalize_openai_responses_request(request).expect("request should normalize");

        assert!(!downstream_stream);
        assert_eq!(
            payload.get("model").and_then(|value| value.as_str()),
            Some("gpt-5.4")
        );
    }

    #[test]
    fn strips_unsupported_fields_from_responses_style_requests() {
        let request = json!({
            "model": "gpt-5-4",
            "input": "hello",
            "metadata": {
                "ide": "cursor"
            },
            "prompt_cache_retention": {
                "scope": "tool_call"
            }
        });

        let (payload, _) =
            normalize_openai_responses_request(request).expect("request should normalize");

        assert!(payload.get("metadata").is_none());
        assert!(payload.get("prompt_cache_retention").is_none());
    }

    #[test]
    fn moves_legacy_responses_reasoning_effort_into_reasoning_object() {
        let request = json!({
            "model": "gpt-5.6-sol",
            "input": "hello",
            "reasoning_effort": "high"
        });

        let (payload, _) =
            normalize_openai_responses_request(request).expect("request should normalize");

        assert!(payload.get("reasoning_effort").is_none());
        assert_eq!(
            payload
                .get("reasoning")
                .and_then(|value| value.get("effort"))
                .and_then(Value::as_str),
            Some("high")
        );
    }

    #[test]
    fn accepts_chat_request_with_official_gpt_5_4_name() {
        let request = json!({
            "model": "gpt-5.4",
            "messages": [
                { "role": "user", "content": "hello" }
            ]
        });

        let (payload, _) =
            convert_openai_chat_request_to_codex(&request).expect("request should convert");

        assert_eq!(
            payload.get("model").and_then(|value| value.as_str()),
            Some("gpt-5.4")
        );
    }

    #[test]
    fn accepts_chat_request_with_official_gpt_5_5_name() {
        let request = json!({
            "model": "gpt-5.5",
            "messages": [
                { "role": "user", "content": "hello" }
            ]
        });

        let (payload, _) =
            convert_openai_chat_request_to_codex(&request).expect("request should convert");

        assert_eq!(
            payload.get("model").and_then(|value| value.as_str()),
            Some("gpt-5.5")
        );
    }

    #[test]
    fn accepts_responses_request_with_legacy_gpt5_4_name() {
        let request = json!({
            "model": "gpt5.4",
            "input": "hello"
        });

        let (payload, _) =
            normalize_openai_responses_request(request).expect("request should normalize");

        assert_eq!(
            payload.get("model").and_then(|value| value.as_str()),
            Some("gpt-5.4")
        );
    }

    #[test]
    fn maps_responses_request_model_gpt_5_5_alias_to_upstream() {
        let request = json!({
            "model": "gpt-5-5",
            "input": "hello"
        });

        let (payload, _) =
            normalize_openai_responses_request(request).expect("request should normalize");

        assert_eq!(
            payload.get("model").and_then(|value| value.as_str()),
            Some("gpt-5.5")
        );
    }

    #[test]
    fn accepts_responses_request_with_legacy_gpt5_5_name() {
        let request = json!({
            "model": "gpt5.5",
            "input": "hello"
        });

        let (payload, _) =
            normalize_openai_responses_request(request).expect("request should normalize");

        assert_eq!(
            payload.get("model").and_then(|value| value.as_str()),
            Some("gpt-5.5")
        );
    }

    #[test]
    fn keeps_gpt_5_5_on_http_sse_upstream() {
        let payload = json!({
            "model": "gpt-5.5",
            "input": "hello",
            "stream": true
        });

        assert!(!should_use_responses_websocket(&payload));
    }

    #[test]
    fn normalizes_websocket_response_create_to_streaming_payload() {
        let request = json!({
            "type": "response.create",
            "model": "gpt-5-5",
            "input": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "input_text",
                            "text": "Say OK only."
                        }
                    ]
                }
            ],
            "stream": false
        });
        let bytes = serde_json::to_vec(&request).expect("serialize request");

        let (payload, request_specifies_tier) = normalize_responses_websocket_create(&bytes)
            .expect("websocket payload should normalize");

        assert_eq!(
            payload.get("type").and_then(Value::as_str),
            None,
            "transport wrapper type should not be sent upstream"
        );
        assert_eq!(
            payload.get("model").and_then(Value::as_str),
            Some("gpt-5.5")
        );
        assert_eq!(payload.get("stream").and_then(Value::as_bool), Some(true));
        assert_eq!(payload.get("store").and_then(Value::as_bool), Some(false));
        assert!(!request_specifies_tier);
    }

    #[test]
    fn rejects_unexpected_websocket_event_type() {
        let request = json!({
            "type": "session.update",
            "model": "gpt-5.5",
            "input": []
        });
        let bytes = serde_json::to_vec(&request).expect("serialize request");

        let error = normalize_responses_websocket_create(&bytes).expect_err("wrong type rejected");

        assert!(error.contains("response.create"));
    }

    #[test]
    fn recognizes_responses_terminal_events_from_sse_payload() {
        let event = SseEvent {
            event: Some("response.completed".to_string()),
            data: "{\"type\":\"response.completed\"}".to_string(),
        };

        assert!(is_responses_terminal_event(&event));
    }

    #[test]
    fn converts_image_generation_request_to_responses_image_tool_payload() {
        let request = json!({
            "model": "gpt-image-2",
            "prompt": "Draw a tiny red square icon.",
            "size": "1024x1024",
            "quality": "low",
            "output_format": "png",
            "stream": true,
            "partial_images": 1
        });

        let converted = convert_openai_image_generation_request_to_codex(&request)
            .expect("image generation request should convert");
        let payload = converted.upstream_payload;

        assert!(converted.downstream_stream);
        assert_eq!(converted.image_count, 1);
        assert_eq!(
            payload.get("model").and_then(Value::as_str),
            Some("gpt-5.5")
        );
        assert_eq!(payload.get("stream").and_then(Value::as_bool), Some(true));
        assert_eq!(
            payload
                .get("input")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("content"))
                .and_then(Value::as_array)
                .and_then(|parts| parts.first())
                .and_then(|part| part.get("text"))
                .and_then(Value::as_str),
            Some("Draw a tiny red square icon.")
        );
        assert_eq!(
            payload
                .get("tools")
                .and_then(Value::as_array)
                .and_then(|tools| tools.first())
                .and_then(|tool| tool.get("type"))
                .and_then(Value::as_str),
            Some("image_generation")
        );
        assert_eq!(
            payload
                .get("tools")
                .and_then(Value::as_array)
                .and_then(|tools| tools.first())
                .and_then(|tool| tool.get("model"))
                .and_then(Value::as_str),
            Some("gpt-image-2")
        );
    }

    #[test]
    fn rejects_image_generation_url_response_format() {
        let request = json!({
            "model": "gpt-image-2",
            "prompt": "Draw a tiny red square icon.",
            "response_format": "url"
        });

        let error = convert_openai_image_generation_request_to_codex(&request)
            .expect_err("url response format should be rejected");

        assert!(error.contains("b64_json"));
    }

    #[test]
    fn converts_image_n_to_downstream_repeat_count_not_upstream_tool_field() {
        let request = json!({
            "model": "gpt-image-2",
            "prompt": "Draw tiny icons.",
            "n": 2
        });

        let converted = convert_openai_image_generation_request_to_codex(&request)
            .expect("image generation request should convert");
        let tool = converted
            .upstream_payload
            .get("tools")
            .and_then(Value::as_array)
            .and_then(|tools| tools.first())
            .expect("image_generation tool");

        assert_eq!(converted.image_count, 2);
        assert!(tool.get("n").is_none());
    }

    #[test]
    fn converts_completed_image_generation_output_to_images_response() {
        let response = json!({
            "created_at": 1777445179i64,
            "output": [
                {
                    "type": "image_generation_call",
                    "status": "completed",
                    "revised_prompt": "A tiny red square icon.",
                    "result": "iVBORw0KGgo="
                }
            ]
        });

        let converted = convert_responses_image_output_to_images_response(&response)
            .expect("image output should convert");

        assert_eq!(
            converted.get("created").and_then(Value::as_i64),
            Some(1777445179)
        );
        assert_eq!(
            converted
                .get("data")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("b64_json"))
                .and_then(Value::as_str),
            Some("iVBORw0KGgo=")
        );
        assert_eq!(
            converted
                .get("data")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("revised_prompt"))
                .and_then(Value::as_str),
            Some("A tiny red square icon.")
        );
    }

    #[test]
    fn extracts_image_generation_output_item_from_sse_when_completed_output_is_empty() {
        let body = format!(
            "event: response.output_item.done\ndata: {}\n\nevent: response.completed\ndata: {}\n\n",
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "image_generation_call",
                    "status": "completed",
                    "result": "iVBORw0KGgo="
                }
            }),
            json!({
                "type": "response.completed",
                "response": {
                    "created_at": 1777445179i64,
                    "output": []
                }
            })
        );

        let completed = extract_completed_response_from_sse(body.as_bytes())
            .expect("response.completed should be extracted");
        let converted = convert_responses_image_output_to_images_response(&completed)
            .expect("image output should convert");

        assert_eq!(
            converted
                .get("data")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("b64_json"))
                .and_then(Value::as_str),
            Some("iVBORw0KGgo=")
        );
    }

    #[test]
    fn converts_image_edit_request_with_input_image_and_mask() {
        let mut fields = serde_json::Map::new();
        fields.insert(
            "prompt".to_string(),
            Value::String("Make this icon blue".to_string()),
        );
        fields.insert(
            "model".to_string(),
            Value::String("gpt-image-2".to_string()),
        );
        fields.insert("stream".to_string(), Value::String("true".to_string()));
        fields.insert("partial_images".to_string(), Value::String("1".to_string()));
        let request = ImageMultipartRequest {
            fields,
            images: vec![json!({
                "type": "input_image",
                "image_url": "data:image/png;base64,AAAA"
            })],
            mask: Some(json!({
                "type": "input_image",
                "image_url": "data:image/png;base64,BBBB"
            })),
        };

        let converted = convert_openai_image_edit_request_to_codex(&request, false)
            .expect("image edit should convert");
        let payload = converted.upstream_payload;

        assert!(converted.downstream_stream);
        assert_eq!(
            payload
                .get("tools")
                .and_then(Value::as_array)
                .and_then(|tools| tools.first())
                .and_then(|tool| tool.get("action"))
                .and_then(Value::as_str),
            Some("edit")
        );
        assert_eq!(
            payload
                .get("tools")
                .and_then(Value::as_array)
                .and_then(|tools| tools.first())
                .and_then(|tool| tool.get("partial_images"))
                .and_then(Value::as_i64),
            Some(1)
        );
        let content = payload
            .get("input")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("content"))
            .and_then(Value::as_array)
            .expect("content parts");
        assert!(content
            .iter()
            .any(|part| { part.get("type").and_then(Value::as_str) == Some("input_image") }));
        assert!(content.iter().any(|part| part
            .get("text")
            .and_then(Value::as_str)
            .map(|text| text.contains("edit mask"))
            .unwrap_or(false)));
        assert_eq!(
            content
                .iter()
                .filter(|part| { part.get("type").and_then(Value::as_str) == Some("input_image") })
                .count(),
            2
        );
    }

    #[test]
    fn converts_image_variation_request_to_edit_action_with_default_prompt() {
        let mut fields = serde_json::Map::new();
        fields.insert(
            "model".to_string(),
            Value::String("gpt-image-2".to_string()),
        );
        let request = ImageMultipartRequest {
            fields,
            images: vec![json!({
                "type": "input_image",
                "image_url": "data:image/png;base64,AAAA"
            })],
            mask: None,
        };

        let converted = convert_openai_image_edit_request_to_codex(&request, true)
            .expect("image variation should convert");
        let payload = converted.upstream_payload;

        assert_eq!(
            payload
                .get("input")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("content"))
                .and_then(Value::as_array)
                .and_then(|parts| parts.first())
                .and_then(|part| part.get("text"))
                .and_then(Value::as_str),
            Some(super::IMAGE_VARIATION_PROMPT)
        );
    }

    #[test]
    fn translates_partial_image_sse_event_to_image_stream_chunk() {
        let event = SseEvent {
            event: Some("response.image_generation_call.partial_image".to_string()),
            data: json!({
                "type": "response.image_generation_call.partial_image",
                "partial_image_b64": "iVBORw0KGgo="
            })
            .to_string(),
        };

        let chunks = translate_sse_event_to_image_chunk(&event, &mut false);

        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0].get("type").and_then(Value::as_str),
            Some("image_generation.partial_image")
        );
        assert_eq!(
            chunks[0].get("b64_json").and_then(Value::as_str),
            Some("iVBORw0KGgo=")
        );
    }

    #[test]
    fn parses_websocket_target_host_and_port() {
        assert_eq!(
            websocket_target_host_port("wss://chatgpt.com/backend-api/codex/responses"),
            Some(("chatgpt.com".to_string(), 443))
        );
        assert_eq!(
            websocket_target_host_port("ws://127.0.0.1:8787/v1/responses"),
            Some(("127.0.0.1".to_string(), 8787))
        );
    }

    #[test]
    fn parses_http_proxy_config_with_basic_auth() {
        let proxy = parse_http_proxy_config("http://user:pass@127.0.0.1:7890", "HTTPS_PROXY")
            .expect("proxy config should parse");

        assert_eq!(proxy.host, "127.0.0.1");
        assert_eq!(proxy.port, 7890);
        assert_eq!(proxy.source, "HTTPS_PROXY");
        assert_eq!(proxy.authorization.as_deref(), Some("Basic dXNlcjpwYXNz"));
    }

    #[test]
    fn rejects_unsupported_websocket_proxy_schemes() {
        assert!(parse_http_proxy_config("socks5://127.0.0.1:7890", "ALL_PROXY").is_none());
    }

    #[test]
    fn matches_no_proxy_domains_and_ips() {
        assert!(host_matches_no_proxy(
            "api.chatgpt.com",
            "localhost,.chatgpt.com"
        ));
        assert!(host_matches_no_proxy("127.0.0.1", "localhost,127.0.0.1"));
        assert!(!host_matches_no_proxy("chatgpt.com", "example.com"));
    }

    #[test]
    fn finds_http_connect_response_header_end() {
        let response = b"HTTP/1.1 200 Connection Established\r\nProxy-Agent: test\r\n\r\n";
        assert_eq!(find_http_header_end(response), Some(response.len()));
        assert_eq!(find_http_header_end(b"HTTP/1.1 200 OK\r\n"), None);
    }

    #[test]
    fn extracts_completed_response_from_sse_body() {
        let body = br#"event: response.completed
data: {"type":"response.completed","response":{"id":"resp_123","created_at":1,"model":"gpt-5.4","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"2"}]}],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}

"#;

        let response =
            extract_completed_response_from_sse(body).expect("response.completed expected");
        assert_eq!(
            response.get("id").and_then(|value| value.as_str()),
            Some("resp_123")
        );
        assert_eq!(
            response
                .get("output")
                .and_then(|value| value.get(0))
                .and_then(|value| value.get("type"))
                .and_then(|value| value.as_str()),
            Some("message")
        );
    }

    #[test]
    fn converts_completed_response_to_chat_completion() {
        let response = json!({
            "id": "resp_123",
            "created_at": 1772966030i64,
            "model": "gpt-5-2025-08-07",
            "status": "completed",
            "output": [
                {
                    "type": "reasoning",
                    "summary": [
                        { "type": "summary_text", "text": "math" }
                    ]
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        { "type": "output_text", "text": "2" }
                    ]
                }
            ],
            "usage": {
                "input_tokens": 17,
                "output_tokens": 85,
                "total_tokens": 102,
                "input_tokens_details": { "cached_tokens": 0 },
                "output_tokens_details": { "reasoning_tokens": 64 }
            }
        });

        let converted = convert_completed_response_to_chat_completion(&response);
        assert_eq!(
            converted
                .get("choices")
                .and_then(|value| value.get(0))
                .and_then(|value| value.get("message"))
                .and_then(|value| value.get("content"))
                .and_then(|value| value.as_str()),
            Some("2")
        );
        assert_eq!(
            converted
                .get("usage")
                .and_then(|value| value.get("completion_tokens"))
                .and_then(|value| value.as_i64()),
            Some(85)
        );
    }

    #[test]
    fn maps_completed_response_model_alias_back_to_client() {
        let response = json!({
            "id": "resp_123",
            "created_at": 1772966030i64,
            "model": "gpt5.4-2026-03-09",
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        { "type": "output_text", "text": "2" }
                    ]
                }
            ]
        });

        let converted = convert_completed_response_to_chat_completion(&response);

        assert_eq!(
            converted.get("model").and_then(|value| value.as_str()),
            Some("gpt-5.4-2026-03-09")
        );
    }

    #[test]
    fn maps_completed_response_gpt_5_5_model_alias_back_to_client() {
        let response = json!({
            "id": "resp_123",
            "created_at": 1772966030i64,
            "model": "gpt5.5-2026-04-23",
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        { "type": "output_text", "text": "2" }
                    ]
                }
            ]
        });

        let converted = convert_completed_response_to_chat_completion(&response);

        assert_eq!(
            converted.get("model").and_then(|value| value.as_str()),
            Some("gpt-5.5-2026-04-23")
        );
    }

    #[test]
    fn converts_completed_response_to_anthropic_message() {
        let response = json!({
            "id": "resp_123",
            "created_at": 1772966030i64,
            "model": "gpt5.4",
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        { "type": "output_text", "text": "done" }
                    ]
                },
                {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "lookup",
                    "arguments": "{\"q\":\"codex\"}"
                }
            ],
            "usage": {
                "input_tokens": 11,
                "output_tokens": 7,
                "input_tokens_details": { "cached_tokens": 3 }
            }
        });

        let converted = convert_completed_response_to_anthropic_message(&response);

        assert_eq!(
            converted.get("type").and_then(Value::as_str),
            Some("message")
        );
        assert_eq!(
            converted.get("model").and_then(Value::as_str),
            Some("gpt-5.4")
        );
        assert_eq!(
            converted.get("stop_reason").and_then(Value::as_str),
            Some("tool_use")
        );
        assert_eq!(
            converted
                .get("content")
                .and_then(|value| value.get(0))
                .and_then(|value| value.get("text"))
                .and_then(Value::as_str),
            Some("done")
        );
        assert_eq!(
            converted
                .get("content")
                .and_then(|value| value.get(1))
                .and_then(|value| value.get("input"))
                .and_then(|value| value.get("q"))
                .and_then(Value::as_str),
            Some("codex")
        );
        assert_eq!(
            converted
                .get("usage")
                .and_then(|value| value.get("cache_read_input_tokens"))
                .and_then(Value::as_i64),
            Some(3)
        );
    }

    #[test]
    fn rewrites_responses_payload_models_for_client() {
        let response = json!({
            "type": "response.completed",
            "response": {
                "id": "resp_123",
                "model": "gpt5.4",
                "status": "completed"
            }
        });

        let rewritten = rewrite_response_models_for_client(response);

        assert_eq!(
            rewritten
                .get("response")
                .and_then(|value| value.get("model"))
                .and_then(|value| value.as_str()),
            Some("gpt-5.4")
        );
    }

    #[test]
    fn rewrites_sse_event_models_for_client() {
        let data = r#"{"type":"response.created","response":{"id":"resp_123","model":"gpt5.4"}}"#;

        let rewritten = rewrite_sse_event_data_models_for_client(data);
        let parsed: serde_json::Value =
            serde_json::from_str(&rewritten).expect("rewritten event should stay valid json");

        assert_eq!(
            parsed
                .get("response")
                .and_then(|value| value.get("model"))
                .and_then(|value| value.as_str()),
            Some("gpt-5.4")
        );
    }

    #[test]
    fn anthropic_streaming_text_events_use_named_sse_shape() {
        let mut state = AnthropicStreamState::default();
        let created = SseEvent {
            event: Some("response.created".to_string()),
            data: json!({
                "type": "response.created",
                "response": {
                    "id": "resp_123",
                    "created_at": 1,
                    "model": "gpt5.4",
                    "usage": {
                        "input_tokens": 5,
                        "output_tokens": 0
                    }
                }
            })
            .to_string(),
        };
        let delta = SseEvent {
            event: Some("response.output_text.delta".to_string()),
            data: json!({
                "type": "response.output_text.delta",
                "delta": "hi"
            })
            .to_string(),
        };
        let completed = SseEvent {
            event: Some("response.completed".to_string()),
            data: json!({
                "type": "response.completed",
                "response": {
                    "id": "resp_123",
                    "model": "gpt5.4",
                    "status": "completed",
                    "usage": {
                        "input_tokens": 5,
                        "output_tokens": 2
                    }
                }
            })
            .to_string(),
        };

        let start = translate_sse_event_to_anthropic_event(&created, &mut state);
        let text = translate_sse_event_to_anthropic_event(&delta, &mut state);
        let stop = translate_sse_event_to_anthropic_event(&completed, &mut state);

        assert_eq!(
            start[0].get("type").and_then(Value::as_str),
            Some("message_start")
        );
        assert_eq!(
            start[0]
                .get("message")
                .and_then(|value| value.get("model"))
                .and_then(Value::as_str),
            Some("gpt-5.4")
        );
        assert_eq!(
            text[0].get("type").and_then(Value::as_str),
            Some("content_block_start")
        );
        assert_eq!(
            text[1]
                .get("delta")
                .and_then(|value| value.get("text"))
                .and_then(Value::as_str),
            Some("hi")
        );
        assert_eq!(
            stop.last()
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("message_stop")
        );
    }

    #[test]
    fn anthropic_streamed_tool_call_emits_single_tool_use_block() {
        let mut state = AnthropicStreamState::default();
        let added = SseEvent {
            event: Some("response.output_item.added".to_string()),
            data: json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "lookup_weather"
                }
            })
            .to_string(),
        };
        let delta = SseEvent {
            event: Some("response.function_call_arguments.delta".to_string()),
            data: json!({
                "type": "response.function_call_arguments.delta",
                "delta": "{\"city\":\"Tokyo\"}"
            })
            .to_string(),
        };
        let arguments_done = SseEvent {
            event: Some("response.function_call_arguments.done".to_string()),
            data: json!({
                "type": "response.function_call_arguments.done",
                "arguments": "{\"city\":\"Tokyo\"}"
            })
            .to_string(),
        };
        let item_done = SseEvent {
            event: Some("response.output_item.done".to_string()),
            data: json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "lookup_weather",
                    "arguments": "{\"city\":\"Tokyo\"}"
                }
            })
            .to_string(),
        };

        let mut tool_block_starts = 0;
        for event in [&added, &delta, &arguments_done, &item_done] {
            for value in translate_sse_event_to_anthropic_event(event, &mut state) {
                if value.get("type").and_then(Value::as_str) == Some("content_block_start")
                    && value
                        .get("content_block")
                        .and_then(|block| block.get("type"))
                        .and_then(Value::as_str)
                        == Some("tool_use")
                {
                    tool_block_starts += 1;
                }
            }
        }

        assert_eq!(
            tool_block_starts, 1,
            "output_item.done must not re-emit a tool_use block that was already streamed"
        );
    }

    #[test]
    fn anthropic_single_shot_tool_item_emits_complete_block() {
        let mut state = AnthropicStreamState::default();
        let item_done = SseEvent {
            event: Some("response.output_item.done".to_string()),
            data: json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "call_id": "call_456",
                    "name": "lookup_weather",
                    "arguments": "{\"city\":\"Osaka\"}"
                }
            })
            .to_string(),
        };

        let values = translate_sse_event_to_anthropic_event(&item_done, &mut state);
        let starts: Vec<&Value> = values
            .iter()
            .filter(|value| {
                value.get("type").and_then(Value::as_str) == Some("content_block_start")
                    && value
                        .get("content_block")
                        .and_then(|block| block.get("type"))
                        .and_then(Value::as_str)
                        == Some("tool_use")
            })
            .collect();

        assert_eq!(starts.len(), 1);
        assert_eq!(
            starts[0]
                .get("content_block")
                .and_then(|block| block.get("id"))
                .and_then(Value::as_str),
            Some("call_456")
        );
        assert_eq!(
            starts[0]
                .get("content_block")
                .and_then(|block| block.get("input"))
                .and_then(|input| input.get("city"))
                .and_then(Value::as_str),
            Some("Osaka")
        );
    }

    #[test]
    fn anthropic_tool_dedup_is_scoped_to_each_call_id() {
        let mut state = AnthropicStreamState::default();
        let first_added = SseEvent {
            event: Some("response.output_item.added".to_string()),
            data: json!({
                "type": "response.output_item.added",
                "item": {"type": "function_call", "call_id": "call_first", "name": "first"}
            })
            .to_string(),
        };
        let first_done = SseEvent {
            event: Some("response.output_item.done".to_string()),
            data: json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "call_id": "call_first",
                    "name": "first",
                    "arguments": "{}"
                }
            })
            .to_string(),
        };
        let second_done = SseEvent {
            event: Some("response.output_item.done".to_string()),
            data: json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "call_id": "call_second",
                    "name": "second",
                    "arguments": "{\"ok\":true}"
                }
            })
            .to_string(),
        };

        translate_sse_event_to_anthropic_event(&first_added, &mut state);
        translate_sse_event_to_anthropic_event(&first_done, &mut state);
        let second_values = translate_sse_event_to_anthropic_event(&second_done, &mut state);

        assert!(second_values.iter().any(|value| {
            value.get("type").and_then(Value::as_str) == Some("content_block_start")
                && value.pointer("/content_block/id").and_then(Value::as_str) == Some("call_second")
        }));
    }

    #[test]
    fn anthropic_system_instructions_strip_billing_header() {
        let request = json!({
            "model": "gpt-5.4",
            "max_tokens": 128,
            "system": [
                {
                    "type": "text",
                    "text": "x-anthropic-billing-header: cc_version=2.1.116.a60; cc_entrypoint=cli; cch=2bb95;"
                },
                {"type": "text", "text": "You are a helpful assistant."}
            ],
            "messages": [{"role": "user", "content": "hi"}]
        });

        let (upstream, _) = convert_anthropic_messages_request_to_codex(&request).unwrap();
        let instructions = upstream
            .get("instructions")
            .and_then(Value::as_str)
            .unwrap_or_default();

        assert!(
            !instructions.contains("x-anthropic-billing-header"),
            "billing header must not leak into upstream instructions: {instructions}"
        );
        assert_eq!(instructions, "You are a helpful assistant.");
    }

    #[test]
    fn anthropic_system_instructions_strip_billing_header_inside_block() {
        let request = json!({
            "model": "gpt-5.4",
            "max_tokens": 128,
            "system": "preamble\nx-anthropic-billing-header: cc_version=2.1.116.a60; cch=2bb95;\nafterword",
            "messages": [{"role": "user", "content": "hi"}]
        });

        let (upstream, _) = convert_anthropic_messages_request_to_codex(&request).unwrap();
        let instructions = upstream
            .get("instructions")
            .and_then(Value::as_str)
            .unwrap_or_default();

        assert!(!instructions.contains("x-anthropic-billing-header"));
        assert_eq!(instructions, "preamble\nafterword");
    }

    #[test]
    fn anthropic_upstream_session_id_prefers_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-claude-code-session-id",
            HeaderValue::from_static("0123abcd-1111-2222-3333-444455556666"),
        );

        let request = json!({
            "metadata": {"user_id": "{\"session_id\":\"fallback-id\"}"}
        });

        assert_eq!(
            anthropic_upstream_session_id(&headers, &request).as_deref(),
            Some("0123abcd-1111-2222-3333-444455556666")
        );
    }

    #[test]
    fn anthropic_upstream_session_id_falls_back_to_metadata_user_id() {
        let headers = HeaderMap::new();
        let request = json!({
            "metadata": {
                "user_id": "{\"device_id\":\"abc\",\"account_uuid\":\"\",\"session_id\":\"095c1c3e-52f2-48bd-aa2a-c53d1cec6736\"}"
            }
        });

        assert_eq!(
            anthropic_upstream_session_id(&headers, &request).as_deref(),
            Some("095c1c3e-52f2-48bd-aa2a-c53d1cec6736")
        );
    }

    #[test]
    fn apply_anthropic_upstream_session_id_inserts_header_once() {
        let request = json!({
            "metadata": {"user_id": "{\"session_id\":\"sid-from-metadata\"}"}
        });

        let headers = apply_anthropic_upstream_session_id(HeaderMap::new(), &request);
        assert_eq!(
            headers
                .get("session-id")
                .and_then(|value| value.to_str().ok()),
            Some("sid-from-metadata")
        );

        let mut existing = HeaderMap::new();
        existing.insert("session-id", HeaderValue::from_static("client-provided"));
        let headers = apply_anthropic_upstream_session_id(existing, &request);
        assert_eq!(
            headers
                .get("session-id")
                .and_then(|value| value.to_str().ok()),
            Some("client-provided")
        );

        let headers = apply_anthropic_upstream_session_id(HeaderMap::new(), &json!({}));
        assert!(headers.get("session-id").is_none());
    }

    #[test]
    fn streaming_completed_without_tool_calls_finishes_with_stop() {
        let mut state = ChatStreamState::default();
        let created = SseEvent {
            event: Some("response.created".to_string()),
            data: json!({
                "type": "response.created",
                "response": {
                    "id": "resp_123",
                    "created_at": 1,
                    "model": "gpt-5.4"
                }
            })
            .to_string(),
        };
        let completed = SseEvent {
            event: Some("response.completed".to_string()),
            data: json!({
                "type": "response.completed",
                "response": {
                    "id": "resp_123",
                    "created_at": 1,
                    "model": "gpt-5.4",
                    "status": "completed"
                }
            })
            .to_string(),
        };

        assert!(translate_sse_event_to_chat_chunk(&created, &mut state).is_empty());

        let chunks = translate_sse_event_to_chat_chunk(&completed, &mut state);
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0]
                .get("choices")
                .and_then(|value| value.get(0))
                .and_then(|value| value.get("finish_reason"))
                .and_then(|value| value.as_str()),
            Some("stop")
        );
    }

    #[test]
    fn streaming_first_tool_call_uses_zero_based_index() {
        let mut state = ChatStreamState::default();
        let added = SseEvent {
            event: Some("response.output_item.added".to_string()),
            data: json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "lookup_weather"
                }
            })
            .to_string(),
        };

        let chunks = translate_sse_event_to_chat_chunk(&added, &mut state);
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0]
                .get("choices")
                .and_then(|value| value.get(0))
                .and_then(|value| value.get("delta"))
                .and_then(|value| value.get("tool_calls"))
                .and_then(|value| value.get(0))
                .and_then(|value| value.get("index"))
                .and_then(|value| value.as_i64()),
            Some(0)
        );
    }

    #[test]
    fn parses_proxy_request_body_limit_mib_from_valid_value() {
        assert_eq!(parse_proxy_request_body_limit_mib(Some("1024")), Some(1024));
    }

    #[test]
    fn ignores_invalid_proxy_request_body_limit_values() {
        assert_eq!(parse_proxy_request_body_limit_mib(Some("")), None);
        assert_eq!(parse_proxy_request_body_limit_mib(Some("0")), None);
        assert_eq!(parse_proxy_request_body_limit_mib(Some("-1")), None);
        assert_eq!(parse_proxy_request_body_limit_mib(Some("abc")), None);
    }

    #[test]
    fn falls_back_to_default_proxy_request_body_limit_bytes() {
        assert_eq!(
            resolve_proxy_request_body_limit_bytes_from_mib_value(None),
            DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES
        );
        assert_eq!(
            resolve_proxy_request_body_limit_bytes_from_mib_value(Some("bad")),
            DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES
        );
    }

    #[test]
    fn converts_proxy_request_body_limit_mib_to_bytes() {
        assert_eq!(
            resolve_proxy_request_body_limit_bytes_from_mib_value(Some("1")),
            1024 * 1024
        );
    }
}
