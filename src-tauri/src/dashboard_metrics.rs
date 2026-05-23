use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::utils::now_unix_seconds;

const METRICS_FILE_NAME: &str = "api-proxy-metrics.jsonl";
const RETENTION_SECONDS: i64 = 7 * 24 * 60 * 60;
const WINDOW_10M_SECONDS: i64 = 10 * 60;
const WINDOW_1H_SECONDS: i64 = 60 * 60;
const WINDOW_24H_SECONDS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardTokenUsage {
    pub(crate) input_tokens: u64,
    pub(crate) cached_input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) reasoning_output_tokens: u64,
    pub(crate) total_tokens: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardMetricEvent {
    pub(crate) finished_at: i64,
    pub(crate) endpoint: String,
    pub(crate) model: Option<String>,
    pub(crate) account_label: Option<String>,
    pub(crate) status_code: Option<u16>,
    pub(crate) error_kind: Option<String>,
    pub(crate) total_ms: u64,
    pub(crate) upstream_headers_ms: Option<u64>,
    pub(crate) first_chunk_ms: Option<u64>,
    pub(crate) stream_ms: Option<u64>,
    pub(crate) request_bytes: Option<u64>,
    pub(crate) downstream_stream: Option<bool>,
    pub(crate) failure_category: Option<String>,
    pub(crate) failure_brief: Option<String>,
    pub(crate) tokens: DashboardTokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardInFlightRequest {
    pub(crate) id: String,
    pub(crate) started_at: i64,
    pub(crate) endpoint: String,
    pub(crate) model: Option<String>,
    pub(crate) account_label: Option<String>,
    pub(crate) phase: String,
    pub(crate) elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardSnapshot {
    pub(crate) data_dir: String,
    pub(crate) metrics_path: String,
    pub(crate) updated_at: i64,
    pub(crate) last_10m: DashboardWindowStats,
    pub(crate) last_1h: DashboardWindowStats,
    pub(crate) last_24h: DashboardWindowStats,
    pub(crate) in_flight: Vec<DashboardInFlightRequest>,
    pub(crate) recent_requests: Vec<DashboardMetricEvent>,
    pub(crate) recent_failures: Vec<DashboardMetricEvent>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardWindowStats {
    pub(crate) window_seconds: i64,
    pub(crate) request_count: usize,
    pub(crate) success_count: usize,
    pub(crate) failure_count: usize,
    pub(crate) failure_rate: f64,
    pub(crate) cache_hit_rate: Option<f64>,
    pub(crate) latency: DashboardLatencyStats,
    pub(crate) tokens: DashboardTokenUsage,
    pub(crate) status_codes: BTreeMap<String, usize>,
    pub(crate) top_models: Vec<DashboardDimensionStat>,
    pub(crate) top_accounts: Vec<DashboardDimensionStat>,
    pub(crate) top_endpoints: Vec<DashboardDimensionStat>,
    pub(crate) timeline: Vec<DashboardTimelineBucket>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardLatencyStats {
    pub(crate) total_p50_ms: Option<u64>,
    pub(crate) total_p90_ms: Option<u64>,
    pub(crate) total_p95_ms: Option<u64>,
    pub(crate) upstream_headers_p95_ms: Option<u64>,
    pub(crate) first_chunk_p95_ms: Option<u64>,
    pub(crate) stream_p95_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardDimensionStat {
    pub(crate) label: String,
    pub(crate) request_count: usize,
    pub(crate) failure_count: usize,
    pub(crate) total_tokens: u64,
    pub(crate) total_p95_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardTimelineBucket {
    pub(crate) start_at: i64,
    pub(crate) request_count: usize,
    pub(crate) failure_count: usize,
    pub(crate) total_tokens: u64,
    pub(crate) total_p95_ms: Option<u64>,
    pub(crate) first_chunk_p95_ms: Option<u64>,
}

#[derive(Debug, Clone)]
struct InFlightState {
    started_instant: std::time::Instant,
    started_at: i64,
    endpoint: String,
    model: Option<String>,
    account_label: Option<String>,
    phase: String,
}

static IN_FLIGHT: OnceLock<Mutex<HashMap<String, InFlightState>>> = OnceLock::new();

pub(crate) fn metrics_path(data_dir: &Path) -> PathBuf {
    data_dir.join("dashboard").join(METRICS_FILE_NAME)
}

pub(crate) fn load_dashboard_snapshot(data_dir: &Path) -> DashboardSnapshot {
    let now = now_unix_seconds();
    let events = read_metric_events(data_dir)
        .into_iter()
        .filter(|event| event.finished_at >= now.saturating_sub(RETENTION_SECONDS))
        .collect::<Vec<_>>();
    build_dashboard_snapshot(
        data_dir.to_path_buf(),
        events,
        current_in_flight_requests(),
        now,
    )
}

pub(crate) fn record_metric_event(data_dir: &Path, event: DashboardMetricEvent) {
    let path = metrics_path(data_dir);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    prune_metric_file(data_dir, now_unix_seconds());
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        if let Ok(line) = serde_json::to_string(&event) {
            let _ = writeln!(file, "{line}");
        }
    }
}

pub(crate) fn begin_in_flight_request(
    endpoint: impl Into<String>,
    model: Option<String>,
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let state = InFlightState {
        started_instant: std::time::Instant::now(),
        started_at: now_unix_seconds(),
        endpoint: endpoint.into(),
        model,
        account_label: None,
        phase: "request_received".to_string(),
    };
    if let Ok(mut guard) = in_flight().lock() {
        guard.insert(id.clone(), state);
    }
    id
}

pub(crate) fn update_in_flight_phase(id: &str, phase: impl Into<String>) {
    if let Ok(mut guard) = in_flight().lock() {
        if let Some(state) = guard.get_mut(id) {
            state.phase = phase.into();
        }
    }
}

pub(crate) fn update_in_flight_model(id: &str, model: Option<String>) {
    if let Ok(mut guard) = in_flight().lock() {
        if let Some(state) = guard.get_mut(id) {
            state.model = model;
        }
    }
}

pub(crate) fn update_in_flight_account(id: &str, account_label: Option<String>) {
    if let Ok(mut guard) = in_flight().lock() {
        if let Some(state) = guard.get_mut(id) {
            state.account_label = account_label;
        }
    }
}

pub(crate) fn finish_in_flight_request(id: &str) {
    if let Ok(mut guard) = in_flight().lock() {
        guard.remove(id);
    }
}

pub(crate) fn token_usage_from_response_usage(usage: Option<&Value>) -> DashboardTokenUsage {
    let Some(usage) = usage else {
        return DashboardTokenUsage::default();
    };
    let input_tokens = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));
    let cached_input_tokens = usage
        .get("input_tokens_details")
        .and_then(|value| value.get("cached_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let reasoning_output_tokens = usage
        .get("output_tokens_details")
        .and_then(|value| value.get("reasoning_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    DashboardTokenUsage {
        input_tokens,
        cached_input_tokens,
        output_tokens,
        reasoning_output_tokens,
        total_tokens,
    }
}

pub(crate) fn token_usage_from_sse_data(data: &str) -> Option<DashboardTokenUsage> {
    let parsed = serde_json::from_str::<Value>(data).ok()?;
    let usage = parsed
        .get("response")
        .and_then(|value| value.get("usage"))?;
    Some(token_usage_from_response_usage(Some(usage)))
}

pub(crate) fn build_dashboard_snapshot(
    data_dir: PathBuf,
    mut events: Vec<DashboardMetricEvent>,
    mut in_flight: Vec<DashboardInFlightRequest>,
    now: i64,
) -> DashboardSnapshot {
    events.sort_by_key(|event| event.finished_at);
    in_flight.sort_by_key(|request| request.started_at);
    let metrics = metrics_path(&data_dir);
    DashboardSnapshot {
        data_dir: data_dir.to_string_lossy().to_string(),
        metrics_path: metrics.to_string_lossy().to_string(),
        updated_at: now,
        last_10m: window_stats(&events, now, WINDOW_10M_SECONDS),
        last_1h: window_stats(&events, now, WINDOW_1H_SECONDS),
        last_24h: window_stats(&events, now, WINDOW_24H_SECONDS),
        in_flight,
        recent_requests: recent_requests(&events),
        recent_failures: recent_failures(&events),
    }
}

fn read_metric_events(data_dir: &Path) -> Vec<DashboardMetricEvent> {
    let path = metrics_path(data_dir);
    let Ok(file) = fs::File::open(path) else {
        return Vec::new();
    };
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<DashboardMetricEvent>(&line).ok())
        .collect()
}

fn prune_metric_file(data_dir: &Path, now: i64) {
    let path = metrics_path(data_dir);
    if !path.exists() {
        return;
    }
    let events = read_metric_events(data_dir)
        .into_iter()
        .filter(|event| event.finished_at >= now.saturating_sub(RETENTION_SECONDS))
        .collect::<Vec<_>>();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = fs::File::create(path) {
        for event in events {
            if let Ok(line) = serde_json::to_string(&event) {
                let _ = writeln!(file, "{line}");
            }
        }
    }
}

fn window_stats(
    events: &[DashboardMetricEvent],
    now: i64,
    window_seconds: i64,
) -> DashboardWindowStats {
    let start = now.saturating_sub(window_seconds);
    let window_events = events
        .iter()
        .filter(|event| event.finished_at >= start)
        .collect::<Vec<_>>();
    let request_count = window_events.len();
    let failure_count = window_events
        .iter()
        .filter(|event| is_failure(event))
        .count();
    let success_count = request_count.saturating_sub(failure_count);
    let mut status_codes = BTreeMap::<String, usize>::new();
    let mut tokens = DashboardTokenUsage::default();
    let mut total_latencies = Vec::new();
    let mut upstream_latencies = Vec::new();
    let mut first_chunk_latencies = Vec::new();
    let mut stream_latencies = Vec::new();

    for event in &window_events {
        let key = event
            .status_code
            .map(|status| status.to_string())
            .or_else(|| event.error_kind.clone())
            .unwrap_or_else(|| "unknown".to_string());
        *status_codes.entry(key).or_insert(0) += 1;
        tokens.add(&event.tokens);
        total_latencies.push(event.total_ms);
        if let Some(value) = event.upstream_headers_ms {
            upstream_latencies.push(value);
        }
        if let Some(value) = event.first_chunk_ms {
            first_chunk_latencies.push(value);
        }
        if let Some(value) = event.stream_ms {
            stream_latencies.push(value);
        }
    }

    let failure_rate = if request_count == 0 {
        0.0
    } else {
        failure_count as f64 / request_count as f64
    };
    let cache_hit_rate = if tokens.input_tokens == 0 {
        None
    } else {
        Some(tokens.cached_input_tokens as f64 / tokens.input_tokens as f64)
    };

    DashboardWindowStats {
        window_seconds,
        request_count,
        success_count,
        failure_count,
        failure_rate,
        cache_hit_rate,
        latency: DashboardLatencyStats {
            total_p50_ms: percentile(&mut total_latencies, 0.5),
            total_p90_ms: percentile(&mut total_latencies, 0.9),
            total_p95_ms: percentile(&mut total_latencies, 0.95),
            upstream_headers_p95_ms: percentile(&mut upstream_latencies, 0.95),
            first_chunk_p95_ms: percentile(&mut first_chunk_latencies, 0.95),
            stream_p95_ms: percentile(&mut stream_latencies, 0.95),
        },
        tokens,
        status_codes,
        top_models: dimension_stats(&window_events, |event| {
            event.model.as_deref().unwrap_or("unknown")
        }),
        top_accounts: dimension_stats(&window_events, |event| {
            event.account_label.as_deref().unwrap_or("unknown")
        }),
        top_endpoints: dimension_stats(&window_events, |event| event.endpoint.as_str()),
        timeline: timeline_buckets(&window_events, now, window_seconds),
    }
}

fn dimension_stats<'a>(
    events: &[&'a DashboardMetricEvent],
    label_for: impl Fn(&'a DashboardMetricEvent) -> &'a str,
) -> Vec<DashboardDimensionStat> {
    let mut grouped = BTreeMap::<String, Vec<&DashboardMetricEvent>>::new();
    for event in events {
        grouped
            .entry(label_for(event).to_string())
            .or_default()
            .push(*event);
    }
    let mut stats = grouped
        .into_iter()
        .map(|(label, group)| {
            let mut total_latencies = group.iter().map(|event| event.total_ms).collect::<Vec<_>>();
            DashboardDimensionStat {
                label,
                request_count: group.len(),
                failure_count: group.iter().filter(|event| is_failure(event)).count(),
                total_tokens: group.iter().map(|event| event.tokens.total_tokens).sum(),
                total_p95_ms: percentile(&mut total_latencies, 0.95),
            }
        })
        .collect::<Vec<_>>();
    stats.sort_by(|left, right| {
        right
            .request_count
            .cmp(&left.request_count)
            .then_with(|| right.total_tokens.cmp(&left.total_tokens))
            .then_with(|| left.label.cmp(&right.label))
    });
    stats.truncate(8);
    stats
}

fn recent_requests(events: &[DashboardMetricEvent]) -> Vec<DashboardMetricEvent> {
    events.iter().rev().take(20).cloned().collect()
}

fn recent_failures(events: &[DashboardMetricEvent]) -> Vec<DashboardMetricEvent> {
    events
        .iter()
        .rev()
        .filter(|event| is_failure(event))
        .take(20)
        .cloned()
        .collect()
}

fn timeline_buckets(
    events: &[&DashboardMetricEvent],
    now: i64,
    window_seconds: i64,
) -> Vec<DashboardTimelineBucket> {
    let bucket_count: i64 = 24;
    let bucket_seconds = (window_seconds / bucket_count).max(1);
    let start = now.saturating_sub(window_seconds);
    let aligned_start = start - start.rem_euclid(bucket_seconds);
    (0..bucket_count)
        .map(|index| {
            let bucket_start = aligned_start + index * bucket_seconds;
            let bucket_end = bucket_start + bucket_seconds;
            let bucket_events = events
                .iter()
                .copied()
                .filter(|event| event.finished_at >= bucket_start && event.finished_at < bucket_end)
                .collect::<Vec<_>>();
            let mut total_latencies = bucket_events
                .iter()
                .map(|event| event.total_ms)
                .collect::<Vec<_>>();
            let mut first_chunk_latencies = bucket_events
                .iter()
                .filter_map(|event| event.first_chunk_ms)
                .collect::<Vec<_>>();
            DashboardTimelineBucket {
                start_at: bucket_start,
                request_count: bucket_events.len(),
                failure_count: bucket_events
                    .iter()
                    .filter(|event| is_failure(event))
                    .count(),
                total_tokens: bucket_events
                    .iter()
                    .map(|event| event.tokens.total_tokens)
                    .sum(),
                total_p95_ms: percentile(&mut total_latencies, 0.95),
                first_chunk_p95_ms: percentile(&mut first_chunk_latencies, 0.95),
            }
        })
        .collect()
}

fn is_failure(event: &DashboardMetricEvent) -> bool {
    match event.error_kind.as_deref() {
        Some("client_disconnected_after_first_chunk") => return false,
        Some("client_disconnected")
            if event.upstream_headers_ms.is_some() && event.first_chunk_ms.is_some() =>
        {
            return false;
        }
        _ => {}
    }

    event
        .status_code
        .map(|status| status >= 400)
        .unwrap_or(true)
        || event.error_kind.is_some()
}

fn percentile(values: &mut [u64], percentile: f64) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    if values.len() == 1 {
        return Some(values[0]);
    }
    let position = percentile * (values.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        return Some(values[lower]);
    }
    let lower_value = values[lower] as f64;
    let upper_value = values[upper] as f64;
    Some((lower_value + (upper_value - lower_value) * (position - lower as f64)).round() as u64)
}

fn in_flight() -> &'static Mutex<HashMap<String, InFlightState>> {
    IN_FLIGHT.get_or_init(|| Mutex::new(HashMap::new()))
}

fn current_in_flight_requests() -> Vec<DashboardInFlightRequest> {
    let Ok(guard) = in_flight().lock() else {
        return Vec::new();
    };
    guard
        .iter()
        .map(|(id, state)| DashboardInFlightRequest {
            id: id.clone(),
            started_at: state.started_at,
            endpoint: state.endpoint.clone(),
            model: state.model.clone(),
            account_label: state.account_label.clone(),
            phase: state.phase.clone(),
            elapsed_ms: state.started_instant.elapsed().as_millis() as u64,
        })
        .collect()
}

impl DashboardTokenUsage {
    fn add(&mut self, other: &DashboardTokenUsage) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(other.cached_input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.reasoning_output_tokens = self
            .reasoning_output_tokens
            .saturating_add(other.reasoning_output_tokens);
        self.total_tokens = self.total_tokens.saturating_add(other.total_tokens);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn aggregates_windows_and_cache_hit_rate() {
        let now = 1_000;
        let events = vec![
            DashboardMetricEvent {
                finished_at: 995,
                endpoint: "/v1/responses".to_string(),
                model: Some("gpt-5.4".to_string()),
                account_label: Some("acct-a".to_string()),
                status_code: Some(200),
                error_kind: None,
                total_ms: 1_000,
                upstream_headers_ms: Some(600),
                first_chunk_ms: Some(650),
                stream_ms: Some(350),
                request_bytes: Some(123),
                downstream_stream: Some(true),
                failure_category: None,
                failure_brief: None,
                tokens: DashboardTokenUsage {
                    input_tokens: 100,
                    cached_input_tokens: 25,
                    output_tokens: 20,
                    reasoning_output_tokens: 5,
                    total_tokens: 120,
                },
            },
            DashboardMetricEvent {
                finished_at: 930,
                endpoint: "/v1/responses".to_string(),
                model: Some("gpt-5.5".to_string()),
                account_label: Some("acct-b".to_string()),
                status_code: Some(401),
                error_kind: Some("auth".to_string()),
                total_ms: 100,
                upstream_headers_ms: None,
                first_chunk_ms: None,
                stream_ms: None,
                request_bytes: Some(88),
                downstream_stream: Some(false),
                failure_category: Some("auth_failed".to_string()),
                failure_brief: Some("invalid api key".to_string()),
                tokens: DashboardTokenUsage::default(),
            },
        ];

        let dashboard = build_dashboard_snapshot(
            PathBuf::from(r"C:\portable\Codex Tools Data"),
            events,
            Vec::new(),
            now,
        );

        assert_eq!(dashboard.data_dir, r"C:\portable\Codex Tools Data");
        assert_eq!(dashboard.last_10m.request_count, 2);
        assert_eq!(dashboard.last_10m.failure_count, 1);
        assert_eq!(dashboard.last_10m.status_codes.get("401").copied(), Some(1));
        assert_eq!(dashboard.last_10m.cache_hit_rate, Some(0.25));
        assert_eq!(dashboard.last_10m.latency.total_p50_ms, Some(550));
        assert_eq!(dashboard.last_10m.timeline.len(), 24);
        assert_eq!(
            dashboard
                .last_10m
                .timeline
                .iter()
                .map(|bucket| bucket.request_count)
                .sum::<usize>(),
            2
        );
    }

    #[test]
    fn metric_event_serializes_proxy_failure_context() {
        let event = DashboardMetricEvent {
            finished_at: 1,
            endpoint: "/v1/responses".to_string(),
            model: Some("gpt-5.5".to_string()),
            account_label: None,
            status_code: Some(400),
            error_kind: Some("upstream_invalid_request".to_string()),
            total_ms: 42,
            upstream_headers_ms: None,
            first_chunk_ms: None,
            stream_ms: None,
            request_bytes: Some(123),
            downstream_stream: Some(false),
            failure_category: Some("invalid_request".to_string()),
            failure_brief: Some("Unknown parameter: 'foo'.".to_string()),
            tokens: DashboardTokenUsage::default(),
        };

        let value = serde_json::to_value(&event).expect("metric event should serialize");

        assert_eq!(value.get("requestBytes").and_then(Value::as_u64), Some(123));
        assert_eq!(
            value.get("downstreamStream").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            value.get("failureCategory").and_then(Value::as_str),
            Some("invalid_request")
        );
        assert_eq!(
            value.get("failureBrief").and_then(Value::as_str),
            Some("Unknown parameter: 'foo'.")
        );
    }

    #[test]
    fn finishing_in_flight_request_removes_it_from_snapshot() {
        let id = begin_in_flight_request("/v1/responses", Some("gpt-5.4".to_string()));
        update_in_flight_phase(&id, "first_upstream_chunk");

        let snapshot_before = build_dashboard_snapshot(
            PathBuf::from(r"C:\portable\Codex Tools Data"),
            Vec::new(),
            current_in_flight_requests(),
            1_000,
        );
        assert_eq!(snapshot_before.in_flight.len(), 1);
        assert_eq!(snapshot_before.in_flight[0].elapsed_ms, 0);

        finish_in_flight_request(&id);

        let snapshot_after = build_dashboard_snapshot(
            PathBuf::from(r"C:\portable\Codex Tools Data"),
            Vec::new(),
            current_in_flight_requests(),
            1_000,
        );
        assert!(snapshot_after.in_flight.is_empty());
    }

    #[test]
    fn dashboard_keeps_latest_requests_without_slow_filter() {
        let now = 2_000;
        let events = vec![
            DashboardMetricEvent {
                finished_at: 1_990,
                endpoint: "/v1/models".to_string(),
                model: None,
                account_label: None,
                status_code: Some(200),
                error_kind: None,
                total_ms: 10,
                upstream_headers_ms: Some(5),
                first_chunk_ms: None,
                stream_ms: None,
                request_bytes: Some(42),
                downstream_stream: Some(false),
                failure_category: None,
                failure_brief: None,
                tokens: DashboardTokenUsage::default(),
            },
            DashboardMetricEvent {
                finished_at: 1_995,
                endpoint: "/v1/responses".to_string(),
                model: Some("gpt-5.5".to_string()),
                account_label: Some("acct-a".to_string()),
                status_code: Some(200),
                error_kind: None,
                total_ms: 20,
                upstream_headers_ms: Some(10),
                first_chunk_ms: Some(12),
                stream_ms: None,
                request_bytes: Some(43),
                downstream_stream: Some(true),
                failure_category: None,
                failure_brief: None,
                tokens: DashboardTokenUsage::default(),
            },
        ];

        let dashboard = build_dashboard_snapshot(
            PathBuf::from(r"C:\portable\Codex Tools Data"),
            events,
            Vec::new(),
            now,
        );

        assert_eq!(dashboard.recent_requests.len(), 2);
        assert_eq!(dashboard.recent_requests[0].finished_at, 1_995);
        assert_eq!(dashboard.recent_requests[1].finished_at, 1_990);
    }

    #[test]
    fn streamed_client_disconnect_is_not_counted_as_failure() {
        let now = 3_000;
        let events = vec![
            DashboardMetricEvent {
                finished_at: 2_990,
                endpoint: "/v1/responses".to_string(),
                model: Some("gpt-5.5".to_string()),
                account_label: Some("acct-a".to_string()),
                status_code: None,
                error_kind: Some("client_disconnected_after_first_chunk".to_string()),
                total_ms: 10_000,
                upstream_headers_ms: Some(2_000),
                first_chunk_ms: Some(2_100),
                stream_ms: Some(7_900),
                request_bytes: Some(200),
                downstream_stream: Some(true),
                failure_category: Some("client_disconnected".to_string()),
                failure_brief: Some("client disconnected after first chunk".to_string()),
                tokens: DashboardTokenUsage::default(),
            },
            DashboardMetricEvent {
                finished_at: 2_995,
                endpoint: "/v1/responses".to_string(),
                model: Some("gpt-5.5".to_string()),
                account_label: None,
                status_code: None,
                error_kind: Some("client_disconnected".to_string()),
                total_ms: 120_000,
                upstream_headers_ms: None,
                first_chunk_ms: None,
                stream_ms: None,
                request_bytes: Some(201),
                downstream_stream: Some(true),
                failure_category: Some("client_disconnected".to_string()),
                failure_brief: Some("client disconnected".to_string()),
                tokens: DashboardTokenUsage::default(),
            },
        ];

        let dashboard = build_dashboard_snapshot(
            PathBuf::from(r"C:\portable\Codex Tools Data"),
            events,
            Vec::new(),
            now,
        );

        assert_eq!(dashboard.last_10m.failure_count, 1);
        assert_eq!(dashboard.recent_failures.len(), 1);
        assert_eq!(dashboard.recent_failures[0].total_ms, 120_000);
    }
}
