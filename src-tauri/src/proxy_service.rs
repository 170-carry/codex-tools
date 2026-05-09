use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::convert::Infallible;
use std::fs;
use std::io::Write;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::Arc;
use std::sync::RwLock;

use async_stream::stream;
use axum::body::Body;
use axum::body::Bytes;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::Message as AxumWebSocketMessage;
use axum::extract::ws::WebSocket as AxumWebSocket;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::DefaultBodyLimit;
use axum::extract::Multipart;
use axum::extract::State;
use axum::http::HeaderMap;
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
use futures_util::SinkExt;
use futures_util::Stream;
use futures_util::StreamExt;
use if_addrs::IfAddr;
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
use tokio_tungstenite::client_async_tls_with_config;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

#[cfg(feature = "desktop")]
use tauri::AppHandle;

use crate::app_paths;
use crate::auth::extract_auth;
use crate::auth::refresh_chatgpt_auth_tokens_serialized;
use crate::models::normalize_api_proxy_sequential_five_hour_limit_percent;
use crate::models::ApiProxyLoadBalanceMode;
use crate::models::ApiProxyStatus;
use crate::models::ApiProxyUsagePoint;
use crate::models::ApiProxyUsageSeries;
use crate::models::ApiProxyUsageStats;
use crate::models::AppSettings;
use crate::models::StoredAccount;
use crate::models::UsageSnapshot;
use crate::models::UsageWindow;
use crate::state::ApiProxyRuntimeHandle;
use crate::state::ApiProxyRuntimeSnapshot;
#[cfg(feature = "desktop")]
use crate::state::AppState;
use crate::store::account_store_path_from_data_dir;
use crate::store::load_store_from_path;
use crate::store::save_store_to_path;
use crate::store::update_account_group_refresh_state_in_path;
use crate::usage::resolve_chatgpt_base_origin;
use crate::utils::now_unix_seconds;
use crate::utils::set_private_permissions;
use crate::utils::truncate_for_error;

const DEFAULT_PROXY_PORT: u16 = 8787;
const DEFAULT_PROXY_REQUEST_BODY_LIMIT_MIB: usize = 512;
const DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES: usize =
    DEFAULT_PROXY_REQUEST_BODY_LIMIT_MIB * 1024 * 1024;
const DEFAULT_PROXY_UPSTREAM_TIMEOUT_SECS: u64 = 1_800;
const DEFAULT_PROXY_CONNECT_TIMEOUT_SECS: u64 = 30;
const MAX_PROXY_CONNECT_RESPONSE_BYTES: usize = 16 * 1024;
const PROXY_REQUEST_BODY_LIMIT_MIB_ENV_VAR: &str = "CODEX_TOOLS_PROXY_MAX_BODY_MIB";
const CODEX_CLIENT_VERSION: &str = "0.125.0";
const CODEX_USER_AGENT: &str = "codex_cli_rs/0.125.0";
const RESPONSES_WEBSOCKETS_BETA: &str = "responses_websockets=2026-02-06";
const SSE_DONE: &str = "data: [DONE]\n\n";
const DEFAULT_IMAGE_CONTROLLER_MODEL: &str = "gpt-5.5";
const DEFAULT_IMAGE_TOOL_MODEL: &str = "gpt-image-2";
const IMAGE_VARIATION_PROMPT: &str = "Create a faithful variation of the provided image.";
const MODELS: &[&str] = &[
    "gpt-5",
    "gpt-5.5",
    "gpt-5.4",
    "gpt-5-mini",
    "gpt-5-codex",
    "gpt-5-codex-mini",
    "gpt-5.1-codex",
    "gpt-5.1-codex-mini",
    "gpt-5.1-codex-max",
    "gpt-5.2-codex",
    "gpt-5.3-codex",
    "gpt-5.3-codex-spark",
    "gpt-image-2",
    "gpt-image-1.5",
    "gpt-image-1",
    "gpt-image-1-mini",
    "chatgpt-image-latest",
];
const REQUEST_MODEL_MAPPINGS: &[(&str, &str)] = &[
    ("gpt5.5", "gpt-5.5"),
    ("gpt-5-5", "gpt-5.5"),
    ("gpt5.4", "gpt-5.4"),
    ("gpt-5-4", "gpt-5.4"),
];
const RESPONSE_MODEL_NORMALIZATIONS: &[(&str, &str)] = &[
    ("gpt5.5", "gpt-5.5"),
    ("gpt-5-5", "gpt-5.5"),
    ("gpt5.4", "gpt-5.4"),
    ("gpt-5-4", "gpt-5.4"),
];
const UNSUPPORTED_RESPONSES_REQUEST_FIELDS: &[&str] = &["metadata", "prompt_cache_retention"];
const API_PROXY_USAGE_FILE_NAME: &str = "api-proxy-usage.json";
const API_PROXY_USAGE_STORE_VERSION: u8 = 1;
const API_PROXY_USAGE_RETENTION_SECONDS: i64 = 30 * 24 * 60 * 60;
const API_PROXY_USAGE_RANGE_1H_SECONDS: i64 = 60 * 60;
const API_PROXY_USAGE_RANGE_24H_SECONDS: i64 = 24 * 60 * 60;
const API_PROXY_USAGE_RANGE_7D_SECONDS: i64 = 7 * 24 * 60 * 60;
const API_PROXY_USAGE_RANGE_14D_SECONDS: i64 = 14 * 24 * 60 * 60;
const API_PROXY_USAGE_RANGE_30D_SECONDS: i64 = 30 * 24 * 60 * 60;
const DEFAULT_API_PROXY_USAGE_RANGE_SECONDS: i64 = API_PROXY_USAGE_RANGE_24H_SECONDS;

