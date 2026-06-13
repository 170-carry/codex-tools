use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::app_paths;
use crate::utils::now_unix_seconds;

const DAY_SECONDS: i64 = 24 * 60 * 60;
const PROMPT_PREVIEW_CHARS: usize = 220;
const TOP_EXPENSIVE_PROMPT_LIMIT: usize = 20;
const SESSION_EXPORT_LIMIT: usize = 500;
const PRICING_SOURCE: &str = "OpenAI API pricing, text tokens per 1M, checked 2026-06-13";
const COST_ANALYTICS_CACHE_VERSION: u8 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexTokenTotals {
    pub(crate) input_tokens: u64,
    pub(crate) cached_input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) reasoning_output_tokens: u64,
    pub(crate) total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexTokenSessionUsage {
    pub(crate) started_at: Option<i64>,
    pub(crate) updated_at: i64,
    pub(crate) total: CodexTokenTotals,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexTokenUsageSnapshot {
    pub(crate) updated_at: i64,
    pub(crate) source_path_count: usize,
    pub(crate) failed_path_count: usize,
    pub(crate) event_count: usize,
    pub(crate) last_24h: CodexTokenTotals,
    pub(crate) last_3d: CodexTokenTotals,
    pub(crate) last_7d: CodexTokenTotals,
    pub(crate) last_30d: CodexTokenTotals,
    pub(crate) latest_session: Option<CodexTokenSessionUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexCostAnalyticsSnapshot {
    pub(crate) updated_at: i64,
    pub(crate) pricing_source: String,
    pub(crate) source_path_count: usize,
    pub(crate) failed_path_count: usize,
    pub(crate) event_count: usize,
    pub(crate) total: CodexTokenTotals,
    pub(crate) total_cost_usd: f64,
    pub(crate) last_7d: CodexTokenTotals,
    pub(crate) last_7d_cost_usd: f64,
    pub(crate) weekly_budget_usd: Option<f64>,
    pub(crate) weekly_budget_percent: Option<f64>,
    pub(crate) weekly_budget_alert: String,
    pub(crate) projects: Vec<CodexProjectCostBreakdown>,
    pub(crate) sessions: Vec<CodexSessionCostBreakdown>,
    pub(crate) heatmap: Vec<CodexHourlyCostBucket>,
    pub(crate) top_prompts: Vec<CodexPromptCostBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexProjectCostBreakdown {
    pub(crate) project_path: String,
    pub(crate) project_name: String,
    pub(crate) session_count: usize,
    pub(crate) prompt_count: usize,
    pub(crate) event_count: usize,
    pub(crate) total: CodexTokenTotals,
    pub(crate) cost_usd: f64,
    pub(crate) last_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexSessionCostBreakdown {
    pub(crate) session_id: String,
    pub(crate) parent_session_id: Option<String>,
    pub(crate) project_path: String,
    pub(crate) project_name: String,
    pub(crate) started_at: Option<i64>,
    pub(crate) updated_at: Option<i64>,
    pub(crate) duration_seconds: Option<i64>,
    pub(crate) prompt_count: usize,
    pub(crate) event_count: usize,
    pub(crate) model: String,
    pub(crate) total: CodexTokenTotals,
    pub(crate) cost_usd: f64,
    pub(crate) source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexHourlyCostBucket {
    pub(crate) weekday: u8,
    pub(crate) hour: u8,
    pub(crate) calls: usize,
    pub(crate) tokens: u64,
    pub(crate) cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexPromptCostBreakdown {
    pub(crate) session_id: String,
    pub(crate) project_path: String,
    pub(crate) project_name: String,
    pub(crate) timestamp: i64,
    pub(crate) model: String,
    pub(crate) prompt_preview: String,
    pub(crate) prompt_chars: usize,
    pub(crate) total: CodexTokenTotals,
    pub(crate) cost_usd: f64,
    pub(crate) source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexCostAnalyticsProgress {
    pub(crate) stage: String,
    pub(crate) processed_files: usize,
    pub(crate) total_files: usize,
    pub(crate) percent: u8,
    pub(crate) current_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexCostAnalyticsCacheFile {
    version: u8,
    snapshot: CodexCostAnalyticsSnapshot,
}

#[derive(Debug, Clone)]
struct ParsedTokenEvent {
    timestamp: i64,
    last: Option<CodexTokenTotals>,
    total: Option<CodexTokenTotals>,
}

#[derive(Debug, Default)]
struct ParsedSession {
    started_at: Option<i64>,
    updated_at: Option<i64>,
    total: CodexTokenTotals,
    fallback_total: CodexTokenTotals,
}

pub(crate) fn collect_codex_token_usage_snapshot() -> Result<CodexTokenUsageSnapshot, String> {
    let codex_dir = app_paths::codex_dir()?;
    let roots = [
        codex_dir.join("sessions"),
        codex_dir.join("archived_sessions"),
    ];
    Ok(scan_codex_token_usage_roots(&roots, now_unix_seconds()))
}

fn scan_codex_token_usage_roots(roots: &[PathBuf], now: i64) -> CodexTokenUsageSnapshot {
    let mut files = Vec::new();
    let mut failed_path_count = 0;
    for root in roots {
        collect_jsonl_files(root, &mut files, &mut failed_path_count);
    }

    let mut snapshot = CodexTokenUsageSnapshot {
        updated_at: now,
        source_path_count: files.len(),
        failed_path_count,
        event_count: 0,
        last_24h: CodexTokenTotals::default(),
        last_3d: CodexTokenTotals::default(),
        last_7d: CodexTokenTotals::default(),
        last_30d: CodexTokenTotals::default(),
        latest_session: None,
    };

    let last_24h_start = now.saturating_sub(DAY_SECONDS);
    let last_3d_start = now.saturating_sub(3 * DAY_SECONDS);
    let last_7d_start = now.saturating_sub(7 * DAY_SECONDS);
    let last_30d_start = now.saturating_sub(30 * DAY_SECONDS);

    for file in files {
        match parse_token_session_file(&file) {
            Ok(session) => {
                for event in session.events {
                    snapshot.event_count += 1;
                    if let Some(last) = event.last.as_ref() {
                        if event.timestamp >= last_24h_start {
                            snapshot.last_24h.add(last);
                        }
                        if event.timestamp >= last_3d_start {
                            snapshot.last_3d.add(last);
                        }
                        if event.timestamp >= last_7d_start {
                            snapshot.last_7d.add(last);
                        }
                        if event.timestamp >= last_30d_start {
                            snapshot.last_30d.add(last);
                        }
                    }
                }

                if let Some(latest_session) = session.latest_session {
                    let should_replace = snapshot
                        .latest_session
                        .as_ref()
                        .map(|current| latest_session.updated_at > current.updated_at)
                        .unwrap_or(true);
                    if should_replace {
                        snapshot.latest_session = Some(latest_session);
                    }
                }
            }
            Err(_) => {
                snapshot.failed_path_count += 1;
            }
        }
    }

    snapshot
}

pub(crate) fn collect_codex_cost_analytics_snapshot(
    weekly_budget_usd: Option<f64>,
) -> Result<CodexCostAnalyticsSnapshot, String> {
    collect_codex_cost_analytics_snapshot_with_progress(weekly_budget_usd, |_| {})
}

pub(crate) fn collect_codex_cost_analytics_snapshot_with_progress<F>(
    weekly_budget_usd: Option<f64>,
    on_progress: F,
) -> Result<CodexCostAnalyticsSnapshot, String>
where
    F: FnMut(CodexCostAnalyticsProgress),
{
    let codex_dir = app_paths::codex_dir()?;
    let roots = [
        codex_dir.join("sessions"),
        codex_dir.join("archived_sessions"),
    ];
    Ok(scan_codex_cost_analytics_roots_with_progress(
        &roots,
        now_unix_seconds(),
        weekly_budget_usd,
        on_progress,
    ))
}

fn scan_codex_cost_analytics_roots_with_progress<F>(
    roots: &[PathBuf],
    now: i64,
    weekly_budget_usd: Option<f64>,
    mut on_progress: F,
) -> CodexCostAnalyticsSnapshot
where
    F: FnMut(CodexCostAnalyticsProgress),
{
    let mut files = Vec::new();
    let mut failed_path_count = 0;
    for root in roots {
        collect_jsonl_files(root, &mut files, &mut failed_path_count);
    }
    on_progress(cost_analytics_progress("scanning", 0, files.len(), None));

    let last_7d_start = now.saturating_sub(7 * DAY_SECONDS);
    let mut all_events = Vec::new();
    let mut sessions = Vec::new();
    let mut projects = BTreeMap::<String, ProjectAccumulator>::new();
    let mut prompts = BTreeMap::<String, PromptAccumulator>::new();
    let mut heatmap = initial_heatmap();
    let mut total = CodexTokenTotals::default();
    let mut total_cost_usd = 0.0;
    let mut last_7d = CodexTokenTotals::default();
    let mut last_7d_cost_usd = 0.0;

    for (index, file) in files.iter().enumerate() {
        match parse_cost_analytics_session_file(file) {
            Ok(parsed) => {
                let session = parsed.session;
                let project_entry = projects
                    .entry(session.project_path.clone())
                    .or_insert_with(|| ProjectAccumulator::new(&session.project_path));
                project_entry.session_count += 1;
                project_entry
                    .prompt_keys
                    .extend(parsed.prompt_keys.iter().cloned());
                project_entry.event_count += session.event_count;
                project_entry.total.add(&session.total);
                project_entry.cost_usd += session.cost_usd;
                project_entry.last_at = max_option_i64(project_entry.last_at, session.updated_at);

                total.add(&session.total);
                total_cost_usd += session.cost_usd;
                sessions.push(session);

                for event in parsed.events {
                    if event.timestamp >= last_7d_start {
                        last_7d.add(&event.total);
                        last_7d_cost_usd += event.cost_usd;
                        if let Some(bucket_key) = heatmap_bucket_key(event.timestamp) {
                            if let Some(bucket) = heatmap.get_mut(&bucket_key) {
                                bucket.calls += 1;
                                bucket.tokens =
                                    bucket.tokens.saturating_add(event.total.total_tokens);
                                bucket.cost_usd += event.cost_usd;
                            }
                        }
                    }

                    let prompt_entry = prompts
                        .entry(event.prompt_key.clone())
                        .or_insert_with(|| PromptAccumulator::from_event(&event));
                    prompt_entry.total.add(&event.total);
                    prompt_entry.cost_usd += event.cost_usd;
                    prompt_entry.timestamp = prompt_entry.timestamp.min(event.timestamp);
                    all_events.push(event);
                }
            }
            Err(_) => {
                failed_path_count += 1;
            }
        }
        on_progress(cost_analytics_progress(
            "scanning",
            index + 1,
            files.len(),
            Some(file.to_string_lossy().to_string()),
        ));
    }

    sessions.sort_by(|left, right| {
        right
            .cost_usd
            .partial_cmp(&left.cost_usd)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
    });

    let mut project_breakdowns = projects
        .into_values()
        .map(ProjectAccumulator::into_breakdown)
        .collect::<Vec<_>>();
    project_breakdowns.sort_by(|left, right| {
        right
            .cost_usd
            .partial_cmp(&left.cost_usd)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.last_at.cmp(&left.last_at))
    });

    let mut top_prompts = prompts
        .into_values()
        .map(PromptAccumulator::into_breakdown)
        .collect::<Vec<_>>();
    top_prompts.sort_by(|left, right| {
        right
            .cost_usd
            .partial_cmp(&left.cost_usd)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.timestamp.cmp(&left.timestamp))
    });
    top_prompts.truncate(TOP_EXPENSIVE_PROMPT_LIMIT);

    let snapshot = CodexCostAnalyticsSnapshot {
        updated_at: now,
        pricing_source: PRICING_SOURCE.to_string(),
        source_path_count: files.len(),
        failed_path_count,
        event_count: all_events.len(),
        total,
        total_cost_usd: round_cost(total_cost_usd),
        last_7d,
        last_7d_cost_usd: round_cost(last_7d_cost_usd),
        weekly_budget_usd: None,
        weekly_budget_percent: None,
        weekly_budget_alert: "none".to_string(),
        projects: project_breakdowns,
        sessions: sessions.into_iter().take(SESSION_EXPORT_LIMIT).collect(),
        heatmap: heatmap.into_values().collect(),
        top_prompts,
    };
    apply_codex_cost_analytics_budget(snapshot, weekly_budget_usd)
}

pub(crate) fn apply_codex_cost_analytics_budget(
    mut snapshot: CodexCostAnalyticsSnapshot,
    weekly_budget_usd: Option<f64>,
) -> CodexCostAnalyticsSnapshot {
    let weekly_budget_usd = normalize_budget(weekly_budget_usd);
    let weekly_budget_percent = weekly_budget_usd.map(|budget| {
        if budget <= 0.0 {
            0.0
        } else {
            (snapshot.last_7d_cost_usd / budget) * 100.0
        }
    });
    snapshot.weekly_budget_usd = weekly_budget_usd;
    snapshot.weekly_budget_percent = weekly_budget_percent.map(round_percent);
    snapshot.weekly_budget_alert = weekly_budget_alert(weekly_budget_percent);
    snapshot
}

pub(crate) fn serialize_codex_cost_analytics_cache(
    snapshot: &CodexCostAnalyticsSnapshot,
) -> Result<Vec<u8>, String> {
    let cache = CodexCostAnalyticsCacheFile {
        version: COST_ANALYTICS_CACHE_VERSION,
        snapshot: snapshot.clone(),
    };
    serde_json::to_vec_pretty(&cache).map_err(|error| format!("序列化成本分析缓存失败: {error}"))
}

pub(crate) fn parse_codex_cost_analytics_cache(
    raw: &str,
    weekly_budget_usd: Option<f64>,
) -> Result<Option<CodexCostAnalyticsSnapshot>, String> {
    if raw.trim().is_empty() {
        return Ok(None);
    }

    if let Ok(cache) = serde_json::from_str::<CodexCostAnalyticsCacheFile>(raw) {
        if cache.version != COST_ANALYTICS_CACHE_VERSION {
            return Ok(None);
        }
        return Ok(Some(apply_codex_cost_analytics_budget(
            cache.snapshot,
            weekly_budget_usd,
        )));
    }

    let snapshot = serde_json::from_str::<CodexCostAnalyticsSnapshot>(raw)
        .map_err(|error| format!("解析成本分析缓存失败: {error}"))?;
    Ok(Some(apply_codex_cost_analytics_budget(
        snapshot,
        weekly_budget_usd,
    )))
}

pub(crate) fn serialize_codex_cost_analytics_export(
    snapshot: &CodexCostAnalyticsSnapshot,
    format: &str,
) -> Result<Vec<u8>, String> {
    match format {
        "json" => serde_json::to_vec_pretty(snapshot)
            .map_err(|error| format!("序列化 JSON 取证导出失败: {error}")),
        "csv" => Ok(cost_analytics_csv(snapshot).into_bytes()),
        other => Err(format!("不支持的导出格式: {other}")),
    }
}

struct ParsedTokenSessionFile {
    events: Vec<ParsedTokenEvent>,
    latest_session: Option<CodexTokenSessionUsage>,
}

struct ParsedAnalyticsSessionFile {
    session: CodexSessionCostBreakdown,
    events: Vec<AnalyticsTokenEvent>,
    prompt_keys: Vec<String>,
}

#[derive(Clone)]
struct AnalyticsTokenEvent {
    timestamp: i64,
    session_id: String,
    project_path: String,
    project_name: String,
    model: String,
    prompt_key: String,
    prompt_preview: String,
    prompt_chars: usize,
    total: CodexTokenTotals,
    cost_usd: f64,
    source_path: String,
}

#[derive(Default)]
struct ProjectAccumulator {
    project_path: String,
    project_name: String,
    session_count: usize,
    prompt_keys: Vec<String>,
    event_count: usize,
    total: CodexTokenTotals,
    cost_usd: f64,
    last_at: Option<i64>,
}

impl ProjectAccumulator {
    fn new(project_path: &str) -> Self {
        Self {
            project_path: project_path.to_string(),
            project_name: project_name_from_path(project_path),
            ..Self::default()
        }
    }

    fn into_breakdown(self) -> CodexProjectCostBreakdown {
        let mut prompt_keys = self.prompt_keys;
        prompt_keys.sort();
        prompt_keys.dedup();

        CodexProjectCostBreakdown {
            project_path: self.project_path,
            project_name: self.project_name,
            session_count: self.session_count,
            prompt_count: prompt_keys.len(),
            event_count: self.event_count,
            total: self.total,
            cost_usd: round_cost(self.cost_usd),
            last_at: self.last_at,
        }
    }
}

struct PromptAccumulator {
    session_id: String,
    project_path: String,
    project_name: String,
    timestamp: i64,
    model: String,
    prompt_preview: String,
    prompt_chars: usize,
    total: CodexTokenTotals,
    cost_usd: f64,
    source_path: String,
}

impl PromptAccumulator {
    fn from_event(event: &AnalyticsTokenEvent) -> Self {
        Self {
            session_id: event.session_id.clone(),
            project_path: event.project_path.clone(),
            project_name: event.project_name.clone(),
            timestamp: event.timestamp,
            model: event.model.clone(),
            prompt_preview: event.prompt_preview.clone(),
            prompt_chars: event.prompt_chars,
            total: CodexTokenTotals::default(),
            cost_usd: 0.0,
            source_path: event.source_path.clone(),
        }
    }

    fn into_breakdown(self) -> CodexPromptCostBreakdown {
        CodexPromptCostBreakdown {
            session_id: self.session_id,
            project_path: self.project_path,
            project_name: self.project_name,
            timestamp: self.timestamp,
            model: self.model,
            prompt_preview: self.prompt_preview,
            prompt_chars: self.prompt_chars,
            total: self.total,
            cost_usd: round_cost(self.cost_usd),
            source_path: self.source_path,
        }
    }
}

struct PricingRate {
    input_per_million: f64,
    cached_input_per_million: f64,
    output_per_million: f64,
}

fn parse_cost_analytics_session_file(path: &Path) -> Result<ParsedAnalyticsSessionFile, String> {
    let file = fs::File::open(path).map_err(|error| format!("读取 Codex 日志失败: {error}"))?;
    let reader = BufReader::new(file);
    let source_path = path.to_string_lossy().to_string();
    let fallback_session_id = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown-session")
        .to_string();

    let mut session_id = fallback_session_id;
    let mut parent_session_id = None;
    let mut project_path = "(unknown project)".to_string();
    let mut current_model = "unknown".to_string();
    let mut current_prompt_preview = "(no prompt captured)".to_string();
    let mut current_prompt_chars = 0usize;
    let mut current_prompt_index = 0usize;
    let mut prompt_keys = Vec::new();
    let mut prompt_key_seen = BTreeMap::<String, ()>::new();
    let mut model_tokens = HashMap::<String, u64>::new();
    let mut events = Vec::new();
    let mut total = CodexTokenTotals::default();
    let mut cost_usd = 0.0;
    let mut event_count = 0usize;
    let mut started_at = None;
    let mut updated_at = None;

    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => continue,
        };
        let root = match serde_json::from_str::<Value>(&line) {
            Ok(root) => root,
            Err(_) => continue,
        };
        let root_type = root.get("type").and_then(Value::as_str).unwrap_or_default();
        let payload = root.get("payload").unwrap_or(&Value::Null);
        let timestamp = root
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(parse_timestamp);

        if root_type == "session_meta" {
            if let Some(id) = payload.get("id").and_then(Value::as_str) {
                session_id = id.to_string();
            }
            parent_session_id = payload
                .get("forked_from_id")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
                project_path = cwd.to_string();
            }
            if let Some(model) = payload.get("model").and_then(Value::as_str) {
                current_model = model.to_string();
            }
            continue;
        }

        if root_type == "turn_context" {
            if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
                project_path = cwd.to_string();
            }
            if let Some(model) = payload.get("model").and_then(Value::as_str) {
                current_model = model.to_string();
            }
            continue;
        }

        if root_type == "event_msg"
            && payload.get("type").and_then(Value::as_str) == Some("user_message")
        {
            if let Some(prompt) = payload.get("message").and_then(Value::as_str) {
                current_prompt_index += 1;
                current_prompt_chars = prompt.chars().count();
                current_prompt_preview = prompt_preview(prompt);
            }
            continue;
        }

        if root_type == "response_item"
            && payload.get("type").and_then(Value::as_str) == Some("message")
            && payload.get("role").and_then(Value::as_str) == Some("user")
        {
            if let Some(prompt) = message_payload_text(payload) {
                current_prompt_index += 1;
                current_prompt_chars = prompt.chars().count();
                current_prompt_preview = prompt_preview(&prompt);
            }
            continue;
        }

        if root_type != "event_msg"
            || payload.get("type").and_then(Value::as_str) != Some("token_count")
        {
            continue;
        }

        let Some(timestamp) = timestamp else {
            continue;
        };
        let Some(last) = payload
            .get("info")
            .and_then(|info| info.get("last_token_usage"))
            .and_then(parse_token_totals)
        else {
            continue;
        };

        // `last_token_usage` is the only per-event delta in the rollout log, so every
        // higher-level view is derived from this same source of truth.
        let event_cost_usd = estimate_token_cost_usd(&current_model, &last);
        let project_name = project_name_from_path(&project_path);
        let prompt_key = format!("{session_id}:{current_prompt_index}");
        if prompt_key_seen.insert(prompt_key.clone(), ()).is_none() {
            prompt_keys.push(prompt_key.clone());
        }

        total.add(&last);
        cost_usd += event_cost_usd;
        event_count += 1;
        started_at = Some(
            started_at
                .map(|current: i64| current.min(timestamp))
                .unwrap_or(timestamp),
        );
        updated_at = Some(
            updated_at
                .map(|current: i64| current.max(timestamp))
                .unwrap_or(timestamp),
        );
        *model_tokens.entry(current_model.clone()).or_insert(0) += last.total_tokens;

        events.push(AnalyticsTokenEvent {
            timestamp,
            session_id: session_id.clone(),
            project_path: project_path.clone(),
            project_name,
            model: current_model.clone(),
            prompt_key,
            prompt_preview: current_prompt_preview.clone(),
            prompt_chars: current_prompt_chars,
            total: last,
            cost_usd: event_cost_usd,
            source_path: source_path.clone(),
        });
    }

    let duration_seconds = match (started_at, updated_at) {
        (Some(start), Some(end)) if end >= start => Some(end - start),
        _ => None,
    };

    let model = model_tokens
        .into_iter()
        .max_by_key(|(_, tokens)| *tokens)
        .map(|(model, _)| model)
        .unwrap_or_else(|| "unknown".to_string());

    Ok(ParsedAnalyticsSessionFile {
        session: CodexSessionCostBreakdown {
            session_id,
            parent_session_id,
            project_name: project_name_from_path(&project_path),
            project_path,
            started_at,
            updated_at,
            duration_seconds,
            prompt_count: prompt_keys.len(),
            event_count,
            model,
            total,
            cost_usd: round_cost(cost_usd),
            source_path,
        },
        events,
        prompt_keys,
    })
}

fn parse_token_session_file(path: &Path) -> Result<ParsedTokenSessionFile, String> {
    let file = fs::File::open(path).map_err(|error| format!("读取 Codex 日志失败: {error}"))?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    let mut session = ParsedSession::default();

    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => continue,
        };
        let Some(event) = parse_token_event_line(&line) else {
            continue;
        };

        session.observe(&event);
        events.push(event);
    }

    Ok(ParsedTokenSessionFile {
        events,
        latest_session: session.into_latest_session(),
    })
}

