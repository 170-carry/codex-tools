use std::cmp::Ordering;
use std::convert::Infallible;
use std::fs;
use std::io::Write;
use std::net::Ipv4Addr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

use async_stream::stream;
use axum::body::Body;
use axum::body::Bytes;
use axum::extract::DefaultBodyLimit;
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
use if_addrs::IfAddr;
use serde_json::json;
use serde_json::Map;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[cfg(feature = "desktop")]
use tauri::AppHandle;
#[cfg(feature = "desktop")]
use tauri::Manager;

use crate::auth::current_auth_variant_key;
use crate::auth::extract_auth;
use crate::auth::refresh_chatgpt_auth_tokens;
use crate::auth::write_active_codex_auth;
use crate::models::ApiProxyStatus;
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
use crate::usage::resolve_chatgpt_base_origin;
use crate::utils::now_unix_seconds;
use crate::utils::set_private_permissions;
use crate::utils::truncate_for_error;

const DEFAULT_PROXY_PORT: u16 = 8787;
const DEFAULT_PROXY_REQUEST_BODY_LIMIT_MIB: usize = 512;
const DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES: usize =
    DEFAULT_PROXY_REQUEST_BODY_LIMIT_MIB * 1024 * 1024;
const DEFAULT_PROXY_UPSTREAM_TIMEOUT_SECS: u64 = 1_800;
const PROXY_REQUEST_BODY_LIMIT_MIB_ENV_VAR: &str = "CODEX_TOOLS_PROXY_MAX_BODY_MIB";
const CODEX_CLIENT_VERSION: &str = "0.101.0";
const CODEX_USER_AGENT: &str = "codex_cli_rs/0.101.0 (Mac OS 26.0.1; arm64) Apple_Terminal/464";
const SSE_DONE: &str = "data: [DONE]\n\n";
const MODELS: &[&str] = &[
    "gpt-5",
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
];
const REQUEST_MODEL_MAPPINGS: &[(&str, &str)] = &[("gpt-5-4", "gpt-5.4")];
const CLIENT_MODEL_REJECTIONS: &[(&str, &str)] = &[("gpt5.4", "gpt-5-4"), ("gpt-5.4", "gpt-5-4")];
const RESPONSE_MODEL_NORMALIZATIONS: &[(&str, &str)] =
    &[("gpt5.4", "gpt-5.4"), ("gpt-5-4", "gpt-5.4")];

#[derive(Clone)]
pub(crate) struct ProxyStorageContext {
    pub(crate) data_dir: PathBuf,
    pub(crate) store_lock: Arc<tokio::sync::Mutex<()>>,
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
}