#[derive(Clone)]
pub(crate) struct ProxyStorageContext {
    pub(crate) data_dir: PathBuf,
    pub(crate) store_lock: Arc<tokio::sync::Mutex<()>>,
    pub(crate) auth_refresh_lock: Arc<tokio::sync::Mutex<()>>,
    pub(crate) sync_active_auth_on_refresh: bool,
}

#[derive(Clone)]
struct ProxyCandidate {
    id: String,
    label: String,
    account_key: String,
    account_id: String,
    access_token: String,
    auth_json: Value,
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
    persisted_sequential_account_key: Option<String>,
}

#[derive(Clone, Debug)]
struct ApiProxyUsageMetadata {
    model: String,
    route: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ApiProxyUsageEvent {
    timestamp: i64,
    model: String,
    calls: i64,
    tokens: i64,
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
    api_key: Arc<RwLock<String>>,
    upstream_base_url: String,
    client: reqwest::Client,
    shared: Arc<tokio::sync::Mutex<ApiProxyRuntimeSnapshot>>,
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
}

struct ApiProxyHandleState {
    port: u16,
    api_key: Arc<RwLock<String>>,
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
        sync_active_auth_on_refresh,
    }
}

#[cfg(feature = "desktop")]
fn app_proxy_storage_context(
    app: &AppHandle,
    state: &AppState,
) -> Result<ProxyStorageContext, String> {
    Ok(new_proxy_storage_context(
        app_data_dir(app)?,
        state.store_lock.clone(),
        state.auth_refresh_lock.clone(),
        true,
    ))
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
    let _guard = storage.store_lock.lock().await;
    let path = api_proxy_usage_path(storage)?;
    let should_scrub_legacy_fields = api_proxy_usage_store_has_legacy_private_fields(&path);
    let mut store = load_api_proxy_usage_store_from_path(&path)?;
    if prune_api_proxy_usage_events(&mut store.events, now) || should_scrub_legacy_fields {
        store.updated_at = now;
        save_api_proxy_usage_store_to_path(&path, &store)?;
    }

    Ok(build_api_proxy_usage_stats(
        &store.events,
        now,
        range_seconds,
    ))
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
    let now = now_unix_seconds();
    let _guard = storage.store_lock.lock().await;
    let path = api_proxy_usage_path(storage)?;
    let store = ApiProxyUsageStore {
        updated_at: now,
        ..ApiProxyUsageStore::default()
    };
    save_api_proxy_usage_store_to_path(&path, &store)
}