fn parse_token_event_line(line: &str) -> Option<ParsedTokenEvent> {
    let root = serde_json::from_str::<Value>(line).ok()?;
    if root.get("type")?.as_str()? != "event_msg" {
        return None;
    }

    let payload = root.get("payload")?;
    if payload.get("type")?.as_str()? != "token_count" {
        return None;
    }

    let timestamp = parse_timestamp(root.get("timestamp")?.as_str()?)?;
    let info = payload.get("info")?;
    let last = info.get("last_token_usage").and_then(parse_token_totals);
    let total = info.get("total_token_usage").and_then(parse_token_totals);
    if last.is_none() && total.is_none() {
        return None;
    }

    Some(ParsedTokenEvent {
        timestamp,
        last,
        total,
    })
}

fn parse_token_totals(value: &Value) -> Option<CodexTokenTotals> {
    if !value.is_object() {
        return None;
    }

    let input_tokens = field_u64(value, "input_tokens");
    let cached_input_tokens = field_u64(value, "cached_input_tokens");
    let output_tokens = field_u64(value, "output_tokens");
    let reasoning_output_tokens = field_u64(value, "reasoning_output_tokens");
    let total_tokens = field_u64(value, "total_tokens").unwrap_or_else(|| {
        input_tokens
            .unwrap_or(0)
            .saturating_add(output_tokens.unwrap_or(0))
    });

    Some(CodexTokenTotals {
        input_tokens: input_tokens.unwrap_or(0),
        cached_input_tokens: cached_input_tokens.unwrap_or(0),
        output_tokens: output_tokens.unwrap_or(0),
        reasoning_output_tokens: reasoning_output_tokens.unwrap_or(0),
        total_tokens,
    })
}

