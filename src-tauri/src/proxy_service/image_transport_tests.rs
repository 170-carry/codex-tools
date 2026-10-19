use super::*;

struct RecordedImageRequest {
    path: String,
    headers: HeaderMap,
    body: Value,
}

#[derive(Clone)]
struct ImageMockState {
    requests: tokio::sync::mpsc::UnboundedSender<RecordedImageRequest>,
    response_body: String,
}

struct ImageMock {
    base_url: String,
    requests: tokio::sync::mpsc::UnboundedReceiver<RecordedImageRequest>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for ImageMock {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl ImageMock {
    async fn start(response_body: &str) -> Self {
        async fn capture(State(state): State<ImageMockState>, request: Request) -> Response<Body> {
            let path = request.uri().path().to_string();
            let headers = request.headers().clone();
            let bytes = axum::body::to_bytes(request.into_body(), 1024 * 1024)
                .await
                .expect("mock request body");
            state
                .requests
                .send(RecordedImageRequest {
                    path,
                    headers,
                    body: serde_json::from_slice(&bytes).expect("upstream JSON request"),
                })
                .expect("capture upstream request");
            Response::builder()
                .header("content-type", "application/json")
                .body(Body::from(state.response_body))
                .expect("mock response")
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local image mock");
        let base_url = format!("http://{}", listener.local_addr().expect("mock address"));
        let (sender, requests) = tokio::sync::mpsc::unbounded_channel();
        let app = axum::Router::new()
            .fallback(capture)
            .with_state(ImageMockState {
                requests: sender,
                response_body: response_body.to_string(),
            });
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve image mock");
        });
        Self {
            base_url,
            requests,
            server,
        }
    }

    async fn request(&mut self) -> RecordedImageRequest {
        tokio::time::timeout(std::time::Duration::from_secs(5), self.requests.recv())
            .await
            .expect("upstream request timeout")
            .expect("upstream request captured")
    }

    fn context(&self) -> ProxyContext {
        ProxyContext {
            storage: ProxyStorageContext {
                // These responses intentionally contain no usage, so the storage
                // path is unique but never created and needs no filesystem cleanup.
                data_dir: std::env::temp_dir().join(format!(
                    "codex-image-transport-test-{}",
                    uuid::Uuid::new_v4()
                )),
                store_lock: Arc::new(tokio::sync::Mutex::new(())),
                auth_refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
                usage_writer: Arc::new(tokio::sync::Mutex::new(None)),
                sync_active_auth_on_refresh: false,
            },
            api_keys: Arc::new(RwLock::new(Vec::new())),
            upstream_base_url: format!("{}/backend-api/codex", self.base_url),
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("local-only test client"),
            shared: Arc::new(tokio::sync::Mutex::new(ApiProxyRuntimeSnapshot::default())),
            request_body_limit_bytes: DEFAULT_PROXY_REQUEST_BODY_LIMIT_BYTES,
        }
    }
}

fn image_chatgpt_candidate() -> ProxyCandidate {
    ProxyCandidate {
        priority: 0,
        id: "image-account".to_string(),
        label: "image mock".to_string(),
        source_kind: AccountSourceKind::Chatgpt,
        account_key: "image-account".to_string(),
        account_id: "image-account".to_string(),
        access_token: "mock-access-token".to_string(),
        auth_json: json!({}),
        api_base_url: None,
        model_name: None,
        variant_key: "image-account".to_string(),
        plan_type: None,
        usage: None,
        auth_refresh_blocked: false,
        auth_refresh_error: None,
        updated_at: 1,
    }
}

fn image_test_headers() -> HeaderMap {
    HeaderMap::from_iter([(
        axum::http::HeaderName::from_static("session-id"),
        axum::http::HeaderValue::from_static("image-test-turn"),
    )])
}

fn image_usage_metadata() -> ApiProxyUsageMetadata {
    ApiProxyUsageMetadata {
        key_id: None,
        key_label: None,
        model: "gpt-image-2".to_string(),
        route: "/v1/images/generations".to_string(),
        reasoning_effort: None,
        service_tier: None,
    }
}

fn assert_native_image_headers(request: &RecordedImageRequest) {
    assert_eq!(request.headers["accept"], "application/json");
    assert_eq!(request.headers["content-type"], "application/json");
    assert_eq!(request.headers["x-codex-image-turn-id"], "image-test-turn");
    assert_eq!(request.headers["chatgpt-account-id"], "image-account");
    assert!(!request.headers.contains_key(RESPONSES_LITE_HEADER));
    assert_eq!(request.body["model"], "gpt-image-2");
    assert!(request.body.get("tools").is_none());
    assert!(request.body.get("reasoning").is_none());
    assert!(request.body.get("stream").is_none());
}

#[tokio::test]
async fn image_generation_sends_native_model_and_json_contract() {
    let mut mock = ImageMock::start(r#"{"created":42,"data":[{"b64_json":"image"}]}"#).await;
    let converted = convert_openai_image_generation_request_to_codex(&json!({
        "model": "gpt-image-2",
        "prompt": "Draw a circle",
        "quality": "high",
        "size": "1024x1024",
        "n": 2,
    }))
    .expect("convert generation");
    assert_eq!(converted.image_count, 2);

    let response = forward_codex_request_with_candidate(
        &mock.context(),
        &image_chatgpt_candidate(),
        &image_test_headers(),
        &converted.upstream_payload,
        CodexUpstreamEndpoint::ImageGeneration,
    )
    .await
    .expect("forward native generation");
    assert_eq!(response.status(), StatusCode::OK);
    let request = mock.request().await;
    assert_eq!(request.path, "/backend-api/codex/images/generations");
    assert_native_image_headers(&request);
    assert_eq!(request.body["prompt"], "Draw a circle");
    assert_eq!(request.body["size"], "1024x1024");
    assert_eq!(request.body["quality"], "high");
    assert_eq!(request.body["n"], 1, "proxy owns the multi-image loop");
    let (_, body) = response.into_bytes().await.expect("native JSON response");
    assert_eq!(parse_native_image_response(&body).unwrap()["created"], 42);
}

#[tokio::test]
async fn image_edit_and_variation_use_native_edits_and_preserve_references() {
    let mut mock = ImageMock::start(r#"{"data":[{"b64_json":"edited"}]}"#).await;
    for variation in [false, true] {
        let multipart = ImageMultipartRequest {
            fields: Map::from_iter([
                ("model".to_string(), json!("gpt-image-2")),
                ("prompt".to_string(), json!("Change the hat")),
            ]),
            images: vec![json!({
                "type": "input_image",
                "image_url": "data:image/png;base64,c291cmNl",
                "detail": "high",
            })],
            mask: None,
        };
        let converted = convert_openai_image_edit_request_to_codex(&multipart, variation)
            .expect("convert edit or variation");
        forward_codex_request_with_candidate(
            &mock.context(),
            &image_chatgpt_candidate(),
            &image_test_headers(),
            &converted.upstream_payload,
            CodexUpstreamEndpoint::ImageEdit,
        )
        .await
        .expect("forward native edit");
        let request = mock.request().await;
        assert_eq!(request.path, "/backend-api/codex/images/edits");
        assert_native_image_headers(&request);
        assert_eq!(
            request.body["prompt"],
            if variation {
                IMAGE_VARIATION_PROMPT
            } else {
                "Change the hat"
            }
        );
        assert_eq!(
            request.body["images"],
            json!([{ "image_url": "data:image/png;base64,c291cmNl" }])
        );
    }
}

#[tokio::test]
async fn image_relay_preserves_responses_transport_and_controller_override() {
    let mut mock = ImageMock::start("{}").await;
    let mut candidate = image_chatgpt_candidate();
    candidate.source_kind = AccountSourceKind::Relay;
    candidate.api_base_url = Some(format!("{}/v1", mock.base_url));
    candidate.model_name = Some("relay-image-controller".to_string());
    let converted = convert_openai_image_generation_request_to_codex(&json!({
        "model": "gpt-image-2",
        "prompt": "Draw a circle",
    }))
    .expect("convert generation");

    for endpoint in [
        CodexUpstreamEndpoint::ImageGeneration,
        CodexUpstreamEndpoint::ImageEdit,
    ] {
        forward_codex_request_with_candidate(
            &mock.context(),
            &candidate,
            &image_test_headers(),
            &converted.upstream_payload,
            endpoint,
        )
        .await
        .expect("forward relay image");
        let request = mock.request().await;
        assert_eq!(request.path, "/v1/responses");
        assert_eq!(request.headers["accept"], "text/event-stream");
        assert!(!request.headers.contains_key("x-codex-image-turn-id"));
        assert!(!request.headers.contains_key("chatgpt-account-id"));
        assert_eq!(request.body["model"], "relay-image-controller");
        assert_eq!(request.body["tools"][0]["model"], "gpt-image-2");
        assert_eq!(request.body["tools"][0]["type"], "image_generation");
        assert_eq!(request.body["stream"], true);
    }
}

#[tokio::test]
async fn image_transport_changes_leave_normal_responses_unchanged() {
    let mut mock = ImageMock::start("{}").await;
    let payload = json!({
        "model": "gpt-5.6-sol",
        "input": [{"type": "message", "role": "user", "content": "hello"}],
        "stream": true,
    });
    forward_codex_request_with_candidate(
        &mock.context(),
        &image_chatgpt_candidate(),
        &image_test_headers(),
        &payload,
        CodexUpstreamEndpoint::Responses,
    )
    .await
    .expect("forward regular responses");
    let request = mock.request().await;
    assert_eq!(request.path, "/backend-api/codex/responses");
    assert_eq!(request.headers["accept"], "text/event-stream");
    assert_eq!(request.headers[RESPONSES_LITE_HEADER], "true");
    assert!(!request.headers.contains_key("x-codex-image-turn-id"));
    assert_eq!(request.body, payload);
}

#[tokio::test]
async fn image_native_json_stream_emits_one_completion_and_done() {
    let mut mock = ImageMock::start(
        r#"{"created":42,"data":[{"b64_json":"aW1hZ2U=","generation_id":"gen-1"}]}"#,
    )
    .await;
    let context = mock.context();
    let payload = convert_openai_image_generation_request_to_codex(&json!({
        "model": "gpt-image-2", "prompt": "Draw a circle", "stream": true,
    }))
    .expect("convert generation")
    .upstream_payload;
    let upstream = forward_codex_request_with_candidate(
        &context,
        &image_chatgpt_candidate(),
        &image_test_headers(),
        &payload,
        CodexUpstreamEndpoint::ImageGeneration,
    )
    .await
    .expect("native JSON upstream");
    mock.request().await;
    let response = build_native_image_streaming_response(
        upstream,
        context.storage.clone(),
        image_usage_metadata(),
    );
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "text/event-stream; charset=utf-8"
    );
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("downstream SSE body");
    let text = std::str::from_utf8(&body).expect("SSE UTF-8");
    let events: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .collect();
    assert_eq!(
        events.len(),
        2,
        "one final image and one DONE event: {text}"
    );
    assert_eq!(events[1], "[DONE]");
    let completed: Value = serde_json::from_str(events[0]).expect("completion JSON");
    assert_eq!(completed["type"], "image_generation.completed");
    assert_eq!(completed["image"]["created"], 42);
    assert_eq!(completed["image"]["data"][0]["b64_json"], "aW1hZ2U=");
    assert_eq!(completed["image"]["data"][0]["generation_id"], "gen-1");
    assert!(!context.storage.data_dir.exists());
}

#[tokio::test]
async fn image_native_bad_response_stream_reports_error_without_completion() {
    for (body, expected_message) in [
        ("not JSON", None),
        (r#"{"data":[]}"#, None),
        (
            r#"{"error":{"message":"image rejected"}}"#,
            Some("image rejected"),
        ),
        (r#"{"detail":"native denied"}"#, Some("native denied")),
    ] {
        let mut mock = ImageMock::start(body).await;
        let context = mock.context();
        let payload = convert_openai_image_generation_request_to_codex(&json!({
            "model": "gpt-image-2", "prompt": "Draw a circle", "stream": true,
        }))
        .expect("convert generation")
        .upstream_payload;
        let upstream = forward_codex_request_with_candidate(
            &context,
            &image_chatgpt_candidate(),
            &image_test_headers(),
            &payload,
            CodexUpstreamEndpoint::ImageGeneration,
        )
        .await
        .expect("native upstream transport");
        mock.request().await;
        let response = build_native_image_streaming_response(
            upstream,
            context.storage.clone(),
            image_usage_metadata(),
        );
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("error SSE body");
        let text = std::str::from_utf8(&bytes).expect("SSE UTF-8");
        let events: Vec<_> = text
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .collect();
        assert_eq!(events.len(), 2, "error plus DONE: {text}");
        assert_eq!(events[1], "[DONE]");
        let error: Value = serde_json::from_str(events[0]).expect("error JSON");
        assert!(error["error"]["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()));
        if let Some(expected_message) = expected_message {
            assert_eq!(error["error"]["message"], expected_message);
        }
        assert!(!text.contains("image_generation.completed"));
        assert!(!context.storage.data_dir.exists());
    }
}