#[cfg(feature = "desktop")]
pub(crate) async fn start_api_proxy_internal(
    app: &AppHandle,
    state: &AppState,
    preferred_port: Option<u16>,
) -> Result<ApiProxyStatus, String> {
    let storage = app_proxy_storage_context(app, state)?;
    start_api_proxy_with_runtime(&storage, &state.api_proxy, preferred_port, "127.0.0.1").await
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
    let api_key = ensure_persisted_api_proxy_key(storage).await?;
    let shared_api_key = Arc::new(RwLock::new(api_key));

    let client = reqwest::Client::builder()
        .user_agent("codex-tools-proxy/0.1")
        .timeout(std::time::Duration::from_secs(
            DEFAULT_PROXY_UPSTREAM_TIMEOUT_SECS,
        ))
        .build()
        .map_err(|error| format!("创建代理 HTTP 客户端失败: {error}"))?;

    let shared = Arc::new(tokio::sync::Mutex::new(ApiProxyRuntimeSnapshot::default()));
    let context = Arc::new(ProxyContext {
        storage: storage.clone(),
        api_key: shared_api_key.clone(),
        upstream_base_url: resolve_codex_upstream_base_url(),
        client,
        shared: shared.clone(),
    });
    let request_body_limit = resolve_proxy_request_body_limit_bytes();

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
        api_key: shared_api_key,
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
        Some(read_current_api_key(&handle.api_key)),
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

pub(crate) async fn refresh_api_proxy_key_with_runtime(
    storage: &ProxyStorageContext,
    runtime_slot: &tokio::sync::Mutex<Option<ApiProxyRuntimeHandle>>,
) -> Result<ApiProxyStatus, String> {
    let new_api_key = regenerate_persisted_api_proxy_key(storage).await?;

    let handle_state = {
        let guard = runtime_slot.lock().await;
        if let Some(handle) = guard.as_ref() {
            if let Ok(mut key_guard) = handle.api_key.write() {
                *key_guard = new_api_key.clone();
            }
            Some(snapshot_handle_state(handle))
        } else {
            None
        }
    };

    match handle_state {
        Some(handle_state) => Ok(status_from_handle_state(handle_state).await),
        None => Ok(stopped_status(Some(new_api_key), None)),
    }
}

async fn health_handler() -> impl IntoResponse {
    Json(json!({ "ok": true }))
}

async fn models_handler(
    State(context): State<Arc<ProxyContext>>,
    headers: HeaderMap,
) -> Response<Body> {
    if let Some(response) = ensure_authorized(&headers, &context.api_key) {
        return response;
    }

    Json(json!({
        "object": "list",
        "data": MODELS
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
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    if let Some(response) = ensure_authorized(&headers, &context.api_key) {
        return response;
    }

    let request_json = match parse_json_request(&body) {
        Ok(value) => value,
        Err(response) => return response,
    };

    let (upstream_payload, downstream_stream) =
        match convert_openai_chat_request_to_codex(&request_json) {
            Ok(value) => value,
            Err(message) => return invalid_request_response(&message),
        };

    let upstream = match send_codex_request_over_candidates(
        &context,
        "/v1/chat/completions",
        &headers,
        &upstream_payload,
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
        api_proxy_usage_metadata(&candidate, "/v1/chat/completions", &upstream_payload);

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
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    if let Some(response) = ensure_authorized(&headers, &context.api_key) {
        return response;
    }

    let request_json = match parse_json_request(&body) {
        Ok(value) => value,
        Err(response) => return response,
    };

    let (upstream_payload, downstream_stream) =
        match normalize_openai_responses_request(request_json) {
            Ok(value) => value,
            Err(message) => return invalid_request_response(&message),
        };

    let upstream = match send_codex_request_over_candidates(
        &context,
        "/v1/responses",
        &headers,
        &upstream_payload,
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return response,
    };

    let (candidate, upstream_response) = upstream;
    update_proxy_target(&context, &candidate).await;
    update_proxy_error(&context, None).await;
    let usage_metadata = api_proxy_usage_metadata(&candidate, "/v1/responses", &upstream_payload);

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

async fn image_generations_handler(
    State(context): State<Arc<ProxyContext>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    if let Some(response) = ensure_authorized(&headers, &context.api_key) {
        return response;
    }

    let request_json = match parse_json_request(&body) {
        Ok(value) => value,
        Err(response) => return response,
    };

    let image_request = match convert_openai_image_generation_request_to_codex(&request_json) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&message),
    };

    forward_image_request(context, "/v1/images/generations", headers, image_request).await
}

async fn image_edits_handler(
    State(context): State<Arc<ProxyContext>>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response<Body> {
    if let Some(response) = ensure_authorized(&headers, &context.api_key) {
        return response;
    }

    let request = match parse_image_multipart_request(multipart).await {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&message),
    };
    let image_request = match convert_openai_image_edit_request_to_codex(&request, false) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&message),
    };

    forward_image_request(context, "/v1/images/edits", headers, image_request).await
}

async fn image_variations_handler(
    State(context): State<Arc<ProxyContext>>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response<Body> {
    if let Some(response) = ensure_authorized(&headers, &context.api_key) {
        return response;
    }

    let request = match parse_image_multipart_request(multipart).await {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&message),
    };
    let image_request = match convert_openai_image_edit_request_to_codex(&request, true) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&message),
    };

    forward_image_request(context, "/v1/images/variations", headers, image_request).await
}

async fn forward_image_request(
    context: Arc<ProxyContext>,
    route: &'static str,
    headers: HeaderMap,
    image_request: ConvertedImageRequest,
) -> Response<Body> {
    let ConvertedImageRequest {
        upstream_payload,
        downstream_stream,
        image_count,
    } = image_request;

    let upstream = match send_codex_request_over_candidates(
        &context,
        route,
        &headers,
        &upstream_payload,
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return response,
    };

    let (candidate, upstream_response) = upstream;
    update_proxy_target(&context, &candidate).await;
    update_proxy_error(&context, None).await;
    let usage_metadata = api_proxy_usage_metadata(&candidate, route, &upstream_payload);

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
                    route,
                    &headers,
                    &upstream_payload,
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
            let usage_metadata = api_proxy_usage_metadata(&candidate, route, &upstream_payload);
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
    if let Some(response) = ensure_authorized(&headers, &context.api_key) {
        return response;
    }

    let Ok(ws) = ws else {
        return json_error_response(
            StatusCode::UPGRADE_REQUIRED,
            "GET /v1/responses requires a WebSocket upgrade.",
        );
    };

    ws.on_upgrade(move |socket| handle_responses_websocket(socket, context, headers))
        .into_response()
}