fn message_payload_text(payload: &Value) -> Option<String> {
    match payload.get("content")? {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let parts = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(Value::as_str)
                        .or_else(|| item.get("input_text").and_then(Value::as_str))
                })
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        _ => None,
    }
}

fn prompt_preview(prompt: &str) -> String {
    let normalized = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&normalized, PROMPT_PREVIEW_CHARS)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn project_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(path)
        .to_string()
}

fn max_option_i64(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn cost_analytics_progress(
    stage: &str,
    processed_files: usize,
    total_files: usize,
    current_path: Option<String>,
) -> CodexCostAnalyticsProgress {
    let percent = if total_files == 0 {
        100
    } else {
        ((processed_files.saturating_mul(100)) / total_files).min(100) as u8
    };

    CodexCostAnalyticsProgress {
        stage: stage.to_string(),
        processed_files,
        total_files,
        percent,
        current_path,
    }
}

fn normalize_budget(value: Option<f64>) -> Option<f64> {
    value.and_then(|budget| {
        if budget.is_finite() && budget > 0.0 {
            Some(round_cost(budget))
        } else {
            None
        }
    })
}

fn weekly_budget_alert(percent: Option<f64>) -> String {
    match percent {
        Some(value) if value >= 100.0 => "danger".to_string(),
        Some(value) if value >= 80.0 => "warning".to_string(),
        Some(_) => "ok".to_string(),
        None => "none".to_string(),
    }
}

fn round_cost(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn round_percent(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn estimate_token_cost_usd(model: &str, usage: &CodexTokenTotals) -> f64 {
    let rate = pricing_rate_for_model(model);
    let cached_input = usage.cached_input_tokens.min(usage.input_tokens);
    let uncached_input = usage.input_tokens.saturating_sub(cached_input);
    let cost = (uncached_input as f64 * rate.input_per_million
        + cached_input as f64 * rate.cached_input_per_million
        + usage.output_tokens as f64 * rate.output_per_million)
        / 1_000_000.0;
    round_cost(cost)
}

fn pricing_rate_for_model(model: &str) -> PricingRate {
    let normalized = model.to_ascii_lowercase();
    if normalized.starts_with("gpt-5.5-pro") {
        return PricingRate {
            input_per_million: 15.0,
            cached_input_per_million: 15.0,
            output_per_million: 90.0,
        };
    }
    if normalized.starts_with("gpt-5.5") {
        return PricingRate {
            input_per_million: 2.5,
            cached_input_per_million: 0.25,
            output_per_million: 15.0,
        };
    }
    if normalized.starts_with("gpt-5.4-pro") {
        return PricingRate {
            input_per_million: 15.0,
            cached_input_per_million: 15.0,
            output_per_million: 90.0,
        };
    }
    if normalized.starts_with("gpt-5.4-mini") {
        return PricingRate {
            input_per_million: 0.375,
            cached_input_per_million: 0.0375,
            output_per_million: 2.25,
        };
    }
    if normalized.starts_with("gpt-5.4-nano") {
        return PricingRate {
            input_per_million: 0.1,
            cached_input_per_million: 0.01,
            output_per_million: 0.625,
        };
    }
    if normalized.starts_with("gpt-5.4") {
        return PricingRate {
            input_per_million: 1.25,
            cached_input_per_million: 0.13,
            output_per_million: 7.5,
        };
    }
    if normalized.contains("codex-mini") || normalized.starts_with("gpt-5-mini") {
        return PricingRate {
            input_per_million: 0.25,
            cached_input_per_million: 0.025,
            output_per_million: 2.0,
        };
    }
    if normalized.starts_with("gpt-5-nano") {
        return PricingRate {
            input_per_million: 0.05,
            cached_input_per_million: 0.005,
            output_per_million: 0.4,
        };
    }
    if normalized.starts_with("o4-mini") {
        return PricingRate {
            input_per_million: 1.1,
            cached_input_per_million: 0.275,
            output_per_million: 4.4,
        };
    }
    if normalized.starts_with("o3") {
        return PricingRate {
            input_per_million: 2.0,
            cached_input_per_million: 0.5,
            output_per_million: 8.0,
        };
    }

    PricingRate {
        input_per_million: 1.25,
        cached_input_per_million: 0.125,
        output_per_million: 10.0,
    }
}

fn initial_heatmap() -> BTreeMap<(u8, u8), CodexHourlyCostBucket> {
    let mut buckets = BTreeMap::new();
    for weekday in 0..7 {
        for hour in 0..24 {
            buckets.insert(
                (weekday, hour),
                CodexHourlyCostBucket {
                    weekday,
                    hour,
                    calls: 0,
                    tokens: 0,
                    cost_usd: 0.0,
                },
            );
        }
    }
    buckets
}

fn heatmap_bucket_key(timestamp: i64) -> Option<(u8, u8)> {
    let date_time = OffsetDateTime::from_unix_timestamp(timestamp).ok()?;
    Some((
        date_time.weekday().number_days_from_sunday() as u8,
        date_time.hour(),
    ))
}

fn cost_analytics_csv(snapshot: &CodexCostAnalyticsSnapshot) -> String {
    let mut rows = Vec::new();
    rows.push(csv_row(&[
        "row_type",
        "id",
        "project",
        "project_path",
        "session_id",
        "parent_session_id",
        "timestamp",
        "updated_at",
        "weekday",
        "hour",
        "model",
        "prompt_preview",
        "prompt_chars",
        "input_tokens",
        "cached_input_tokens",
        "output_tokens",
        "reasoning_output_tokens",
        "total_tokens",
        "calls",
        "cost_usd",
        "source_path",
        "pricing_source",
    ]));

    rows.push(csv_row(&[
        "summary",
        "all",
        "",
        "",
        "",
        "",
        "",
        &snapshot.updated_at.to_string(),
        "",
        "",
        "",
        "",
        "",
        &snapshot.total.input_tokens.to_string(),
        &snapshot.total.cached_input_tokens.to_string(),
        &snapshot.total.output_tokens.to_string(),
        &snapshot.total.reasoning_output_tokens.to_string(),
        &snapshot.total.total_tokens.to_string(),
        &snapshot.event_count.to_string(),
        &snapshot.total_cost_usd.to_string(),
        "",
        &snapshot.pricing_source,
    ]));

    for project in &snapshot.projects {
        rows.push(csv_row(&[
            "project",
            &project.project_name,
            &project.project_name,
            &project.project_path,
            "",
            "",
            "",
            &project
                .last_at
                .map(|value| value.to_string())
                .unwrap_or_default(),
            "",
            "",
            "",
            "",
            "",
            &project.total.input_tokens.to_string(),
            &project.total.cached_input_tokens.to_string(),
            &project.total.output_tokens.to_string(),
            &project.total.reasoning_output_tokens.to_string(),
            &project.total.total_tokens.to_string(),
            &project.event_count.to_string(),
            &project.cost_usd.to_string(),
            "",
            &snapshot.pricing_source,
        ]));
    }

    for session in &snapshot.sessions {
        rows.push(csv_row(&[
            "session",
            &session.session_id,
            &session.project_name,
            &session.project_path,
            &session.session_id,
            session.parent_session_id.as_deref().unwrap_or_default(),
            &session
                .started_at
                .map(|value| value.to_string())
                .unwrap_or_default(),
            &session
                .updated_at
                .map(|value| value.to_string())
                .unwrap_or_default(),
            "",
            "",
            &session.model,
            "",
            "",
            &session.total.input_tokens.to_string(),
            &session.total.cached_input_tokens.to_string(),
            &session.total.output_tokens.to_string(),
            &session.total.reasoning_output_tokens.to_string(),
            &session.total.total_tokens.to_string(),
            &session.event_count.to_string(),
            &session.cost_usd.to_string(),
            &session.source_path,
            &snapshot.pricing_source,
        ]));
    }

    for prompt in &snapshot.top_prompts {
        rows.push(csv_row(&[
            "top_prompt",
            &format!("{}:{}", prompt.session_id, prompt.timestamp),
            &prompt.project_name,
            &prompt.project_path,
            &prompt.session_id,
            "",
            &prompt.timestamp.to_string(),
            "",
            "",
            "",
            &prompt.model,
            &prompt.prompt_preview,
            &prompt.prompt_chars.to_string(),
            &prompt.total.input_tokens.to_string(),
            &prompt.total.cached_input_tokens.to_string(),
            &prompt.total.output_tokens.to_string(),
            &prompt.total.reasoning_output_tokens.to_string(),
            &prompt.total.total_tokens.to_string(),
            "",
            &prompt.cost_usd.to_string(),
            &prompt.source_path,
            &snapshot.pricing_source,
        ]));
    }

    for bucket in &snapshot.heatmap {
        if bucket.calls == 0 {
            continue;
        }
        rows.push(csv_row(&[
            "heatmap",
            &format!("{}-{}", bucket.weekday, bucket.hour),
            "",
            "",
            "",
            "",
            "",
            "",
            &bucket.weekday.to_string(),
            &bucket.hour.to_string(),
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            &bucket.tokens.to_string(),
            &bucket.calls.to_string(),
            &round_cost(bucket.cost_usd).to_string(),
            "",
            &snapshot.pricing_source,
        ]));
    }

    rows.join("\n") + "\n"
}

fn csv_row(fields: &[&str]) -> String {
    fields
        .iter()
        .map(|field| csv_escape(field))
        .collect::<Vec<_>>()
        .join(",")
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn field_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key)?.as_u64()
}

fn parse_timestamp(value: &str) -> Option<i64> {
    OffsetDateTime::parse(value, &Rfc3339)
        .ok()
        .map(|timestamp| timestamp.unix_timestamp())
}

fn collect_jsonl_files(path: &Path, files: &mut Vec<PathBuf>, failed_path_count: &mut usize) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        if path.exists() {
            *failed_path_count += 1;
        }
        return;
    };

    if metadata.is_file() {
        if path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
            files.push(path.to_path_buf());
        }
        return;
    }

    if !metadata.is_dir() {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        *failed_path_count += 1;
        return;
    };

    for entry in entries {
        match entry {
            Ok(entry) => collect_jsonl_files(&entry.path(), files, failed_path_count),
            Err(_) => *failed_path_count += 1,
        }
    }
}