#[derive(Clone)]
struct ProxyContext {
    storage: ProxyStorageContext,
    api_key: Arc<RwLock<String>>,
    upstream_base_url: String,
    client: reqwest::Client,
    shared: Arc<tokio::sync::Mutex<ApiProxyRuntimeSnapshot>>,
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
    sync_active_auth_on_refresh: bool,
) -> ProxyStorageContext {
    ProxyStorageContext {
        data_dir,
        store_lock,
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
        .route("/v1/responses", post(responses_handler))
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

    let upstream =
        match send_codex_request_over_candidates(&context, &headers, &upstream_payload).await {
            Ok(value) => value,
            Err(response) => return response,
        };

    let (candidate, upstream_response) = upstream;
    update_proxy_target(&context, &candidate).await;
    update_proxy_error(&context, None).await;

    if downstream_stream {
        build_chat_streaming_response(upstream_response)
    } else {
        let upstream_headers = upstream_response.headers().clone();
        let upstream_body = match upstream_response.bytes().await {
            Ok(bytes) => bytes,
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

    let upstream =
        match send_codex_request_over_candidates(&context, &headers, &upstream_payload).await {
            Ok(value) => value,
            Err(response) => return response,
        };

    let (candidate, upstream_response) = upstream;
    update_proxy_target(&context, &candidate).await;
    update_proxy_error(&context, None).await;

    if downstream_stream {
        build_passthrough_sse_response(upstream_response)
    } else {
        let upstream_headers = upstream_response.headers().clone();
        let upstream_body = match upstream_response.bytes().await {
            Ok(bytes) => bytes,
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
            "当前反代只支持 GET /v1/models、POST /v1/chat/completions、POST /v1/responses，收到的是 {method} {}",
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

    Ok((request, downstream_stream))
}

fn map_client_model_to_upstream(model: &str) -> Result<String, String> {
    if let Some(mapped) = remap_model_name(model, REQUEST_MODEL_MAPPINGS) {
        return Ok(mapped);
    }
    if let Some(suggested) = remap_model_name(model, CLIENT_MODEL_REJECTIONS) {
        return Err(format!("模型 {model} 不支持，请改用 {suggested}"));
    }

    Ok(model.to_string())
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
    headers: &HeaderMap,
    payload: &Value,
) -> Result<(ProxyCandidate, reqwest::Response), Response<Body>> {
    let candidates = match load_proxy_candidates(&context.storage).await {
        Ok(items) if !items.is_empty() => items,
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

    let mut attempt_errors = Vec::new();
    let mut retriable_failures = Vec::new();

    for mut candidate in candidates {
        let mut did_refresh = false;

        loop {
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
            if status.is_success() {
                return Ok((candidate, upstream));
            }

            let upstream_headers = upstream.headers().clone();
            let upstream_body = match upstream.bytes().await {
                Ok(bytes) => bytes,
                Err(error) => {
                    attempt_errors
                        .push(format!("{}: 读取上游响应失败: {}", candidate.label, error));
                    break;
                }
            };

            if !did_refresh && should_retry_with_token_refresh(status, &upstream_body) {
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
) -> Result<reqwest::Response, String> {
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
        .map_err(|error| format!("请求 Codex 上游失败 {upstream_url}: {error}"))
}

async fn load_proxy_candidates(
    storage: &ProxyStorageContext,
) -> Result<Vec<ProxyCandidate>, String> {
    let _guard = storage.store_lock.lock().await;
    let store = load_store_from_path(&account_store_path_from_data_dir(&storage.data_dir))?;

    let mut candidates = store
        .accounts
        .into_iter()
        .filter_map(account_to_proxy_candidate)
        .collect::<Vec<_>>();
    candidates.sort_by(compare_proxy_candidates);
    Ok(candidates)
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
    })
}

fn compare_proxy_candidates(left: &ProxyCandidate, right: &ProxyCandidate) -> Ordering {
    match is_free_plan(&right.plan_type).cmp(&is_free_plan(&left.plan_type)) {
        Ordering::Equal => {}
        ordering => return ordering,
    }

    match remaining_percent(
        right
            .usage
            .as_ref()
            .and_then(|usage| usage.one_week.as_ref()),
    )
    .cmp(&remaining_percent(
        left.usage
            .as_ref()
            .and_then(|usage| usage.one_week.as_ref()),
    )) {
        Ordering::Equal => {}
        ordering => return ordering,
    }

    match remaining_percent(
        right
            .usage
            .as_ref()
            .and_then(|usage| usage.five_hour.as_ref()),
    )
    .cmp(&remaining_percent(
        left.usage
            .as_ref()
            .and_then(|usage| usage.five_hour.as_ref()),
    )) {
        Ordering::Equal => {}
        ordering => return ordering,
    }

    left.label.cmp(&right.label)
}

fn is_free_plan(plan_type: &Option<String>) -> bool {
    plan_type
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("free"))
        .unwrap_or(false)
}

fn remaining_percent(window: Option<&UsageWindow>) -> i32 {
    match window {
        Some(window) => (100.0 - window.used_percent).round().clamp(0.0, 100.0) as i32,
        None => -1,
    }
}

async fn refresh_proxy_candidate_auth(
    storage: &ProxyStorageContext,
    candidate: &ProxyCandidate,
) -> Result<ProxyCandidate, String> {
    let refreshed_auth_json = refresh_chatgpt_auth_tokens(&candidate.auth_json).await?;
    persist_refreshed_candidate_auth(storage, &candidate.id, &refreshed_auth_json).await?;

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
    })
}

async fn persist_refreshed_candidate_auth(
    storage: &ProxyStorageContext,
    id: &str,
    refreshed_auth_json: &Value,
) -> Result<(), String> {
    let _guard = storage.store_lock.lock().await;
    let store_path = account_store_path_from_data_dir(&storage.data_dir);
    let mut store = load_store_from_path(&store_path)?;

    if let Some(account) = store.accounts.iter_mut().find(|account| account.id == id) {
        account.auth_json = refreshed_auth_json.clone();
        account.updated_at = now_unix_seconds();
    }

    save_store_to_path(&store_path, &store)?;

    if storage.sync_active_auth_on_refresh
        && current_auth_variant_key().as_deref()
            == store
                .accounts
                .iter()
                .find(|account| account.id == id)
                .map(|account| account.variant_key())
                .as_deref()
    {
        write_active_codex_auth(refreshed_auth_json)?;
    }

    Ok(())
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

#[cfg(feature = "desktop")]
fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("无法获取应用数据目录: {error}"))
}

fn write_private_file_atomically(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析 API Key 存储目录 {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("创建 API Key 存储目录失败 {}: {error}", parent.display()))?;

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
                format!("创建 API Key 临时文件失败 {}: {error}", temp_path.display())
            })?;
        temp_file.write_all(contents).map_err(|error| {
            format!("写入 API Key 临时文件失败 {}: {error}", temp_path.display())
        })?;
        temp_file.sync_all().map_err(|error| {
            format!("刷新 API Key 临时文件失败 {}: {error}", temp_path.display())
        })?;
        drop(temp_file);
        set_private_permissions(&temp_path);

        #[cfg(target_family = "unix")]
        {
            fs::rename(&temp_path, path).map_err(|error| {
                format!(
                    "替换 API Key 存储文件失败 {} -> {}: {error}",
                    temp_path.display(),
                    path.display()
                )
            })?;

            let parent_dir = fs::File::open(parent).map_err(|error| {
                format!("打开 API Key 存储目录失败 {}: {error}", parent.display())
            })?;
            parent_dir.sync_all().map_err(|error| {
                format!("刷新 API Key 存储目录失败 {}: {error}", parent.display())
            })?;
        }

        #[cfg(not(target_family = "unix"))]
        {
            if path.exists() {
                fs::remove_file(path).map_err(|error| {
                    format!("移除旧 API Key 存储文件失败 {}: {error}", path.display())
                })?;
            }
            fs::rename(&temp_path, path).map_err(|error| {
                format!(
                    "替换 API Key 存储文件失败 {} -> {}: {error}",
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

fn build_passthrough_sse_response(upstream: reqwest::Response) -> Response<Body> {
    let upstream_headers = upstream.headers().clone();
    let output = stream! {
        let mut upstream = upstream;
        let mut decoder = SseDecoder::default();

        loop {
            match upstream.chunk().await {
                Ok(Some(chunk)) => {
                    for event in decoder.push(&chunk) {
                        yield Ok::<Bytes, Infallible>(serialize_sse_event(
                            event.event.as_deref(),
                            &rewrite_sse_event_data_models_for_client(&event.data),
                        ));
                    }
                }
                Ok(None) => break,
                Err(_) => return,
            }
        }

        for event in decoder.finish() {
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

fn build_chat_streaming_response(mut upstream: reqwest::Response) -> Response<Body> {
    let upstream_headers = upstream.headers().clone();
    let output = stream! {
        let mut decoder = SseDecoder::default();
        let mut state = ChatStreamState::default();

        loop {
            match upstream.chunk().await {
                Ok(Some(chunk)) => {
                    for event in decoder.push(&chunk) {
                        for value in translate_sse_event_to_chat_chunk(&event, &mut state) {
                            yield Ok::<Bytes, Infallible>(sse_data_chunk(&value));
                        }
                    }
                }
                Ok(None) => break,
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
                .ok_or_else(|| "Codex 响应缺少 response 字段".to_string());
        }
    }

    let mut decoder = SseDecoder::default();
    for event in decoder.push(bytes) {
        if let Some(response) = response_completed_from_event(&event) {
            return Ok(response);
        }
    }
    for event in decoder.finish() {
        if let Some(response) = response_completed_from_event(&event) {
            return Ok(response);
        }
    }

    Err("未在 Codex SSE 中找到 response.completed 事件".to_string())
}

fn response_completed_from_event(event: &SseEvent) -> Option<Value> {
    let parsed = serde_json::from_str::<Value>(&event.data).ok()?;
    if parsed.get("type").and_then(Value::as_str) != Some("response.completed") {
        return None;
    }
    parsed.get("response").cloned()
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
    let mut snapshot = context.shared.lock().await;
    snapshot.active_account_key = Some(candidate.account_key.clone());
    snapshot.active_account_id = Some(candidate.account_id.clone());
    snapshot.active_account_label = Some(candidate.label.clone());
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
    use super::convert_completed_response_to_chat_completion;
    use super::convert_openai_chat_request_to_codex;
    use super::extract_completed_response_from_sse;
    use super::normalize_openai_responses_request;
    use super::parse_proxy_request_body_limit_mib;
    use super::resolve_proxy_request_body_limit_bytes_from_mib_value;
    use super::rewrite_response_models_for_client;
    use super::rewrite_sse_event_data_models_for_client;
    use super::translate_sse_event_to_chat_chunk;
    use super::ChatStreamState;
    use super::SseEvent;
    use super::DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES;
    use serde_json::json;

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
    fn rejects_chat_request_with_non_alias_gpt_5_4_name() {
        let request = json!({
            "model": "gpt-5.4",
            "messages": [
                { "role": "user", "content": "hello" }
            ]
        });

        let error = convert_openai_chat_request_to_codex(&request)
            .expect_err("request should require gpt-5-4 alias");

        assert!(error.contains("gpt-5-4"));
    }

    #[test]
    fn rejects_responses_request_with_legacy_gpt5_4_name() {
        let request = json!({
            "model": "gpt5.4",
            "input": "hello"
        });

        let error = normalize_openai_responses_request(request)
            .expect_err("request should require gpt-5-4 alias");

        assert!(error.contains("gpt-5-4"));
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