async fn handle_responses_websocket(
    mut socket: AxumWebSocket,
    context: Arc<ProxyContext>,
    headers: HeaderMap,
) {
    let upstream_payload = match receive_responses_websocket_create(&mut socket).await {
        Ok(value) => value,
        Err(message) => {
            let _ = send_responses_websocket_error(&mut socket, &message).await;
            let _ = socket.close().await;
            return;
        }
    };

    let upstream = match send_codex_request_over_candidates(
        &context,
        "/v1/responses websocket",
        &headers,
        &upstream_payload,
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
    let usage_metadata =
        api_proxy_usage_metadata(&candidate, "/v1/responses websocket", &upstream_payload);

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

async fn receive_responses_websocket_create(socket: &mut AxumWebSocket) -> Result<Value, String> {
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

fn normalize_responses_websocket_create(bytes: &[u8]) -> Result<Value, String> {
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

    normalize_openai_responses_request(request).map(|(payload, _)| payload)
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
            );
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
        );
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

    if let Some(response) = ensure_authorized(&headers, &context.api_key) {
        return response;
    }

    json_error_response(
        StatusCode::NOT_FOUND,
        &format!(
            "当前反代只支持 GET /v1/models、POST /v1/chat/completions、POST /v1/responses、POST /v1/images/generations、POST /v1/images/edits、POST /v1/images/variations，收到的是 {method} {}",
            uri.path()
        ),
    )
}

fn ensure_authorized(headers: &HeaderMap, api_key: &Arc<RwLock<String>>) -> Option<Response<Body>> {
    if is_authorized(headers, &read_current_api_key(api_key)) {
        None
    } else {
        Some(json_error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid proxy api key.",
        ))
    }
}

fn parse_json_request(body: &Bytes) -> Result<Value, Response<Body>> {
    serde_json::from_slice::<Value>(body)
        .map_err(|error| invalid_request_response(&format!("请求体不是合法 JSON: {error}")))
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

    let model = map_client_model_to_upstream(&required_string(request_object, "model")?)?;
    let messages = request_object
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| "聊天请求缺少 messages 数组".to_string())?;

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
            "effort": request_object
                .get("reasoning_effort")
                .and_then(Value::as_str)
                .or_else(|| request_object.get("reasoning").and_then(|value| value.get("effort")).and_then(Value::as_str))
                .unwrap_or("medium"),
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

    Ok((Value::Object(root), downstream_stream))
}