impl CodexTokenTotals {
    fn add(&mut self, other: &CodexTokenTotals) {
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

    fn is_empty(&self) -> bool {
        self.total_tokens == 0
            && self.input_tokens == 0
            && self.output_tokens == 0
            && self.cached_input_tokens == 0
            && self.reasoning_output_tokens == 0
    }
}

impl ParsedSession {
    fn observe(&mut self, event: &ParsedTokenEvent) {
        self.started_at = Some(
            self.started_at
                .map(|current| current.min(event.timestamp))
                .unwrap_or(event.timestamp),
        );
        self.updated_at = Some(
            self.updated_at
                .map(|current| current.max(event.timestamp))
                .unwrap_or(event.timestamp),
        );

        if let Some(total) = event.total.as_ref() {
            self.total = total.clone();
        }
        if let Some(last) = event.last.as_ref() {
            self.fallback_total.add(last);
        }
    }

    fn into_latest_session(self) -> Option<CodexTokenSessionUsage> {
        let updated_at = self.updated_at?;
        let total = if self.total.is_empty() {
            self.fallback_total
        } else {
            self.total
        };

        Some(CodexTokenSessionUsage {
            started_at: self.started_at,
            updated_at,
            total,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    fn event_line(timestamp: &str, total: u64, last: u64) -> String {
        serde_json::json!({
            "timestamp": timestamp,
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "total_token_usage": {
                        "input_tokens": total,
                        "cached_input_tokens": 10,
                        "output_tokens": 20,
                        "reasoning_output_tokens": 5,
                        "total_tokens": total
                    },
                    "last_token_usage": {
                        "input_tokens": last,
                        "cached_input_tokens": 1,
                        "output_tokens": 2,
                        "reasoning_output_tokens": 1,
                        "total_tokens": last
                    }
                }
            }
        })
        .to_string()
    }

    fn analytics_token_line(timestamp: &str, input: u64, cached: u64, output: u64) -> String {
        serde_json::json!({
            "timestamp": timestamp,
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "last_token_usage": {
                        "input_tokens": input,
                        "cached_input_tokens": cached,
                        "output_tokens": output,
                        "reasoning_output_tokens": 0,
                        "total_tokens": input + output
                    }
                }
            }
        })
        .to_string()
    }

    #[test]
    fn parses_codex_token_event_lines() {
        let event =
            parse_token_event_line(&event_line("2026-04-28T06:37:43.263Z", 40902952, 206498))
                .expect("token event");

        assert_eq!(event.timestamp, 1_777_358_263);
        assert_eq!(event.last.expect("last usage").total_tokens, 206_498);
        assert_eq!(event.total.expect("total usage").input_tokens, 40_902_952);
    }

    #[test]
    fn scans_windows_from_known_roots() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions").join("2026").join("04").join("28");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        fs::write(
            sessions.join("rollout-test.jsonl"),
            [
                event_line("2026-04-27T06:00:00Z", 100, 100),
                event_line("2026-04-28T06:00:00Z", 350, 250),
            ]
            .join("\n"),
        )
        .expect("write log");

        let snapshot = scan_codex_token_usage_roots(
            &[root.join("sessions"), root.join("archived_sessions")],
            1_777_361_000,
        );

        assert_eq!(snapshot.source_path_count, 1);
        assert_eq!(snapshot.event_count, 2);
        assert_eq!(snapshot.last_24h.total_tokens, 250);
        assert_eq!(snapshot.last_3d.total_tokens, 350);
        assert_eq!(snapshot.last_7d.total_tokens, 350);
        assert_eq!(
            snapshot
                .latest_session
                .expect("latest session")
                .total
                .total_tokens,
            350
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scans_cost_analytics_by_project_session_prompt_and_budget() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions").join("2026").join("06").join("10");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        fs::write(
            sessions.join("rollout-analytics.jsonl"),
            [
                serde_json::json!({
                    "timestamp": "2026-06-10T00:00:00Z",
                    "type": "session_meta",
                    "payload": {
                        "id": "session-1",
                        "cwd": "/tmp/project-alpha"
                    }
                })
                .to_string(),
                serde_json::json!({
                    "timestamp": "2026-06-10T00:00:01Z",
                    "type": "turn_context",
                    "payload": {
                        "cwd": "/tmp/project-alpha",
                        "model": "gpt-5.5"
                    }
                })
                .to_string(),
                serde_json::json!({
                    "timestamp": "2026-06-10T00:00:02Z",
                    "type": "event_msg",
                    "payload": {
                        "type": "user_message",
                        "message": "Build forensic export"
                    }
                })
                .to_string(),
                analytics_token_line("2026-06-10T00:00:03Z", 1_000, 100, 2_000),
            ]
            .join("\n"),
        )
        .expect("write analytics log");

        let now = parse_timestamp("2026-06-11T00:00:00Z").expect("parse now");
        let mut progress_events = Vec::new();
        let snapshot = scan_codex_cost_analytics_roots_with_progress(
            &[root.join("sessions")],
            now,
            Some(0.01),
            |progress| progress_events.push(progress),
        );

        assert_eq!(snapshot.source_path_count, 1);
        assert_eq!(snapshot.event_count, 1);
        assert_eq!(snapshot.total.total_tokens, 3_000);
        assert!((snapshot.total_cost_usd - 0.032275).abs() < 0.000001);
        assert_eq!(snapshot.weekly_budget_alert, "danger");
        assert_eq!(progress_events.last().expect("progress").percent, 100);
        assert_eq!(snapshot.projects[0].project_name, "project-alpha");
        assert_eq!(snapshot.projects[0].prompt_count, 1);
        assert_eq!(snapshot.sessions[0].session_id, "session-1");
        assert_eq!(snapshot.sessions[0].model, "gpt-5.5");
        assert_eq!(
            snapshot.top_prompts[0].prompt_preview,
            "Build forensic export"
        );

        let csv = String::from_utf8(
            serialize_codex_cost_analytics_export(&snapshot, "csv").expect("csv export"),
        )
        .expect("utf8 csv");
        assert!(csv.contains("top_prompt"));
        assert!(csv.contains("Build forensic export"));

        let cache = String::from_utf8(
            serialize_codex_cost_analytics_cache(&snapshot).expect("cache export"),
        )
        .expect("utf8 cache");
        let cached = parse_codex_cost_analytics_cache(&cache, Some(1.0))
            .expect("cache parse")
            .expect("cache snapshot");
        assert_eq!(cached.event_count, snapshot.event_count);
        assert_eq!(cached.weekly_budget_usd, Some(1.0));
        assert_eq!(cached.weekly_budget_alert, "ok");

        let _ = fs::remove_dir_all(root);
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "codex-tools-token-usage-{}-{nanos}",
            std::process::id()
        ))
    }
}