fn normalize_openai_responses_request(mut request: Value) -> Result<(Value, bool), String> {
    let object = request
        .as_object_mut()
        .ok_or_else(|| "responses 请求必须是 JSON 对象".to_string())?;

    let model = map_client_model_to_upstream(&required_string(object, "model")?)?;
    let downstream_stream = object
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    object.insert("model".to_string(), Value::String(model));
    object.insert("stream".to_string(), Value::Bool(true));
    object.insert("store".to_string(), Value::Bool(false));
    if !object.contains_key("instructions") {
        object.insert("instructions".to_string(), Value::String(String::new()));
    }
    if !object.contains_key("parallel_tool_calls") {
        object.insert("parallel_tool_calls".to_string(), Value::Bool(true));
    }

    let reasoning = object
        .entry("reasoning".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !reasoning.is_object() {
        *reasoning = Value::Object(Map::new());
    }
    if let Some(reasoning_object) = reasoning.as_object_mut() {
        if !reasoning_object.contains_key("effort") {
            reasoning_object.insert("effort".to_string(), Value::String("medium".to_string()));
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

    Ok((request, downstream_stream))
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
    matches!(
        model,
        "gpt-image-2"
            | "gpt-image-1.5"
            | "gpt-image-1"
            | "gpt-image-1-mini"
            | "chatgpt-image-latest"
    )
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

fn should_use_responses_websocket(payload: &Value) -> bool {
    let _ = payload;
    false
}

fn websocket_response_create_payload(payload: &Value) -> Value {
    let mut payload = payload.clone();
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "type".to_string(),
            Value::String("response.create".to_string()),
        );
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

async fn send_codex_request_over_candidates(
    context: &ProxyContext,
    route: &str,
    headers: &HeaderMap,
    payload: &Value,
) -> Result<(ProxyCandidate, CodexUpstreamResponse), Response<Body>> {
    let selection = match load_proxy_candidate_selection(&context.storage).await {
        Ok(selection) if !selection.candidates.is_empty() => selection,
        Ok(_) => {
            return Err(json_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "No authorized account is available for proxying.",
            ));
        }
        Err(error) => {
            update_proxy_error(context, Some(error.clone())).await;
            return Err(json_error_response(StatusCode::BAD_GATEWAY, &error));
        }
    };
    let current_sequential_account_key = sequential_account_key_for_request(
        current_sequential_proxy_account_key(context).await,
        selection.persisted_sequential_account_key,
    );
    let candidates = order_proxy_candidates_for_request(
        selection.candidates,
        selection.load_balance,
        current_sequential_account_key.as_deref(),
    );

    let mut attempt_errors = Vec::new();
    let mut retriable_failures = Vec::new();

    for mut candidate in candidates {
        let mut did_refresh = false;

        loop {
            log_proxy_request_route(route);
            let upstream =
                match forward_codex_request_with_candidate(context, &candidate, headers, payload)
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
                record_api_proxy_call_success(context, &candidate, route, payload).await;
                return Ok((candidate, upstream));
            }

            let upstream_headers = upstream.headers().clone();
            let upstream_body = match upstream.into_bytes().await {
                Ok((_, bytes)) => bytes,
                Err(error) => {
                    attempt_errors
                        .push(format!("{}: 读取上游响应失败: {}", candidate.label, error));
                    break;
                }
            };

            if !did_refresh && should_retry_with_token_refresh(status, &upstream_body) {
                if candidate.auth_refresh_blocked {
                    attempt_errors.push(format!(
                        "{}: {}",
                        candidate.label,
                        candidate
                            .auth_refresh_error
                            .clone()
                            .unwrap_or_else(|| "授权过期，请重新登录授权。".to_string())
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
                        attempt_errors
                            .push(format!("{}: 刷新登录态失败: {}", candidate.label, error));
                        break;
                    }
                }
            }

            if let Some(failure) = classify_retriable_failure(status, &upstream_body) {
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
) -> Result<CodexUpstreamResponse, String> {
    let upstream_url = format!("{}/responses", context.upstream_base_url);
    let session_id = headers
        .get("session_id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

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

    if should_use_responses_websocket(payload) {
        return forward_codex_websocket_request_with_candidate(
            context,
            candidate,
            payload,
            &session_id,
            version,
            user_agent,
        )
        .await;
    }

    let serialized =
        serde_json::to_vec(payload).map_err(|error| format!("序列化上游请求失败: {error}"))?;

    context
        .client
        .post(&upstream_url)
        .header(
            "Authorization",
            format!("Bearer {}", candidate.access_token),
        )
        .header("ChatGPT-Account-Id", &candidate.account_id)
        .header("Accept", "text/event-stream")
        .header("Content-Type", "application/json")
        .header("Originator", "codex_cli_rs")
        .header("Version", version)
        .header("Session_id", session_id)
        .header("User-Agent", user_agent)
        .header("Connection", "Keep-Alive")
        .body(serialized)
        .send()
        .await
        .map(CodexUpstreamResponse::Http)
        .map_err(|error| format!("请求 Codex 上游失败 {upstream_url}: {error}"))
}

async fn forward_codex_websocket_request_with_candidate(
    context: &ProxyContext,
    candidate: &ProxyCandidate,
    payload: &Value,
    session_id: &str,
    version: &str,
    user_agent: &str,
) -> Result<CodexUpstreamResponse, String> {
    let upstream_url = format!("{}/responses", context.upstream_base_url);
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
    insert_header(request.headers_mut(), "Session_id", session_id)?;
    insert_header(request.headers_mut(), "x-client-request-id", session_id)?;
    insert_header(
        request.headers_mut(),
        "OpenAI-Beta",
        RESPONSES_WEBSOCKETS_BETA,
    )?;
    insert_header(request.headers_mut(), "User-Agent", user_agent)?;

    let (mut websocket, response) = connect_codex_websocket(request, &websocket_url).await?;

    let request_text = serde_json::to_string(&websocket_response_create_payload(payload))
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
        persisted_sequential_account_key: store.settings.api_proxy_sequential_account_key,
    })
}

fn account_to_proxy_candidate(account: StoredAccount) -> Option<ProxyCandidate> {
    let extracted = extract_auth(&account.auth_json).ok()?;
    let account_key = account.account_key();
    let variant_key = account.variant_key();
    Some(ProxyCandidate {
        id: account.id,
        label: account.label,
        account_key,
        account_id: extracted.account_id,
        access_token: extracted.access_token,
        auth_json: account.auth_json,
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
) -> Vec<ProxyCandidate> {
    candidates.sort_by(compare_proxy_candidates);

    if !matches!(load_balance.mode, ApiProxyLoadBalanceMode::Sequential) {
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

    match usage_window_used_percent(
        left.usage
            .as_ref()
            .and_then(|usage| usage.one_week.as_ref()),
    )
    .total_cmp(&usage_window_used_percent(
        right
            .usage
            .as_ref()
            .and_then(|usage| usage.one_week.as_ref()),
    )) {
        Ordering::Equal => {}
        ordering => return ordering,
    }

    match usage_window_used_percent(
        left.usage
            .as_ref()
            .and_then(|usage| usage.five_hour.as_ref()),
    )
    .total_cmp(&usage_window_used_percent(
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
    let refreshed_auth_json = match refresh_chatgpt_auth_tokens_serialized(
        &candidate.auth_json,
        &storage.auth_refresh_lock,
    )
    .await
    {
        Ok(refreshed) => refreshed,
        Err(error) => {
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

    let extracted = extract_auth(&refreshed_auth_json)
        .map_err(|error| format!("刷新后解析账号登录态失败: {error}"))?;

    Ok(ProxyCandidate {
        id: candidate.id.clone(),
        label: candidate.label.clone(),
        account_key: candidate.account_key.clone(),
        account_id: extracted.account_id,
        access_token: extracted.access_token,
        auth_json: refreshed_auth_json,
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
        return "授权过期，请重新登录授权。".to_string();
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

fn is_authorized(headers: &HeaderMap, api_key: &str) -> bool {
    if let Some(value) = headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
    {
        if value == api_key {
            return true;
        }
    }

    if let Some(value) = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
    {
        if let Some(token) = value.strip_prefix("Bearer ") {
            return token == api_key;
        }
    }

    false
}

async fn read_persisted_api_proxy_key(
    storage: &ProxyStorageContext,
) -> Result<Option<String>, String> {
    if let Some(value) = read_api_proxy_key_file(storage)? {
        return Ok(Some(value));
    }

    let _guard = storage.store_lock.lock().await;
    let store = load_store_from_path(&account_store_path_from_data_dir(&storage.data_dir))?;
    let legacy_value = store
        .settings
        .api_proxy_api_key
        .clone()
        .filter(|value| !value.trim().is_empty());

    if let Some(value) = legacy_value.clone() {
        write_api_proxy_key_file(storage, &value)?;
    }

    Ok(legacy_value)
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

async fn ensure_persisted_api_proxy_key(storage: &ProxyStorageContext) -> Result<String, String> {
    if let Some(existing) = read_api_proxy_key_file(storage)? {
        return Ok(existing);
    }

    let _guard = storage.store_lock.lock().await;
    let store_path = account_store_path_from_data_dir(&storage.data_dir);
    let mut store = load_store_from_path(&store_path)?;

    if let Some(existing) = store
        .settings
        .api_proxy_api_key
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        write_api_proxy_key_file(storage, &existing)?;
        return Ok(existing);
    }

    let new_key = generate_api_proxy_key();
    store.settings.api_proxy_api_key = Some(new_key.clone());
    save_store_to_path(&store_path, &store)?;
    write_api_proxy_key_file(storage, &new_key)?;
    Ok(new_key)
}

async fn regenerate_persisted_api_proxy_key(
    storage: &ProxyStorageContext,
) -> Result<String, String> {
    let new_key = generate_api_proxy_key();
    write_api_proxy_key_file(storage, &new_key)?;

    let _guard = storage.store_lock.lock().await;
    let store_path = account_store_path_from_data_dir(&storage.data_dir);
    let mut store = load_store_from_path(&store_path)?;
    store.settings.api_proxy_api_key = Some(new_key.clone());
    save_store_to_path(&store_path, &store)?;

    Ok(new_key)
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

fn api_proxy_usage_path(storage: &ProxyStorageContext) -> Result<PathBuf, String> {
    Ok(storage.data_dir.join(API_PROXY_USAGE_FILE_NAME))
}

async fn record_api_proxy_call_success(
    context: &ProxyContext,
    candidate: &ProxyCandidate,
    route: &str,
    payload: &Value,
) {
    let metadata = api_proxy_usage_metadata(candidate, route, payload);
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
    let Some(tokens) = api_proxy_usage_tokens_from_response(response) else {
        return;
    };

    if let Err(error) = append_api_proxy_usage_event(
        storage,
        api_proxy_usage_event(metadata, 0, tokens, now_unix_seconds()),
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

fn maybe_record_stream_usage_tokens(
    storage: &ProxyStorageContext,
    metadata: &ApiProxyUsageMetadata,
    event: &SseEvent,
    recorded_usage: &mut bool,
) {
    if *recorded_usage {
        return;
    }
    let Some(tokens) = api_proxy_usage_tokens_from_sse_event(event) else {
        return;
    };

    *recorded_usage = true;
    let storage = storage.clone();
    let metadata = metadata.clone();
    tokio::spawn(async move {
        if let Err(error) = append_api_proxy_usage_event(
            &storage,
            api_proxy_usage_event(&metadata, 0, tokens, now_unix_seconds()),
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
    });
}

fn api_proxy_usage_metadata(
    _candidate: &ProxyCandidate,
    route: &str,
    payload: &Value,
) -> ApiProxyUsageMetadata {
    ApiProxyUsageMetadata {
        model: api_proxy_usage_model_from_payload(payload),
        route: route.to_string(),
    }
}

fn api_proxy_usage_model_from_payload(payload: &Value) -> String {
    payload
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(normalize_model_for_client)
        .unwrap_or_else(|| "unknown".to_string())
}

fn api_proxy_usage_event(
    metadata: &ApiProxyUsageMetadata,
    calls: i64,
    tokens: i64,
    timestamp: i64,
) -> ApiProxyUsageEvent {
    ApiProxyUsageEvent {
        timestamp,
        model: metadata.model.clone(),
        calls,
        tokens,
    }
}

async fn append_api_proxy_usage_event(
    storage: &ProxyStorageContext,
    mut event: ApiProxyUsageEvent,
) -> Result<(), String> {
    event.calls = event.calls.max(0);
    event.tokens = event.tokens.max(0);
    if event.calls == 0 && event.tokens == 0 {
        return Ok(());
    }
    if event.model.trim().is_empty() {
        event.model = "unknown".to_string();
    }
    let now = now_unix_seconds();
    if event.timestamp <= 0 {
        event.timestamp = now;
    }

    let _guard = storage.store_lock.lock().await;
    let path = api_proxy_usage_path(storage)?;
    let mut store = load_api_proxy_usage_store_from_path(&path)?;
    store.events.push(event);
    prune_api_proxy_usage_events(&mut store.events, now);
    store.updated_at = now;
    save_api_proxy_usage_store_to_path(&path, &store)
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

fn api_proxy_usage_store_has_legacy_private_fields(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    fs::read_to_string(path).is_ok_and(|raw| {
        raw.contains("accountKey")
            || raw.contains("accountId")
            || raw.contains("accountLabel")
            || raw.contains("route")
    })
}

fn save_api_proxy_usage_store_to_path(
    path: &Path,
    store: &ApiProxyUsageStore,
) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(store)
        .map_err(|error| format!("序列化 API 反代统计存储失败: {error}"))?;
    write_private_named_file_atomically(path, serialized.as_bytes(), "API 反代统计")
}

fn prune_api_proxy_usage_events(events: &mut Vec<ApiProxyUsageEvent>, now: i64) -> bool {
    let cutoff = now.saturating_sub(API_PROXY_USAGE_RETENTION_SECONDS);
    let before = events.len();
    events.retain(|event| {
        event.timestamp >= cutoff
            && !event.model.trim().is_empty()
            && (event.calls.max(0) > 0 || event.tokens.max(0) > 0)
    });
    before != events.len()
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

    ApiProxyUsageStats {
        updated_at: now,
        range_seconds,
        bucket_seconds,
        series,
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

fn api_proxy_usage_tokens_from_response(response: &Value) -> Option<i64> {
    response
        .get("usage")
        .and_then(api_proxy_usage_tokens_from_usage)
}

fn api_proxy_usage_tokens_from_sse_event(event: &SseEvent) -> Option<i64> {
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

fn api_proxy_usage_tokens_from_usage(usage: &Value) -> Option<i64> {
    token_count_field(usage, "total_tokens").or_else(|| {
        let input = token_count_field(usage, "input_tokens")
            .or_else(|| token_count_field(usage, "prompt_tokens"))?;
        let output = token_count_field(usage, "output_tokens")
            .or_else(|| token_count_field(usage, "completion_tokens"))?;
        input.checked_add(output)
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
        let mut temp_file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|error| {
                format!(
                    "创建 {purpose} 临时文件失败 {}: {error}",
                    temp_path.display()
                )
            })?;
        temp_file.write_all(contents).map_err(|error| {
            format!(
                "写入 {purpose} 临时文件失败 {}: {error}",
                temp_path.display()
            )
        })?;
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

fn read_current_api_key(shared: &Arc<RwLock<String>>) -> String {
    shared.read().map(|value| value.clone()).unwrap_or_default()
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
                        );
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
            );
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
                        );
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
            );
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
                        );
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
            );
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
        snapshot.active_account_label = Some(candidate.label.clone());
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

async fn current_sequential_proxy_account_key(context: &ProxyContext) -> Option<String> {
    context.shared.lock().await.sequential_account_key.clone()
}

async fn update_proxy_error(context: &ProxyContext, error: Option<String>) {
    let mut snapshot = context.shared.lock().await;
    snapshot.last_error = error;
}

fn snapshot_handle_state(handle: &ApiProxyRuntimeHandle) -> ApiProxyHandleState {
    ApiProxyHandleState {
        port: handle.port,
        api_key: handle.api_key.clone(),
        task_finished: handle.task.is_finished(),
        shared: handle.shared.clone(),
    }
}

async fn status_from_handle_state(handle: ApiProxyHandleState) -> ApiProxyStatus {
    let snapshot = handle.shared.lock().await.clone();
    if handle.task_finished {
        ApiProxyStatus {
            running: false,
            port: None,
            api_key: Some(read_current_api_key(&handle.api_key)),
            base_url: None,
            lan_base_url: None,
            active_account_key: snapshot.active_account_key,
            active_account_id: snapshot.active_account_id,
            active_account_label: snapshot.active_account_label,
            last_error: snapshot.last_error,
        }
    } else {
        ApiProxyStatus {
            running: true,
            port: Some(handle.port),
            api_key: Some(read_current_api_key(&handle.api_key)),
            base_url: Some(proxy_base_url(handle.port)),
            lan_base_url: proxy_lan_base_url(handle.port),
            active_account_key: snapshot.active_account_key,
            active_account_id: snapshot.active_account_id,
            active_account_label: snapshot.active_account_label,
            last_error: snapshot.last_error,
        }
    }
}

fn stopped_status(api_key: Option<String>, last_error: Option<String>) -> ApiProxyStatus {
    ApiProxyStatus {
        running: false,
        port: None,
        api_key,
        base_url: None,
        lan_base_url: None,
        active_account_key: None,
        active_account_id: None,
        active_account_label: None,
        last_error,
    }
}

fn resolve_codex_upstream_base_url() -> String {
    format!(
        "{}/backend-api/codex",
        resolve_chatgpt_base_origin().trim_end_matches('/')
    )
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
    use super::api_proxy_usage_bucket_seconds;
    use super::api_proxy_usage_store_has_legacy_private_fields;
    use super::build_api_proxy_usage_stats;
    use super::convert_completed_response_to_chat_completion;
    use super::convert_openai_chat_request_to_codex;
    use super::convert_openai_image_edit_request_to_codex;
    use super::convert_openai_image_generation_request_to_codex;
    use super::convert_responses_image_output_to_images_response;
    use super::extract_completed_response_from_sse;
    use super::find_http_header_end;
    use super::host_matches_no_proxy;
    use super::is_responses_terminal_event;
    use super::normalize_openai_responses_request;
    use super::normalize_responses_websocket_create;
    use super::now_unix_seconds;
    use super::order_proxy_candidates_for_request;
    use super::parse_http_proxy_config;
    use super::parse_proxy_request_body_limit_mib;
    use super::prune_api_proxy_usage_events;
    use super::resolve_proxy_request_body_limit_bytes_from_mib_value;
    use super::rewrite_response_models_for_client;
    use super::rewrite_sse_event_data_models_for_client;
    use super::sequential_account_key_for_request;
    use super::should_use_responses_websocket;
    use super::translate_sse_event_to_chat_chunk;
    use super::translate_sse_event_to_image_chunk;
    use super::websocket_target_host_port;
    use super::ApiProxyUsageEvent;
    use super::ChatStreamState;
    use super::ImageMultipartRequest;
    use super::ProxyCandidate;
    use super::ProxyLoadBalanceConfig;
    use super::SseEvent;
    use super::API_PROXY_USAGE_RANGE_1H_SECONDS;
    use super::API_PROXY_USAGE_RETENTION_SECONDS;
    use super::DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES;
    use crate::models::ApiProxyLoadBalanceMode;
    use crate::models::UsageSnapshot;
    use crate::models::UsageWindow;
    use serde_json::json;
    use serde_json::Value;

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
            account_key: account_key.to_string(),
            account_id: account_key.to_string(),
            access_token: "token".to_string(),
            auth_json: json!({}),
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
            model: model.to_string(),
            calls,
            tokens,
        }
    }

    #[test]
    fn api_proxy_usage_stats_bucket_calls_and_tokens_per_model() {
        let now = 3_600;
        let bucket_seconds = api_proxy_usage_bucket_seconds(API_PROXY_USAGE_RANGE_1H_SECONDS);
        let call_bucket = now - 120;
        let token_bucket = now - 60;
        let events = vec![
            usage_event(call_bucket, "gpt-5", 1, 0),
            usage_event(token_bucket, "gpt-5", 0, 42),
            usage_event(now - API_PROXY_USAGE_RANGE_1H_SECONDS - 1, "gpt-5", 1, 99),
            usage_event(now - 30, "gpt-5.5", 1, 12),
        ];

        let stats = build_api_proxy_usage_stats(&events, now, API_PROXY_USAGE_RANGE_1H_SECONDS);

        assert_eq!(stats.range_seconds, API_PROXY_USAGE_RANGE_1H_SECONDS);
        assert_eq!(stats.bucket_seconds, bucket_seconds);
        assert_eq!(stats.series.len(), 2);

        let gpt5 = stats
            .series
            .iter()
            .find(|series| series.model == "gpt-5")
            .expect("gpt-5 series");
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
    fn api_proxy_usage_pruning_removes_events_older_than_retention() {
        let now = 10_000_000;
        let cutoff = now - API_PROXY_USAGE_RETENTION_SECONDS;
        let mut events = vec![
            usage_event(cutoff - 1, "gpt-5", 1, 0),
            usage_event(cutoff, "gpt-5", 1, 0),
            usage_event(now, "gpt-5.5", 0, 20),
        ];

        let changed = prune_api_proxy_usage_events(&mut events, now);

        assert!(changed);
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|event| event.timestamp >= cutoff));
    }

    #[test]
    fn api_proxy_usage_store_detects_legacy_private_fields() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "codex-tools-usage-legacy-{}-{}.json",
            std::process::id(),
            now_unix_seconds()
        ));

        std::fs::write(
            &path,
            r#"{"version":1,"events":[{"accountKey":"a","accountId":"b","accountLabel":"c","route":"/v1/responses"}]}"#,
        )
        .expect("write legacy usage store");

        assert!(api_proxy_usage_store_has_legacy_private_fields(&path));

        std::fs::write(
            &path,
            r#"{"version":1,"events":[{"timestamp":1,"model":"gpt-5","calls":1,"tokens":0}]}"#,
        )
        .expect("write sanitized usage store");

        assert!(!api_proxy_usage_store_has_legacy_private_fields(&path));
        let _ = std::fs::remove_file(path);
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
    fn sequential_load_balance_reuses_current_candidate_with_missing_five_hour_usage() {
        let candidates = vec![
            proxy_candidate("smart best", "a", Some(5.0), Some(10.0), false),
            proxy_candidate("current missing 5h", "b", Some(50.0), None, false),
        ];

        let ordered = order_proxy_candidates_for_request(
            candidates,
            load_balance_config(ApiProxyLoadBalanceMode::Sequential, 80.0),
            Some("b"),
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
        );

        assert_eq!(
            candidate_labels(&ordered),
            vec!["next under limit", "current at limit", "blocked"]
        );
    }

    #[test]
    fn converts_chat_request_to_codex_payload() {
        let request = json!({
            "model": "gpt-5",
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
            Some("gpt-5")
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

        let payload = normalize_responses_websocket_create(&bytes)
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
data: {"type":"response.completed","response":{"id":"resp_123","created_at":1,"model":"gpt-5","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"2"}]}],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}

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
    fn streaming_completed_without_tool_calls_finishes_with_stop() {
        let mut state = ChatStreamState::default();
        let created = SseEvent {
            event: Some("response.created".to_string()),
            data: json!({
                "type": "response.created",
                "response": {
                    "id": "resp_123",
                    "created_at": 1,
                    "model": "gpt-5"
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
                    "model": "gpt-5",
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
