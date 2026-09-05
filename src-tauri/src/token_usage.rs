use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::hash::Hash;
use std::hash::Hasher;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::SystemTime;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::Date;
use time::OffsetDateTime;
use time::UtcOffset;

use crate::app_paths;
use crate::utils::now_unix_seconds;

const DAY_SECONDS: i64 = 24 * 60 * 60;
const PROMPT_PREVIEW_CHARS: usize = 220;
const TOP_EXPENSIVE_PROMPT_LIMIT: usize = 20;
const SESSION_EXPORT_LIMIT: usize = 500;
const TOKEN_USAGE_TAIL_SIGNATURE_BYTES: u64 = 128;
const FORK_MATCH_RESYNC_WINDOW: usize = 32;
const FORK_MATCH_ANCHOR_RECORDS: usize = 4;
const PRICING_SOURCE: &str =
    "OpenAI API standard short-context pricing, historical rates by event time, checked 2026-09-05";
// OpenAI announced lower GPT-5.6 Terra and Luna prices effective 2026-07-30.
// The announcement only specifies the date, so analytics use the UTC day boundary.
// Source: https://openai.com/index/advancing-the-price-performance-frontier-with-gpt-5-6/
const GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT: i64 = 1_785_369_600;
// Sol price reduction announced 2026-08-21; use the UTC day boundary because
// the official changelog does not specify an intraday effective time.
// https://developers.openai.com/api/docs/changelog
const GPT_5_6_SOL_PRICE_REDUCTION_EFFECTIVE_AT: i64 = 1_787_270_400;
const COST_ANALYTICS_CACHE_VERSION: u8 = 10;
const COST_SOURCE_LOCAL: &str = "local_estimate";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    pub(crate) unresolved_fork_count: usize,
    pub(crate) unresolved_usage_event_count: usize,
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
    #[serde(default)]
    pub(crate) unresolved_fork_count: usize,
    #[serde(default)]
    pub(crate) unresolved_usage_event_count: usize,
    pub(crate) event_count: usize,
    pub(crate) total: CodexTokenTotals,
    pub(crate) total_cost_usd: f64,
    #[serde(default)]
    pub(crate) local_total_cost_usd: f64,
    pub(crate) last_7d: CodexTokenTotals,
    pub(crate) last_7d_cost_usd: f64,
    #[serde(default)]
    pub(crate) local_last_7d_cost_usd: f64,
    #[serde(default)]
    pub(crate) budget_period_cost_usd: f64,
    #[serde(default)]
    pub(crate) local_budget_period_cost_usd: f64,
    #[serde(default)]
    pub(crate) cost_source: String,
    #[serde(default)]
    pub(crate) cost_source_updated_at: Option<i64>,
    #[serde(default)]
    pub(crate) cost_source_error: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedTokenEvent {
    record_index: usize,
    timestamp: i64,
    last: Option<CodexTokenTotals>,
    total: Option<CodexTokenTotals>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogFileFingerprint {
    length: u64,
    modified_at: Option<SystemTime>,
}

#[derive(Debug, Clone)]
struct CachedTokenUsageFile {
    fingerprint: LogFileFingerprint,
    tail_signature: Vec<u8>,
    parsed: ParsedTokenSessionFile,
}

#[derive(Debug, Default)]
struct TokenUsageCache {
    files: HashMap<PathBuf, CachedTokenUsageFile>,
}

static TOKEN_USAGE_CACHE: OnceLock<Mutex<TokenUsageCache>> = OnceLock::new();

struct CachedCostAnalyticsFile {
    fingerprint: LogFileFingerprint,
    tail_signature: Vec<u8>,
    state: AnalyticsFileState,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CostAnalyticsCacheStats {
    reused_files: usize,
    appended_files: usize,
    reparsed_files: usize,
    evicted_files: usize,
}

#[derive(Default)]
struct CostAnalyticsCache {
    files: HashMap<PathBuf, CachedCostAnalyticsFile>,
    last_scan: CostAnalyticsCacheStats,
}

static COST_ANALYTICS_CACHE: OnceLock<Mutex<CostAnalyticsCache>> = OnceLock::new();

struct StableFnvHasher(u64);

impl StableFnvHasher {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for StableFnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

#[derive(Debug, Default)]
struct ParsedSession {
    started_at: Option<i64>,
    updated_at: Option<i64>,
    summed_last_usage: CodexTokenTotals,
    latest_cumulative_total: CodexTokenTotals,
}

pub(crate) fn collect_codex_token_usage_snapshot() -> Result<CodexTokenUsageSnapshot, String> {
    let codex_dir = app_paths::codex_dir()?;
    let roots = [
        codex_dir.join("sessions"),
        codex_dir.join("archived_sessions"),
    ];
    let mut cache = TOKEN_USAGE_CACHE
        .get_or_init(|| Mutex::new(TokenUsageCache::default()))
        .lock()
        .unwrap_or_else(|poisoned| {
            log::warn!("Token 用量缓存锁异常，继续使用恢复后的缓存");
            poisoned.into_inner()
        });
    Ok(scan_codex_token_usage_roots_with_cache(
        &roots,
        now_unix_seconds(),
        &mut cache,
    ))
}

#[cfg(test)]
fn scan_codex_token_usage_roots(roots: &[PathBuf], now: i64) -> CodexTokenUsageSnapshot {
    let mut cache = TokenUsageCache::default();
    scan_codex_token_usage_roots_with_cache(roots, now, &mut cache)
}

fn scan_codex_token_usage_roots_with_cache(
    roots: &[PathBuf],
    now: i64,
    cache: &mut TokenUsageCache,
) -> CodexTokenUsageSnapshot {
    let mut file_paths = Vec::new();
    let mut failed_path_count = 0;
    for root in roots {
        collect_jsonl_files(root, &mut file_paths, &mut failed_path_count);
    }
    file_paths.sort();
    file_paths.dedup();

    let source_path_count = file_paths.len();
    let mut sources = Vec::with_capacity(source_path_count);
    for path in file_paths {
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => sources.push((
                path,
                LogFileFingerprint {
                    length: metadata.len(),
                    modified_at: metadata.modified().ok(),
                },
            )),
            Ok(_) | Err(_) => failed_path_count += 1,
        }
    }

    let source_paths: HashSet<&PathBuf> = sources.iter().map(|(path, _)| path).collect();
    cache.files.retain(|path, _| source_paths.contains(path));

    for (path, fingerprint) in &sources {
        let cache_hit = cache
            .files
            .get(path)
            .map(|cached| cached.fingerprint == *fingerprint)
            .unwrap_or(false);
        if cache_hit {
            continue;
        }

        let appended = cache
            .files
            .get_mut(path)
            .and_then(|cached| append_token_session_file(path, fingerprint, cached).ok())
            .unwrap_or(false);
        if appended {
            continue;
        }

        match parse_token_session_file(path) {
            Ok(cached) => {
                cache.files.insert(path.clone(), cached);
            }
            Err(_) => {
                cache.files.remove(path);
                failed_path_count += 1;
            }
        }
    }

    let parsed_files: Vec<&ParsedTokenSessionFile> = sources
        .iter()
        .filter_map(|(path, _)| cache.files.get(path).map(|cached| &cached.parsed))
        .collect();

    build_codex_token_usage_snapshot(&parsed_files, now, source_path_count, failed_path_count)
}

fn build_codex_token_usage_snapshot(
    files: &[&ParsedTokenSessionFile],
    now: i64,
    source_path_count: usize,
    failed_path_count: usize,
) -> CodexTokenUsageSnapshot {
    let mut snapshot = CodexTokenUsageSnapshot {
        updated_at: now,
        source_path_count,
        failed_path_count,
        unresolved_fork_count: 0,
        unresolved_usage_event_count: 0,
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
    let lineage_nodes = files
        .iter()
        .map(|file| LineageNode {
            thread_id: &file.session_id,
            history_parent_id: file.parent_session_id.as_deref(),
            agent_parent_id: file.parent_thread_id.as_deref(),
            identity_trusted: file.identity_trusted,
            record_hashes: &file.record_hashes,
            record_resync_window: FORK_MATCH_RESYNC_WINDOW,
        })
        .collect::<Vec<_>>();
    let ownerships = derive_session_ownerships(&lineage_nodes);

    for (session_index, session) in files.iter().enumerate() {
        let ownership = ownerships[session_index];
        if ownership.unresolved {
            snapshot.unresolved_fork_count += 1;
            continue;
        }
        let delta_result =
            derive_confirmed_token_deltas(&session.events, ownership.inherited_record_end);
        snapshot.unresolved_usage_event_count += delta_result.unresolved_event_count;
        for event in &delta_result.events {
            snapshot.event_count += 1;
            if event.timestamp >= last_24h_start {
                snapshot.last_24h.add(&event.total);
            }
            if event.timestamp >= last_3d_start {
                snapshot.last_3d.add(&event.total);
            }
            if event.timestamp >= last_7d_start {
                snapshot.last_7d.add(&event.total);
            }
            if event.timestamp >= last_30d_start {
                snapshot.last_30d.add(&event.total);
            }
        }

        if let Some(latest_session) = latest_confirmed_token_session(&delta_result.events) {
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

    snapshot
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfirmedTokenDelta {
    timestamp: i64,
    total: CodexTokenTotals,
}

#[derive(Debug, Default)]
struct ConfirmedTokenDeltaResult {
    events: Vec<ConfirmedTokenDelta>,
    unresolved_event_count: usize,
}

fn derive_confirmed_token_deltas(
    events: &[ParsedTokenEvent],
    inherited_record_end: usize,
) -> ConfirmedTokenDeltaResult {
    let mut result = ConfirmedTokenDeltaResult::default();
    let inherited_baseline = events
        .iter()
        .filter(|event| event.record_index < inherited_record_end)
        .filter_map(|event| event.total.as_ref())
        .last()
        .cloned();
    let inherited_token_event_count = events
        .iter()
        .filter(|event| event.record_index < inherited_record_end)
        .count();
    if inherited_record_end > 0 && inherited_token_event_count > 0 && inherited_baseline.is_none() {
        result.unresolved_event_count = events
            .iter()
            .filter(|event| event.record_index >= inherited_record_end)
            .count();
        return result;
    }

    let mut previous_total = inherited_baseline.unwrap_or_default();
    for event in events
        .iter()
        .filter(|event| event.record_index >= inherited_record_end)
    {
        let Some(current_total) = event.total.as_ref() else {
            result.unresolved_event_count += 1;
            continue;
        };
        let Some(delta) = token_totals_delta(current_total, &previous_total) else {
            result.unresolved_event_count += 1;
            previous_total = current_total.clone();
            continue;
        };
        previous_total = current_total.clone();
        if delta.is_empty() {
            continue;
        }
        result.events.push(ConfirmedTokenDelta {
            timestamp: event.timestamp,
            total: delta,
        });
    }
    result
}

fn latest_confirmed_token_session(
    events: &[ConfirmedTokenDelta],
) -> Option<CodexTokenSessionUsage> {
    let mut total = CodexTokenTotals::default();
    for event in events {
        total.add(&event.total);
    }
    Some(CodexTokenSessionUsage {
        started_at: events.first().map(|event| event.timestamp),
        updated_at: events.last()?.timestamp,
        total,
    })
}

#[derive(Debug, Clone, Copy)]
struct LineageNode<'a> {
    thread_id: &'a str,
    history_parent_id: Option<&'a str>,
    agent_parent_id: Option<&'a str>,
    identity_trusted: bool,
    record_hashes: &'a [u64],
    record_resync_window: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SessionOwnership {
    inherited_record_end: usize,
    unresolved: bool,
}

fn derive_session_ownerships(nodes: &[LineageNode<'_>]) -> Vec<SessionOwnership> {
    let mut files_by_thread_id = HashMap::<&str, Vec<usize>>::new();
    for (index, node) in nodes.iter().enumerate() {
        files_by_thread_id
            .entry(node.thread_id)
            .or_default()
            .push(index);
    }

    nodes
        .iter()
        .enumerate()
        .map(|(child_index, child)| {
            // parent_thread_id is deliberately not considered here. It models
            // agent ownership, whereas only forked_from_id proves copied history.
            let _agent_parent_id = child.agent_parent_id;
            if !child.identity_trusted {
                return SessionOwnership {
                    unresolved: true,
                    ..SessionOwnership::default()
                };
            }
            let Some(history_parent_id) = child.history_parent_id else {
                return SessionOwnership::default();
            };
            let Some(parent_candidates) = files_by_thread_id.get(history_parent_id) else {
                return SessionOwnership {
                    unresolved: true,
                    ..SessionOwnership::default()
                };
            };
            let trusted_parent_candidates = parent_candidates
                .iter()
                .copied()
                .filter(|parent_index| {
                    *parent_index != child_index && nodes[*parent_index].identity_trusted
                })
                .collect::<Vec<_>>();
            if trusted_parent_candidates.len() != 1
                || lineage_has_cycle(child_index, nodes, &files_by_thread_id)
            {
                return SessionOwnership {
                    unresolved: true,
                    ..SessionOwnership::default()
                };
            }
            let parent = nodes[trusted_parent_candidates[0]];
            let inherited_record_count = [0usize, 1usize]
                .into_iter()
                .filter(|parent_start| *parent_start < parent.record_hashes.len())
                .map(|parent_start| {
                    matching_record_prefix(
                        &child.record_hashes[1..],
                        &parent.record_hashes[parent_start..],
                        child.record_resync_window,
                    )
                })
                .max()
                .unwrap_or(0);
            if inherited_record_count == 0 {
                return SessionOwnership {
                    unresolved: true,
                    ..SessionOwnership::default()
                };
            }
            SessionOwnership {
                inherited_record_end: inherited_record_count.saturating_add(1),
                unresolved: false,
            }
        })
        .collect()
}

fn matching_record_prefix(child: &[u64], parent: &[u64], resync_window: usize) -> usize {
    matching_record_boundary(child, parent, resync_window).0
}

fn matching_record_boundary(child: &[u64], parent: &[u64], resync_window: usize) -> (usize, usize) {
    let mut child_index = 0usize;
    let mut parent_index = 0usize;
    while child_index < child.len() && parent_index < parent.len() {
        if child[child_index] == parent[parent_index] {
            child_index += 1;
            parent_index += 1;
            continue;
        }

        // Fork replay can regenerate, insert, or omit a small number of
        // envelope records. Re-synchronise only after a run of normalized,
        // high-entropy records confirms the alignment, so a genuine branch
        // point remains a conservative stop boundary.
        let child_search_end = child
            .len()
            .saturating_sub(FORK_MATCH_ANCHOR_RECORDS)
            .min(child_index.saturating_add(resync_window));
        let parent_search_end = parent
            .len()
            .saturating_sub(FORK_MATCH_ANCHOR_RECORDS)
            .min(parent_index.saturating_add(resync_window));
        let mut best_alignment = None::<(usize, usize, usize)>;
        for next_child in child_index..=child_search_end {
            for next_parent in parent_index..=parent_search_end {
                if next_child == child_index && next_parent == parent_index {
                    continue;
                }
                let child_anchor_end = next_child + FORK_MATCH_ANCHOR_RECORDS;
                let parent_anchor_end = next_parent + FORK_MATCH_ANCHOR_RECORDS;
                if child[next_child..child_anchor_end] != parent[next_parent..parent_anchor_end] {
                    continue;
                }
                let child_skip = next_child - child_index;
                let parent_skip = next_parent - parent_index;
                let candidate = (
                    child_skip.saturating_add(parent_skip),
                    next_child,
                    next_parent,
                );
                if best_alignment
                    .map(|current| candidate < current)
                    .unwrap_or(true)
                {
                    best_alignment = Some(candidate);
                }
            }
        }
        let Some((_, next_child, next_parent)) = best_alignment else {
            break;
        };
        child_index = next_child;
        parent_index = next_parent;
    }
    (child_index, parent_index)
}

fn lineage_has_cycle(
    start_index: usize,
    nodes: &[LineageNode<'_>],
    files_by_thread_id: &HashMap<&str, Vec<usize>>,
) -> bool {
    let mut visited = HashSet::<usize>::new();
    let mut current_index = start_index;
    while visited.insert(current_index) {
        let Some(parent_id) = nodes[current_index].history_parent_id else {
            return false;
        };
        let Some(candidates) = files_by_thread_id.get(parent_id) else {
            return false;
        };
        if candidates.len() != 1 {
            return false;
        }
        current_index = candidates[0];
    }
    true
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
    let mut cache = COST_ANALYTICS_CACHE
        .get_or_init(|| Mutex::new(CostAnalyticsCache::default()))
        .lock()
        .unwrap_or_else(|poisoned| {
            log::warn!("成本分析缓存锁异常，继续使用恢复后的缓存");
            poisoned.into_inner()
        });
    Ok(scan_codex_cost_analytics_roots_with_cache(
        &roots,
        now_unix_seconds(),
        weekly_budget_usd,
        &mut cache,
        on_progress,
    ))
}

#[cfg(test)]
fn scan_codex_cost_analytics_roots_with_progress<F>(
    roots: &[PathBuf],
    now: i64,
    weekly_budget_usd: Option<f64>,
    on_progress: F,
) -> CodexCostAnalyticsSnapshot
where
    F: FnMut(CodexCostAnalyticsProgress),
{
    let mut cache = CostAnalyticsCache::default();
    scan_codex_cost_analytics_roots_with_cache(
        roots,
        now,
        weekly_budget_usd,
        &mut cache,
        on_progress,
    )
}

fn scan_codex_cost_analytics_roots_with_cache<F>(
    roots: &[PathBuf],
    now: i64,
    weekly_budget_usd: Option<f64>,
    cache: &mut CostAnalyticsCache,
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
    files.sort();
    files.dedup();
    on_progress(cost_analytics_progress("scanning", 0, files.len(), None));

    let source_path_count = files.len();
    let sources = files
        .into_iter()
        .map(|path| {
            let fingerprint = match fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() => Some(LogFileFingerprint {
                    length: metadata.len(),
                    modified_at: metadata.modified().ok(),
                }),
                Ok(_) | Err(_) => {
                    failed_path_count += 1;
                    None
                }
            };
            (path, fingerprint)
        })
        .collect::<Vec<_>>();

    let source_paths = sources
        .iter()
        .filter_map(|(path, fingerprint)| fingerprint.as_ref().map(|_| path))
        .collect::<HashSet<_>>();
    let cached_file_count = cache.files.len();
    cache.files.retain(|path, _| source_paths.contains(path));
    let mut stats = CostAnalyticsCacheStats {
        evicted_files: cached_file_count.saturating_sub(cache.files.len()),
        ..CostAnalyticsCacheStats::default()
    };

    for (index, (path, fingerprint)) in sources.iter().enumerate() {
        if let Some(fingerprint) = fingerprint {
            let cache_hit = cache
                .files
                .get(path)
                .map(|cached| cached.fingerprint == *fingerprint)
                .unwrap_or(false);
            if cache_hit {
                stats.reused_files += 1;
            } else {
                let appended = cache
                    .files
                    .get_mut(path)
                    .and_then(|cached| {
                        append_cost_analytics_session_file(path, fingerprint, cached).ok()
                    })
                    .unwrap_or(false);
                if appended {
                    stats.appended_files += 1;
                } else {
                    match parse_cost_analytics_session_file(path) {
                        Ok(cached) => {
                            cache.files.insert(path.clone(), cached);
                            stats.reparsed_files += 1;
                        }
                        Err(_) => {
                            cache.files.remove(path);
                            failed_path_count += 1;
                        }
                    }
                }
            }
        }
        on_progress(cost_analytics_progress(
            "scanning",
            index + 1,
            source_path_count,
            Some(path.to_string_lossy().to_string()),
        ));
    }

    cache.last_scan = stats;
    log::debug!(
        "成本分析增量缓存：复用 {}，追加 {}，重解析 {}，淘汰 {}",
        stats.reused_files,
        stats.appended_files,
        stats.reparsed_files,
        stats.evicted_files
    );

    let last_7d_date_range = previous_complete_local_date_range(now);
    let heatmap_start = now.saturating_sub(7 * DAY_SECONDS);
    let mut parsed_files = sources
        .iter()
        .filter_map(|(path, _)| {
            cache
                .files
                .get(path)
                .map(|cached| cached.state.parsed.clone())
        })
        .collect::<Vec<_>>();

    let (removed_replayed_event_count, unresolved_fork_count, unresolved_usage_event_count) =
        apply_analytics_lineage_ownership(&mut parsed_files);
    if removed_replayed_event_count > 0 {
        log::info!(
            "成本分析已排除 {removed_replayed_event_count} 条 fork 会话复制的历史 Token 事件"
        );
    }

    let mut event_count = 0usize;
    let mut sessions = Vec::new();
    let mut projects = BTreeMap::<String, ProjectAccumulator>::new();
    let mut prompts = BTreeMap::<String, PromptAccumulator>::new();
    let mut heatmap = initial_heatmap();
    let mut total = CodexTokenTotals::default();
    let mut total_cost_usd = 0.0;
    let mut last_7d = CodexTokenTotals::default();
    let mut last_7d_cost_usd = 0.0;
    let mut budget_period_cost_usd = 0.0;

    for parsed in parsed_files {
        let ParsedAnalyticsSessionFile {
            session,
            events,
            prompt_keys,
            ..
        } = parsed;
        let project_entry = projects
            .entry(session.project_path.clone())
            .or_insert_with(|| ProjectAccumulator::new(&session.project_path));
        project_entry.session_count += 1;
        project_entry.prompt_keys.extend(prompt_keys);
        project_entry.event_count += session.event_count;
        project_entry.total.add(&session.total);
        project_entry.cost_usd += session.cost_usd;
        project_entry.last_at = max_option_i64(project_entry.last_at, session.updated_at);

        total.add(&session.total);
        total_cost_usd += session.cost_usd;
        sessions.push(session);

        for event in events {
            event_count += 1;
            if last_7d_date_range
                .and_then(|(start, end)| {
                    local_date_at(event.timestamp).map(|date| date >= start && date <= end)
                })
                .unwrap_or(false)
            {
                last_7d.add(&event.total);
                last_7d_cost_usd += event.cost_usd;
            }
            if event.timestamp >= heatmap_start {
                budget_period_cost_usd += event.cost_usd;
                if let Some(bucket_key) = heatmap_bucket_key(event.timestamp) {
                    if let Some(bucket) = heatmap.get_mut(&bucket_key) {
                        bucket.calls += 1;
                        bucket.tokens = bucket.tokens.saturating_add(event.total.total_tokens);
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
        }
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
        source_path_count,
        failed_path_count,
        unresolved_fork_count,
        unresolved_usage_event_count,
        event_count,
        total,
        total_cost_usd: round_cost(total_cost_usd),
        local_total_cost_usd: round_cost(total_cost_usd),
        last_7d,
        last_7d_cost_usd: round_cost(last_7d_cost_usd),
        local_last_7d_cost_usd: round_cost(last_7d_cost_usd),
        budget_period_cost_usd: round_cost(budget_period_cost_usd),
        local_budget_period_cost_usd: round_cost(budget_period_cost_usd),
        cost_source: COST_SOURCE_LOCAL.to_string(),
        cost_source_updated_at: Some(now),
        cost_source_error: None,
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
            (snapshot.budget_period_cost_usd / budget) * 100.0
        }
    });
    snapshot.weekly_budget_usd = weekly_budget_usd;
    snapshot.weekly_budget_percent = weekly_budget_percent.map(round_percent);
    snapshot.weekly_budget_alert = weekly_budget_alert(weekly_budget_percent);
    snapshot
}

fn normalize_cost_source_fields(snapshot: &mut CodexCostAnalyticsSnapshot) {
    if snapshot.cost_source.trim().is_empty() {
        snapshot.local_total_cost_usd = snapshot.total_cost_usd;
        snapshot.local_last_7d_cost_usd = snapshot.last_7d_cost_usd;
        snapshot.local_budget_period_cost_usd = snapshot.budget_period_cost_usd;
        snapshot.cost_source = COST_SOURCE_LOCAL.to_string();
        snapshot.cost_source_updated_at = Some(snapshot.updated_at);
    }
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
        let mut snapshot = cache.snapshot;
        normalize_cost_source_fields(&mut snapshot);
        return Ok(Some(apply_codex_cost_analytics_budget(
            snapshot,
            weekly_budget_usd,
        )));
    }

    let snapshot = serde_json::from_str::<CodexCostAnalyticsSnapshot>(raw)
        .map_err(|error| format!("解析成本分析缓存失败: {error}"))?;
    let mut snapshot = snapshot;
    normalize_cost_source_fields(&mut snapshot);
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

#[derive(Debug, Clone)]
struct ParsedTokenSessionFile {
    session_id: String,
    payload_session_id: Option<String>,
    filename_session_id: Option<String>,
    parent_session_id: Option<String>,
    parent_thread_id: Option<String>,
    identity_trusted: bool,
    header_checked: bool,
    record_hashes: Vec<u64>,
    events: Vec<ParsedTokenEvent>,
    latest_session: Option<CodexTokenSessionUsage>,
}

#[derive(Clone)]
struct ParsedAnalyticsSessionFile {
    session: CodexSessionCostBreakdown,
    payload_session_id: Option<String>,
    filename_session_id: Option<String>,
    parent_thread_id: Option<String>,
    identity_trusted: bool,
    record_hashes: Vec<u64>,
    events: Vec<AnalyticsTokenEvent>,
    prompt_keys: Vec<String>,
}

#[derive(Clone)]
struct AnalyticsTokenEvent {
    record_index: usize,
    timestamp: i64,
    session_id: String,
    project_path: String,
    project_name: String,
    model: String,
    prompt_key: String,
    prompt_preview: String,
    prompt_chars: usize,
    total: CodexTokenTotals,
    cumulative_total: Option<CodexTokenTotals>,
    cost_usd: f64,
    source_path: String,
}

struct AnalyticsFileState {
    parsed: ParsedAnalyticsSessionFile,
    header_checked: bool,
    current_model: String,
    current_prompt_preview: String,
    current_prompt_chars: usize,
    current_prompt_index: usize,
    prompt_key_seen: HashSet<String>,
    model_tokens: HashMap<String, u64>,
    cost_usd: f64,
}

impl AnalyticsFileState {
    fn new(path: &Path) -> Self {
        let source_path = path.to_string_lossy().to_string();
        let filename_session_id = session_id_from_rollout_filename(path);
        let session_id = filename_session_id.clone().unwrap_or_else(|| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("unknown-session")
                .to_string()
        });
        let project_path = "(unknown project)".to_string();

        Self {
            parsed: ParsedAnalyticsSessionFile {
                session: CodexSessionCostBreakdown {
                    session_id,
                    parent_session_id: None,
                    project_name: project_name_from_path(&project_path),
                    project_path,
                    started_at: None,
                    updated_at: None,
                    duration_seconds: None,
                    prompt_count: 0,
                    event_count: 0,
                    model: "unknown".to_string(),
                    total: CodexTokenTotals::default(),
                    cost_usd: 0.0,
                    source_path,
                },
                payload_session_id: None,
                filename_session_id,
                parent_thread_id: None,
                identity_trusted: false,
                record_hashes: Vec::new(),
                events: Vec::new(),
                prompt_keys: Vec::new(),
            },
            header_checked: false,
            current_model: "unknown".to_string(),
            current_prompt_preview: "(no prompt captured)".to_string(),
            current_prompt_chars: 0,
            current_prompt_index: 0,
            prompt_key_seen: HashSet::new(),
            model_tokens: HashMap::new(),
            cost_usd: 0.0,
        }
    }

    fn refresh_session_summary(&mut self) {
        let session = &mut self.parsed.session;
        session.project_name = project_name_from_path(&session.project_path);
        session.duration_seconds = match (session.started_at, session.updated_at) {
            (Some(start), Some(end)) if end >= start => Some(end - start),
            _ => None,
        };
        session.prompt_count = self.parsed.prompt_keys.len();
        session.event_count = self.parsed.events.len();
        session.model = self
            .model_tokens
            .iter()
            .max_by_key(|(_, tokens)| **tokens)
            .map(|(model, _)| model.clone())
            .unwrap_or_else(|| "unknown".to_string());
        session.cost_usd = round_cost(self.cost_usd);
    }
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

fn parse_cost_analytics_session_file(path: &Path) -> Result<CachedCostAnalyticsFile, String> {
    let source_metadata =
        fs::metadata(path).map_err(|error| format!("读取 Codex 日志元数据失败: {error}"))?;
    let captured_length = source_metadata.len();
    let modified_at = source_metadata.modified().ok();
    let file = fs::File::open(path).map_err(|error| format!("读取 Codex 日志失败: {error}"))?;
    let mut reader = BufReader::new(file);
    let mut state = AnalyticsFileState::new(path);
    {
        let mut bounded_reader = Read::by_ref(&mut reader).take(captured_length);
        parse_cost_analytics_lines(&mut bounded_reader, &mut state)?;
    }
    state.refresh_session_summary();

    let parsed_length = reader
        .stream_position()
        .map_err(|error| format!("读取成本分析日志位置失败: {error}"))?;
    let tail_signature = log_tail_signature(path, parsed_length)?;

    Ok(CachedCostAnalyticsFile {
        fingerprint: LogFileFingerprint {
            length: parsed_length,
            modified_at,
        },
        tail_signature,
        state,
    })
}

fn append_cost_analytics_session_file(
    path: &Path,
    source_fingerprint: &LogFileFingerprint,
    cached: &mut CachedCostAnalyticsFile,
) -> Result<bool, String> {
    let parsed_length = cached.fingerprint.length;
    if source_fingerprint.length <= parsed_length {
        return Ok(false);
    }
    if parsed_length > 0 && cached.tail_signature.last() != Some(&b'\n') {
        return Ok(false);
    }
    if log_tail_signature(path, parsed_length)? != cached.tail_signature {
        return Ok(false);
    }

    let mut file = fs::File::open(path).map_err(|error| format!("读取 Codex 日志失败: {error}"))?;
    file.seek(SeekFrom::Start(parsed_length))
        .map_err(|error| format!("定位成本分析日志增量失败: {error}"))?;
    let mut reader = BufReader::new(file);
    {
        let captured_append_length = source_fingerprint.length.saturating_sub(parsed_length);
        let mut bounded_reader = Read::by_ref(&mut reader).take(captured_append_length);
        parse_cost_analytics_lines(&mut bounded_reader, &mut cached.state)?;
    }
    cached.state.refresh_session_summary();

    let next_length = reader
        .stream_position()
        .map_err(|error| format!("读取成本分析日志增量位置失败: {error}"))?;
    let modified_at = fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok());
    cached.fingerprint = LogFileFingerprint {
        length: next_length,
        modified_at,
    };
    cached.tail_signature = log_tail_signature(path, next_length)?;
    Ok(true)
}

fn parse_cost_analytics_lines<R: BufRead>(
    reader: &mut R,
    state: &mut AnalyticsFileState,
) -> Result<(), String> {
    loop {
        let mut line = String::new();
        let read = reader
            .read_line(&mut line)
            .map_err(|error| format!("读取成本分析日志行失败: {error}"))?;
        if read == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let is_header = !state.header_checked;
        if is_header {
            state.header_checked = true;
        }
        let root = match serde_json::from_str::<Value>(&line) {
            Ok(root) => root,
            Err(_) => continue,
        };
        let timestamp = root
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(parse_timestamp);
        let record_index = state.parsed.record_hashes.len();
        state.parsed.record_hashes.push(fast_record_hash(&line));
        let root_type = root.get("type").and_then(Value::as_str).unwrap_or_default();
        let payload = root.get("payload").unwrap_or(&Value::Null);

        if is_header && root_type == "session_meta" {
            let metadata = parse_session_metadata_payload(payload);
            if let Some(id) = metadata
                .payload_id
                .as_ref()
                .or(metadata.payload_session_id.as_ref())
            {
                state.parsed.session.session_id = id.clone();
                state.parsed.identity_trusted = state
                    .parsed
                    .filename_session_id
                    .as_ref()
                    .map(|filename_id| filename_id == id)
                    .unwrap_or(true);
            }
            state.parsed.payload_session_id = metadata.payload_session_id;
            state.parsed.session.parent_session_id = metadata.forked_from_id;
            state.parsed.parent_thread_id = metadata.parent_thread_id;
            if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
                state.parsed.session.project_path = cwd.to_string();
            }
            if let Some(model) = payload.get("model").and_then(Value::as_str) {
                state.current_model = model.to_string();
            }
            continue;
        }
        if root_type == "session_meta" {
            // Forked rollout files may replay the parent's session_meta and even
            // older ancestors. The first physical record names this file's
            // canonical session; later metadata belongs to copied history.
            continue;
        }

        if root_type == "turn_context" {
            if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
                state.parsed.session.project_path = cwd.to_string();
            }
            if let Some(model) = payload.get("model").and_then(Value::as_str) {
                state.current_model = model.to_string();
            }
            continue;
        }

        if root_type == "event_msg"
            && payload.get("type").and_then(Value::as_str) == Some("user_message")
        {
            if let Some(prompt) = payload.get("message").and_then(Value::as_str) {
                state.current_prompt_index += 1;
                state.current_prompt_chars = prompt.chars().count();
                state.current_prompt_preview = prompt_preview(prompt);
            }
            continue;
        }

        if root_type == "response_item"
            && payload.get("type").and_then(Value::as_str) == Some("message")
            && payload.get("role").and_then(Value::as_str) == Some("user")
        {
            if let Some(prompt) = message_payload_text(payload) {
                state.current_prompt_index += 1;
                state.current_prompt_chars = prompt.chars().count();
                state.current_prompt_preview = prompt_preview(&prompt);
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
        let Some(info) = payload.get("info") else {
            continue;
        };
        let last = parse_local_token_event_usage(info);
        let cumulative_total = info.get("total_token_usage").and_then(parse_token_totals);
        if last.is_none() && cumulative_total.is_none() {
            continue;
        }
        let provisional_usage = last
            .clone()
            .or_else(|| cumulative_total.clone())
            .unwrap_or_default();

        // This provisional value keeps the append-only parser state coherent.
        // The scan derives confirmed usage from cumulative deltas only after
        // direct-parent ownership is known.
        let event_cost_usd =
            estimate_token_cost_usd(&state.current_model, timestamp, &provisional_usage);
        let session_id = state.parsed.session.session_id.clone();
        let project_path = state.parsed.session.project_path.clone();
        let project_name = project_name_from_path(&project_path);
        let prompt_key = format!("{session_id}:{}", state.current_prompt_index);
        if state.prompt_key_seen.insert(prompt_key.clone()) {
            state.parsed.prompt_keys.push(prompt_key.clone());
        }

        state.parsed.session.total.add(&provisional_usage);
        state.cost_usd += event_cost_usd;
        state.parsed.session.started_at = Some(
            state
                .parsed
                .session
                .started_at
                .map(|current: i64| current.min(timestamp))
                .unwrap_or(timestamp),
        );
        state.parsed.session.updated_at = Some(
            state
                .parsed
                .session
                .updated_at
                .map(|current: i64| current.max(timestamp))
                .unwrap_or(timestamp),
        );
        *state
            .model_tokens
            .entry(state.current_model.clone())
            .or_insert(0) += provisional_usage.total_tokens;

        state.parsed.events.push(AnalyticsTokenEvent {
            record_index,
            timestamp,
            session_id,
            project_path,
            project_name,
            model: state.current_model.clone(),
            prompt_key,
            prompt_preview: state.current_prompt_preview.clone(),
            prompt_chars: state.current_prompt_chars,
            total: provisional_usage,
            cumulative_total,
            cost_usd: event_cost_usd,
            source_path: state.parsed.session.source_path.clone(),
        });
    }
    Ok(())
}

fn apply_analytics_lineage_ownership(
    files: &mut [ParsedAnalyticsSessionFile],
) -> (usize, usize, usize) {
    let lineage_nodes = files
        .iter()
        .map(|file| LineageNode {
            thread_id: &file.session.session_id,
            history_parent_id: file.session.parent_session_id.as_deref(),
            agent_parent_id: file.parent_thread_id.as_deref(),
            identity_trusted: file.identity_trusted,
            record_hashes: &file.record_hashes,
            record_resync_window: FORK_MATCH_RESYNC_WINDOW,
        })
        .collect::<Vec<_>>();
    let ownerships = derive_session_ownerships(&lineage_nodes);

    let mut removed_event_count = 0usize;
    let mut unresolved_fork_count = 0usize;
    let mut unresolved_usage_event_count = 0usize;
    for (file, ownership) in files.iter_mut().zip(ownerships) {
        if ownership.unresolved {
            unresolved_fork_count += 1;
            removed_event_count += file.events.len();
            file.events.clear();
            refresh_analytics_session_summary(file);
            continue;
        }
        if ownership.inherited_record_end == 0 {
            unresolved_usage_event_count +=
                apply_cumulative_deltas_to_analytics(file, ownership.inherited_record_end);
            refresh_analytics_session_summary(file);
            continue;
        }
        let inherited_event_count = file
            .events
            .iter()
            .filter(|event| event.record_index < ownership.inherited_record_end)
            .count();
        unresolved_usage_event_count +=
            apply_cumulative_deltas_to_analytics(file, ownership.inherited_record_end);
        removed_event_count += inherited_event_count;
        refresh_analytics_session_summary(file);
    }
    (
        removed_event_count,
        unresolved_fork_count,
        unresolved_usage_event_count,
    )
}

fn apply_cumulative_deltas_to_analytics(
    file: &mut ParsedAnalyticsSessionFile,
    inherited_record_end: usize,
) -> usize {
    if !file
        .events
        .iter()
        .any(|event| event.cumulative_total.is_some())
    {
        file.events
            .retain(|event| event.record_index >= inherited_record_end);
        for event in &mut file.events {
            event.cost_usd = estimate_token_cost_usd(&event.model, event.timestamp, &event.total);
        }
        return 0;
    }

    let inherited_baseline = file
        .events
        .iter()
        .filter(|event| event.record_index < inherited_record_end)
        .filter_map(|event| event.cumulative_total.as_ref())
        .last()
        .cloned();
    let inherited_token_event_count = file
        .events
        .iter()
        .filter(|event| event.record_index < inherited_record_end)
        .count();
    if inherited_record_end > 0 && inherited_token_event_count > 0 && inherited_baseline.is_none() {
        let unresolved_event_count = file
            .events
            .iter()
            .filter(|event| event.record_index >= inherited_record_end)
            .count();
        file.events.clear();
        return unresolved_event_count;
    }

    let mut previous_total = inherited_baseline.unwrap_or_default();
    let mut unresolved_event_count = 0usize;
    let mut confirmed_events = Vec::new();
    for mut event in file.events.drain(..) {
        if event.record_index < inherited_record_end {
            continue;
        }
        let Some(current_total) = event.cumulative_total.as_ref() else {
            unresolved_event_count += 1;
            continue;
        };
        let Some(delta) = token_totals_delta(current_total, &previous_total) else {
            unresolved_event_count += 1;
            previous_total = current_total.clone();
            continue;
        };
        previous_total = current_total.clone();
        if delta.is_empty() {
            continue;
        }
        event.total = delta;
        event.cost_usd = estimate_token_cost_usd(&event.model, event.timestamp, &event.total);
        confirmed_events.push(event);
    }
    file.events = confirmed_events;
    unresolved_event_count
}

fn refresh_analytics_session_summary(parsed: &mut ParsedAnalyticsSessionFile) {
    let fallback_model = parsed.session.model.clone();
    let mut prompt_key_seen = HashSet::<String>::new();
    let mut prompt_keys = Vec::new();
    let mut model_tokens = HashMap::<String, u64>::new();
    let mut total = CodexTokenTotals::default();
    let mut cost_usd = 0.0;
    let mut started_at = None;
    let mut updated_at = None;

    for event in &parsed.events {
        if prompt_key_seen.insert(event.prompt_key.clone()) {
            prompt_keys.push(event.prompt_key.clone());
        }
        total.add(&event.total);
        cost_usd += event.cost_usd;
        started_at = Some(
            started_at
                .map(|current: i64| current.min(event.timestamp))
                .unwrap_or(event.timestamp),
        );
        updated_at = Some(
            updated_at
                .map(|current: i64| current.max(event.timestamp))
                .unwrap_or(event.timestamp),
        );
        *model_tokens.entry(event.model.clone()).or_insert(0) += event.total.total_tokens;
    }

    parsed.prompt_keys = prompt_keys;
    parsed.session.started_at = started_at;
    parsed.session.updated_at = updated_at;
    parsed.session.duration_seconds = match (started_at, updated_at) {
        (Some(start), Some(end)) if end >= start => Some(end - start),
        _ => None,
    };
    parsed.session.prompt_count = parsed.prompt_keys.len();
    parsed.session.event_count = parsed.events.len();
    parsed.session.model = model_tokens
        .into_iter()
        .max_by_key(|(_, tokens)| *tokens)
        .map(|(model, _)| model)
        .unwrap_or(fallback_model);
    parsed.session.total = total;
    parsed.session.cost_usd = round_cost(cost_usd);
}

fn parse_token_session_file(path: &Path) -> Result<CachedTokenUsageFile, String> {
    let source_metadata =
        fs::metadata(path).map_err(|error| format!("读取 Codex 日志元数据失败: {error}"))?;
    let captured_length = source_metadata.len();
    let modified_at = source_metadata.modified().ok();
    let file = fs::File::open(path).map_err(|error| format!("读取 Codex 日志失败: {error}"))?;
    let mut reader = BufReader::new(file);
    let filename_session_id = session_id_from_rollout_filename(path);
    let fallback_session_id = filename_session_id.clone().unwrap_or_else(|| {
        path.file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("unknown-session")
            .to_string()
    });
    let mut parsed = ParsedTokenSessionFile {
        session_id: fallback_session_id,
        payload_session_id: None,
        filename_session_id,
        parent_session_id: None,
        parent_thread_id: None,
        identity_trusted: false,
        header_checked: false,
        record_hashes: Vec::new(),
        events: Vec::new(),
        latest_session: None,
    };

    {
        let mut bounded_reader = Read::by_ref(&mut reader).take(captured_length);
        parse_token_session_lines(&mut bounded_reader, &mut parsed)?;
    }
    let parsed_length = reader
        .stream_position()
        .map_err(|error| format!("读取 Codex 日志位置失败: {error}"))?;
    let tail_signature = log_tail_signature(path, parsed_length)?;

    Ok(CachedTokenUsageFile {
        fingerprint: LogFileFingerprint {
            length: parsed_length,
            modified_at,
        },
        tail_signature,
        parsed: {
            parsed.latest_session = latest_token_session_usage(&parsed.events);
            parsed
        },
    })
}

fn append_token_session_file(
    path: &Path,
    source_fingerprint: &LogFileFingerprint,
    cached: &mut CachedTokenUsageFile,
) -> Result<bool, String> {
    let parsed_length = cached.fingerprint.length;
    if source_fingerprint.length <= parsed_length {
        return Ok(false);
    }
    if parsed_length > 0 && cached.tail_signature.last() != Some(&b'\n') {
        return Ok(false);
    }
    if log_tail_signature(path, parsed_length)? != cached.tail_signature {
        return Ok(false);
    }

    let mut file = fs::File::open(path).map_err(|error| format!("读取 Codex 日志失败: {error}"))?;
    file.seek(SeekFrom::Start(parsed_length))
        .map_err(|error| format!("定位 Codex 日志增量失败: {error}"))?;
    let mut reader = BufReader::new(file);
    {
        let captured_append_length = source_fingerprint.length.saturating_sub(parsed_length);
        let mut bounded_reader = Read::by_ref(&mut reader).take(captured_append_length);
        parse_token_session_lines(&mut bounded_reader, &mut cached.parsed)?;
    }
    let next_length = reader
        .stream_position()
        .map_err(|error| format!("读取 Codex 日志增量位置失败: {error}"))?;
    let modified_at = fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok());

    cached.parsed.latest_session = latest_token_session_usage(&cached.parsed.events);
    cached.fingerprint = LogFileFingerprint {
        length: next_length,
        modified_at,
    };
    cached.tail_signature = log_tail_signature(path, next_length)?;
    Ok(true)
}

fn parse_token_session_lines<R: BufRead>(
    reader: &mut R,
    parsed: &mut ParsedTokenSessionFile,
) -> Result<(), String> {
    loop {
        let mut line = String::new();
        let read = reader
            .read_line(&mut line)
            .map_err(|error| format!("读取 Codex 日志行失败: {error}"))?;
        if read == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let is_header = !parsed.header_checked;
        if is_header {
            parsed.header_checked = true;
        }
        let record_index = parsed.record_hashes.len();
        parsed.record_hashes.push(fast_record_hash(&line));

        if let Some(metadata) = is_header
            .then(|| parse_session_metadata_line(&line))
            .flatten()
        {
            if is_header {
                if let Some(session_id) = metadata
                    .payload_id
                    .as_ref()
                    .or(metadata.payload_session_id.as_ref())
                {
                    parsed.session_id = session_id.clone();
                    parsed.identity_trusted = parsed
                        .filename_session_id
                        .as_ref()
                        .map(|filename_id| filename_id == session_id)
                        .unwrap_or(true);
                }
                parsed.payload_session_id = metadata.payload_session_id;
                parsed.parent_session_id = metadata.forked_from_id;
                parsed.parent_thread_id = metadata.parent_thread_id;
            }
            continue;
        }
        if let Some(event) = parse_token_event_line_at(&line, record_index) {
            parsed.events.push(event);
        }
    }
    Ok(())
}

fn latest_token_session_usage(events: &[ParsedTokenEvent]) -> Option<CodexTokenSessionUsage> {
    let mut session = ParsedSession::default();
    for event in events {
        session.observe(event);
    }
    session.into_latest_session()
}

fn log_tail_signature(path: &Path, length: u64) -> Result<Vec<u8>, String> {
    if length == 0 {
        return Ok(Vec::new());
    }

    let start = length.saturating_sub(TOKEN_USAGE_TAIL_SIGNATURE_BYTES);
    let signature_length = usize::try_from(length.saturating_sub(start))
        .map_err(|error| format!("Codex 日志尾部签名长度无效: {error}"))?;
    let mut file = fs::File::open(path).map_err(|error| format!("读取 Codex 日志失败: {error}"))?;
    file.seek(SeekFrom::Start(start))
        .map_err(|error| format!("定位 Codex 日志尾部失败: {error}"))?;
    let mut signature = vec![0; signature_length];
    file.read_exact(&mut signature)
        .map_err(|error| format!("读取 Codex 日志尾部失败: {error}"))?;
    Ok(signature)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ParsedSessionMetadata {
    payload_id: Option<String>,
    payload_session_id: Option<String>,
    forked_from_id: Option<String>,
    parent_thread_id: Option<String>,
}

fn parse_session_metadata_payload(payload: &Value) -> ParsedSessionMetadata {
    ParsedSessionMetadata {
        payload_id: payload
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string),
        payload_session_id: payload
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        forked_from_id: payload
            .get("forked_from_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        parent_thread_id: payload
            .get("parent_thread_id")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

fn parse_session_metadata_root(root: &Value) -> Option<ParsedSessionMetadata> {
    (root.get("type")?.as_str()? == "session_meta")
        .then(|| parse_session_metadata_payload(root.get("payload").unwrap_or(&Value::Null)))
}

fn parse_session_metadata_line(line: &str) -> Option<ParsedSessionMetadata> {
    if !line.contains("\"session_meta\"") {
        return None;
    }
    let root = serde_json::from_str::<Value>(line).ok()?;
    parse_session_metadata_root(&root)
}

fn fast_record_hash(line: &str) -> u64 {
    const FULL_HASH_BYTES: usize = 64 * 1024;
    const EDGE_SAMPLE_BYTES: usize = 16 * 1024;

    fn field_range(line: &str, key: &str, quoted_value: bool) -> Option<(usize, usize)> {
        let pattern = format!("\"{key}\":");
        let key_start = line.find(&pattern)?;
        let value_start = key_start + pattern.len();
        let mut value_end = value_start;
        if quoted_value {
            if line.as_bytes().get(value_start) != Some(&b'"') {
                return None;
            }
            value_end += 1;
            let bytes = line.as_bytes();
            while value_end < bytes.len() {
                if bytes[value_end] == b'"' && bytes[value_end.saturating_sub(1)] != b'\\' {
                    value_end += 1;
                    break;
                }
                value_end += 1;
            }
        } else {
            while value_end < line.len() && !matches!(line.as_bytes()[value_end], b',' | b'}') {
                value_end += 1;
            }
        }

        let mut start = key_start;
        let mut end = value_end;
        if start > 0 && line.as_bytes()[start - 1] == b',' {
            start -= 1;
        } else if line.as_bytes().get(end) == Some(&b',') {
            end += 1;
        }
        Some((start, end))
    }

    fn hash_range_excluding(
        line: &str,
        start: usize,
        end: usize,
        ranges: &[(usize, usize)],
        hasher: &mut StableFnvHasher,
    ) {
        let mut cursor = start;
        for (skip_start, skip_end) in ranges {
            if *skip_end <= start || *skip_start >= end {
                continue;
            }
            let clipped_start = (*skip_start).max(start);
            let clipped_end = (*skip_end).min(end);
            if clipped_start > cursor {
                hasher.write(line[cursor..clipped_start].as_bytes());
            }
            cursor = cursor.max(clipped_end);
        }
        if cursor < end {
            hasher.write(line[cursor..end].as_bytes());
        }
    }

    fn floor_char_boundary(value: &str, mut index: usize) -> usize {
        index = index.min(value.len());
        while index > 0 && !value.is_char_boundary(index) {
            index -= 1;
        }
        index
    }

    fn ceil_char_boundary(value: &str, mut index: usize) -> usize {
        index = index.min(value.len());
        while index < value.len() && !value.is_char_boundary(index) {
            index += 1;
        }
        index
    }

    let mut ranges = Vec::<(usize, usize)>::new();
    if let Some(range) = field_range(line, "timestamp", true) {
        ranges.push(range);
    }
    let probe_end = floor_char_boundary(line, line.len().min(2 * 1024));
    let probe = &line[..probe_end];
    if line.len() <= FULL_HASH_BYTES {
        for (key, expected) in [
            ("local_audio", "[]"),
            ("cache_write_input_tokens", "0"),
            ("spend_control_reached", "null"),
            ("history_mode", "\"legacy\""),
            ("history_base", "null"),
            ("subagent_history_start_ordinal", "null"),
            ("parent_thread_id", "null"),
        ] {
            let mut search_start = 0usize;
            let pattern = format!("\"{key}\":{expected}");
            while let Some(relative_start) = line[search_start..].find(&pattern) {
                let key_start = search_start + relative_start;
                if let Some((start, end)) = field_range(&line[key_start..], key, false) {
                    ranges.push((key_start + start, key_start + end));
                }
                search_start = key_start.saturating_add(pattern.len());
            }
        }
    }
    if probe.contains("\"type\":\"response_item\"") {
        if let Some(range) = field_range(line, "id", true) {
            ranges.push(range);
        }
        if line.len() <= FULL_HASH_BYTES && probe.contains("\"content\":null") {
            if let Some(range) = field_range(line, "content", false) {
                ranges.push(range);
            }
        }
    }
    ranges.sort_unstable();
    ranges.dedup();

    let trimmed_end = line.trim_end().len();
    let removed_length = ranges
        .iter()
        .map(|(start, end)| end.min(&trimmed_end).saturating_sub(*start))
        .sum::<usize>();
    let mut hasher = StableFnvHasher::new();
    trimmed_end.saturating_sub(removed_length).hash(&mut hasher);
    if trimmed_end <= FULL_HASH_BYTES {
        hash_range_excluding(line, 0, trimmed_end, &ranges, &mut hasher);
    } else {
        let head_end = floor_char_boundary(line, EDGE_SAMPLE_BYTES.min(trimmed_end));
        let tail_start = ceil_char_boundary(line, trimmed_end.saturating_sub(EDGE_SAMPLE_BYTES));
        hash_range_excluding(line, 0, head_end, &ranges, &mut hasher);
        hash_range_excluding(line, tail_start, trimmed_end, &ranges, &mut hasher);
    }
    hasher.finish()
}

fn normalize_record_for_fork_match(root: &mut Value) {
    let root_type = root
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let payload_type = root
        .get("payload")
        .and_then(|payload| payload.get("type"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let Some(root_object) = root.as_object_mut() else {
        return;
    };
    // The fork recorder rewrites the envelope timestamp while preserving the
    // semantic record. It may also regenerate response IDs and add newer
    // schema fields whose default value carries no historical information.
    root_object.remove("timestamp");
    let Some(payload) = root_object
        .get_mut("payload")
        .and_then(Value::as_object_mut)
    else {
        return;
    };

    if root_type == "response_item" {
        payload.remove("id");
        if payload.get("content") == Some(&Value::Null) {
            payload.remove("content");
        }
    }
    if root_type == "session_meta" {
        for (key, expected) in [
            ("history_mode", Value::String("legacy".to_string())),
            ("history_base", Value::Null),
            ("subagent_history_start_ordinal", Value::Null),
            ("parent_thread_id", Value::Null),
        ] {
            if payload.get(key) == Some(&expected) {
                payload.remove(key);
            }
        }
    }
    if root_type == "event_msg" && payload_type == "user_message" {
        if payload
            .get("local_audio")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            payload.remove("local_audio");
        }
    }
    if root_type == "event_msg" && payload_type == "token_count" {
        if let Some(info) = payload.get_mut("info").and_then(Value::as_object_mut) {
            for usage_key in ["total_token_usage", "last_token_usage"] {
                if let Some(usage) = info.get_mut(usage_key).and_then(Value::as_object_mut) {
                    if usage
                        .get("cache_write_input_tokens")
                        .and_then(Value::as_u64)
                        == Some(0)
                    {
                        usage.remove("cache_write_input_tokens");
                    }
                }
            }
        }
        if let Some(rate_limits) = payload
            .get_mut("rate_limits")
            .and_then(Value::as_object_mut)
        {
            if rate_limits.get("spend_control_reached") == Some(&Value::Null) {
                rate_limits.remove("spend_control_reached");
            }
        }
    }
}

fn session_id_from_rollout_filename(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let candidate = stem.get(stem.len().checked_sub(36)?..)?;
    let bytes = candidate.as_bytes();
    if bytes.len() == 36
        && [8, 13, 18, 23]
            .into_iter()
            .all(|index| bytes[index] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| [8, 13, 18, 23].contains(&index) || byte.is_ascii_hexdigit())
    {
        Some(candidate.to_ascii_lowercase())
    } else {
        None
    }
}

#[cfg(test)]
fn parse_token_event_line(line: &str) -> Option<ParsedTokenEvent> {
    parse_token_event_line_at(line, 0)
}

fn parse_token_event_line_at(line: &str, record_index: usize) -> Option<ParsedTokenEvent> {
    // Most rollout lines are prompts, tool output, or UI events. Avoid allocating a
    // full serde_json::Value for them before checking for the one event this view uses.
    if !line.contains("\"token_count\"") {
        return None;
    }
    let mut root = serde_json::from_str::<Value>(line).ok()?;
    let timestamp = parse_timestamp(root.get("timestamp")?.as_str()?)?;
    normalize_record_for_fork_match(&mut root);
    parse_token_event_root(&root, Some(timestamp), record_index)
}

fn parse_token_event_root(
    root: &Value,
    timestamp: Option<i64>,
    record_index: usize,
) -> Option<ParsedTokenEvent> {
    if root.get("type")?.as_str()? != "event_msg" {
        return None;
    }

    let payload = root.get("payload")?;
    if payload.get("type")?.as_str()? != "token_count" {
        return None;
    }

    let timestamp = timestamp?;
    let info = payload.get("info")?;
    let last = parse_local_token_event_usage(info);
    let total = info.get("total_token_usage").and_then(parse_token_totals);
    if last.is_none() && total.is_none() {
        return None;
    }

    Some(ParsedTokenEvent {
        record_index,
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

fn parse_local_token_event_usage(info: &Value) -> Option<CodexTokenTotals> {
    info.get("last_token_usage").and_then(parse_token_totals)
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

fn estimate_token_cost_usd(model: &str, event_timestamp: i64, usage: &CodexTokenTotals) -> f64 {
    let rate = pricing_rate_for_model_at(model, event_timestamp);
    let cached_input = usage.cached_input_tokens.min(usage.input_tokens);
    let uncached_input = usage.input_tokens.saturating_sub(cached_input);
    let cost = (uncached_input as f64 * rate.input_per_million
        + cached_input as f64 * rate.cached_input_per_million
        + usage.output_tokens as f64 * rate.output_per_million)
        / 1_000_000.0;
    round_cost(cost)
}

fn pricing_rate_for_model_at(model: &str, event_timestamp: i64) -> PricingRate {
    let normalized = model.to_ascii_lowercase();
    // Standard short-context rates, not subscription billing or fast-tier rates.
    // https://developers.openai.com/api/docs/models/gpt-6-astra
    if normalized == "gpt-6-astra" || normalized.starts_with("gpt-6-astra-") {
        return PricingRate {
            input_per_million: 10.0,
            cached_input_per_million: 1.0,
            output_per_million: 50.0,
        };
    }
    if normalized == "gpt-5.6"
        || normalized == "gpt5.6"
        || normalized == "gpt-5-6"
        || normalized.starts_with("gpt-5.6-sol")
        || normalized.starts_with("gpt5.6-sol")
        || normalized.starts_with("gpt-5-6-sol")
    {
        // https://developers.openai.com/api/docs/models/gpt-5.6-sol
        if event_timestamp >= GPT_5_6_SOL_PRICE_REDUCTION_EFFECTIVE_AT {
            return PricingRate {
                input_per_million: 4.0,
                cached_input_per_million: 0.4,
                output_per_million: 20.0,
            };
        }
        return PricingRate {
            input_per_million: 5.0,
            cached_input_per_million: 0.5,
            output_per_million: 30.0,
        };
    }
    if normalized.starts_with("gpt-5.6-terra")
        || normalized.starts_with("gpt5.6-terra")
        || normalized.starts_with("gpt-5-6-terra")
    {
        if event_timestamp >= GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT {
            return PricingRate {
                input_per_million: 2.0,
                cached_input_per_million: 0.2,
                output_per_million: 12.0,
            };
        }
        return PricingRate {
            input_per_million: 2.5,
            cached_input_per_million: 0.25,
            output_per_million: 15.0,
        };
    }
    if normalized.starts_with("gpt-5.6-luna")
        || normalized.starts_with("gpt5.6-luna")
        || normalized.starts_with("gpt-5-6-luna")
    {
        if event_timestamp >= GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT {
            return PricingRate {
                input_per_million: 0.2,
                cached_input_per_million: 0.02,
                output_per_million: 1.2,
            };
        }
        return PricingRate {
            input_per_million: 1.0,
            cached_input_per_million: 0.1,
            output_per_million: 6.0,
        };
    }
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
    let utc_date_time = OffsetDateTime::from_unix_timestamp(timestamp).ok()?;
    let local_offset = UtcOffset::local_offset_at(utc_date_time).unwrap_or(UtcOffset::UTC);
    Some(heatmap_bucket_key_with_offset(utc_date_time, local_offset))
}

fn previous_complete_local_date_range(now: i64) -> Option<(Date, Date)> {
    let today = local_date_at(now)?;
    Some(previous_complete_date_range(today))
}

fn previous_complete_date_range(today: Date) -> (Date, Date) {
    (
        today - time::Duration::days(7),
        today - time::Duration::days(1),
    )
}

fn local_date_at(timestamp: i64) -> Option<Date> {
    let utc_date_time = OffsetDateTime::from_unix_timestamp(timestamp).ok()?;
    let local_offset = UtcOffset::local_offset_at(utc_date_time).unwrap_or(UtcOffset::UTC);
    Some(utc_date_time.to_offset(local_offset).date())
}

fn heatmap_bucket_key_with_offset(
    utc_date_time: OffsetDateTime,
    local_offset: UtcOffset,
) -> (u8, u8) {
    let date_time = utc_date_time.to_offset(local_offset);
    (
        date_time.weekday().number_days_from_sunday(),
        date_time.hour(),
    )
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
        "selected_cost_source",
        "cost_source_updated_at",
        "cost_source_error",
        "local_total_cost_usd",
        "local_last_7d_cost_usd",
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
        &snapshot.cost_source,
        &snapshot
            .cost_source_updated_at
            .map(|value| value.to_string())
            .unwrap_or_default(),
        snapshot.cost_source_error.as_deref().unwrap_or_default(),
        &snapshot.local_total_cost_usd.to_string(),
        &snapshot.local_last_7d_cost_usd.to_string(),
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
            COST_SOURCE_LOCAL,
            "",
            "",
            "",
            "",
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
            COST_SOURCE_LOCAL,
            "",
            "",
            "",
            "",
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
            COST_SOURCE_LOCAL,
            "",
            "",
            "",
            "",
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
            COST_SOURCE_LOCAL,
            "",
            "",
            "",
            "",
            "",
            "",
            "",
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

fn token_totals_delta(
    current: &CodexTokenTotals,
    previous: &CodexTokenTotals,
) -> Option<CodexTokenTotals> {
    Some(CodexTokenTotals {
        input_tokens: current.input_tokens.checked_sub(previous.input_tokens)?,
        cached_input_tokens: current
            .cached_input_tokens
            .checked_sub(previous.cached_input_tokens)?,
        output_tokens: current.output_tokens.checked_sub(previous.output_tokens)?,
        reasoning_output_tokens: current
            .reasoning_output_tokens
            .checked_sub(previous.reasoning_output_tokens)?,
        total_tokens: current.total_tokens.checked_sub(previous.total_tokens)?,
    })
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

        if let Some(last) = event.last.as_ref() {
            self.summed_last_usage.add(last);
        }
        if let Some(total) = event.total.as_ref() {
            self.latest_cumulative_total = total.clone();
        }
    }

    fn into_latest_session(self) -> Option<CodexTokenSessionUsage> {
        let updated_at = self.updated_at?;
        let total = if self.summed_last_usage.is_empty() {
            self.latest_cumulative_total
        } else {
            self.summed_last_usage
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
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::UNIX_EPOCH;

    static TEMP_DIR_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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

    fn session_meta_line(
        timestamp: &str,
        session_id: &str,
        parent_session_id: Option<&str>,
        cwd: &str,
        model: &str,
    ) -> String {
        let mut payload = serde_json::json!({
            "id": session_id,
            "cwd": cwd,
            "model": model,
        });
        if let Some(parent_session_id) = parent_session_id {
            payload["forked_from_id"] = Value::String(parent_session_id.to_string());
        }
        serde_json::json!({
            "timestamp": timestamp,
            "type": "session_meta",
            "payload": payload,
        })
        .to_string()
    }

    #[test]
    fn uses_july_gpt_5_6_variant_pricing() {
        for (model, input, cached, output) in [
            ("gpt-5.6-sol", 5.0, 0.5, 30.0),
            ("gpt-5.6-terra", 2.0, 0.2, 12.0),
            ("gpt-5.6-luna", 0.2, 0.02, 1.2),
            ("gpt-5.6", 5.0, 0.5, 30.0),
            ("gpt5.6-terra", 2.0, 0.2, 12.0),
            ("gpt-5-6-luna", 0.2, 0.02, 1.2),
            ("gpt-5.6-sol-2026-07-01", 5.0, 0.5, 30.0),
        ] {
            let rate =
                pricing_rate_for_model_at(model, GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT);
            assert_eq!(rate.input_per_million, input, "input price for {model}");
            assert_eq!(
                rate.cached_input_per_million, cached,
                "cached input price for {model}"
            );
            assert_eq!(rate.output_per_million, output, "output price for {model}");
        }
    }

    #[test]
    fn prices_sol_cutover_and_astra_without_fallback_rates() {
        let usage = CodexTokenTotals {
            input_tokens: 1_000_000,
            cached_input_tokens: 500_000,
            output_tokens: 1_000_000,
            ..Default::default()
        };
        let cutoff = GPT_5_6_SOL_PRICE_REDUCTION_EFFECTIVE_AT;
        for model in [
            "gpt-5.6-sol",
            "gpt-5.6",
            "gpt5.6",
            "gpt-5-6",
            "gpt-5.6-sol-2026-07-01",
        ] {
            assert_eq!(estimate_token_cost_usd(model, cutoff - 1, &usage), 32.75);
            assert_eq!(estimate_token_cost_usd(model, cutoff, &usage), 22.2);
        }
        for model in ["gpt-6-astra", "GPT-6-ASTRA", "gpt-6-astra-2026-09-03"] {
            assert_eq!(estimate_token_cost_usd(model, cutoff, &usage), 55.5);
        }
    }

    #[test]
    fn preserves_pre_reduction_gpt_5_6_terra_and_luna_pricing() {
        for (model, input, cached, output) in [
            ("gpt-5.6-terra", 2.5, 0.25, 15.0),
            ("gpt5.6-terra", 2.5, 0.25, 15.0),
            ("gpt-5.6-luna", 1.0, 0.1, 6.0),
            ("gpt-5-6-luna", 1.0, 0.1, 6.0),
        ] {
            let rate = pricing_rate_for_model_at(
                model,
                GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT - 1,
            );
            assert_eq!(rate.input_per_million, input, "input price for {model}");
            assert_eq!(
                rate.cached_input_per_million, cached,
                "cached input price for {model}"
            );
            assert_eq!(rate.output_per_million, output, "output price for {model}");
        }
    }

    #[test]
    fn prices_mixed_gpt_5_6_history_by_event_timestamp() {
        let usage = CodexTokenTotals {
            input_tokens: 1_000_000,
            cached_input_tokens: 0,
            output_tokens: 1_000_000,
            reasoning_output_tokens: 0,
            total_tokens: 2_000_000,
        };
        let before = GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT - 1;
        let after = GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT;

        assert_eq!(
            estimate_token_cost_usd("gpt-5.6-terra", before, &usage),
            17.5
        );
        assert_eq!(
            estimate_token_cost_usd("gpt-5.6-terra", after, &usage),
            14.0
        );
        assert_eq!(estimate_token_cost_usd("gpt-5.6-luna", before, &usage), 7.0);
        assert_eq!(estimate_token_cost_usd("gpt-5.6-luna", after, &usage), 1.4);
    }

    #[test]
    fn cost_analytics_applies_historical_price_to_each_event() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions").join("2026").join("07").join("30");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        fs::write(
            sessions.join("rollout-price-cutover.jsonl"),
            [
                session_meta_line(
                    "2026-07-29T23:59:58Z",
                    "price-cutover-session",
                    None,
                    "/tmp/price-cutover-project",
                    "gpt-5.6-terra",
                ),
                analytics_token_line("2026-07-29T23:59:59Z", 1_000_000, 0, 1_000_000),
                analytics_token_line("2026-07-30T00:00:00Z", 1_000_000, 0, 1_000_000),
            ]
            .join("\n"),
        )
        .expect("write price cutover log");

        let now = parse_timestamp("2026-08-01T00:00:00Z").expect("parse now");
        let snapshot = scan_codex_cost_analytics_roots_with_progress(
            &[root.join("sessions")],
            now,
            None,
            |_| {},
        );

        assert_eq!(snapshot.event_count, 2);
        assert!((snapshot.total_cost_usd - 31.5).abs() < 0.000001);
        assert!((snapshot.sessions[0].cost_usd - 31.5).abs() < 0.000001);

        let _ = fs::remove_dir_all(root);
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
                session_meta_line(
                    "2026-04-27T05:59:00Z",
                    "window-session",
                    None,
                    "/tmp/window-project",
                    "gpt-5.5",
                ),
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
    fn ignores_repeated_last_usage_when_cumulative_snapshot_is_unchanged() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions").join("2026").join("07").join("14");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        fs::write(
            sessions.join("rollout-repeated-total.jsonl"),
            [
                session_meta_line(
                    "2026-07-14T09:59:00Z",
                    "repeated-total-session",
                    None,
                    "/tmp/repeated-project",
                    "gpt-5.5",
                ),
                event_line("2026-07-14T10:00:00Z", 100, 100),
                event_line("2026-07-14T10:01:00Z", 100, 25),
            ]
            .join("\n"),
        )
        .expect("write repeated cumulative snapshots");

        let now = parse_timestamp("2026-07-15T00:00:00Z").expect("parse now");
        let token_snapshot = scan_codex_token_usage_roots(&[root.join("sessions")], now);
        assert_eq!(token_snapshot.event_count, 1);
        assert_eq!(token_snapshot.last_7d.total_tokens, 100);
        assert_eq!(
            token_snapshot
                .latest_session
                .expect("latest session")
                .total
                .total_tokens,
            100
        );

        let analytics_snapshot = scan_codex_cost_analytics_roots_with_progress(
            &[root.join("sessions")],
            now,
            None,
            |_| {},
        );
        assert_eq!(analytics_snapshot.event_count, 1);
        assert_eq!(analytics_snapshot.total.total_tokens, 100);
        assert_eq!(analytics_snapshot.last_7d.total_tokens, 100);
        assert_eq!(
            analytics_snapshot
                .heatmap
                .iter()
                .map(|bucket| bucket.tokens)
                .sum::<u64>(),
            100
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn diagnoses_counter_decrease_and_resumes_from_the_new_epoch() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        fs::write(
            sessions.join("rollout-counter-reset.jsonl"),
            [
                session_meta_line(
                    "2026-07-14T09:59:00Z",
                    "counter-reset-session",
                    None,
                    "/tmp/reset-project",
                    "gpt-5.5",
                ),
                event_line("2026-07-14T10:00:00Z", 100, 100),
                event_line("2026-07-14T10:01:00Z", 80, 20),
                event_line("2026-07-14T10:02:00Z", 100, 20),
            ]
            .join("\n"),
        )
        .expect("write counter reset log");

        let now = parse_timestamp("2026-07-15T00:00:00Z").expect("parse now");
        let token_snapshot = scan_codex_token_usage_roots(&[sessions.clone()], now);
        assert_eq!(token_snapshot.unresolved_usage_event_count, 1);
        assert_eq!(token_snapshot.event_count, 2);
        assert_eq!(token_snapshot.last_7d.total_tokens, 120);

        let analytics_snapshot =
            scan_codex_cost_analytics_roots_with_progress(&[sessions], now, None, |_| {});
        assert_eq!(analytics_snapshot.unresolved_usage_event_count, 1);
        assert_eq!(analytics_snapshot.event_count, 2);
        assert_eq!(analytics_snapshot.total.total_tokens, 120);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reuses_token_usage_cache_and_reparses_changed_files() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions").join("2026").join("04").join("28");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        let path = sessions.join("rollout-cache-test.jsonl");
        let cache_header = session_meta_line(
            "2026-04-28T05:59:00Z",
            "cache-session",
            None,
            "/tmp/cache-project",
            "gpt-5.5",
        );
        fs::write(
            &path,
            [
                cache_header.clone(),
                event_line("2026-04-28T06:00:00Z", 100, 100),
            ]
            .join("\n"),
        )
        .expect("write initial log");

        let roots = [root.join("sessions"), root.join("archived_sessions")];
        let mut cache = TokenUsageCache::default();
        let initial = scan_codex_token_usage_roots_with_cache(&roots, 1_777_361_000, &mut cache);
        let unchanged = scan_codex_token_usage_roots_with_cache(&roots, 1_777_361_001, &mut cache);

        assert_eq!(cache.files.len(), 1);
        assert_eq!(initial.event_count, 1);
        assert_eq!(unchanged.event_count, 1);
        assert_eq!(unchanged.last_24h.total_tokens, 100);

        fs::write(
            &path,
            [
                cache_header,
                event_line("2026-04-28T06:00:00Z", 100, 100),
                event_line("2026-04-28T06:01:00Z", 300, 200),
            ]
            .join("\n"),
        )
        .expect("update log");

        let changed = scan_codex_token_usage_roots_with_cache(&roots, 1_777_361_002, &mut cache);
        assert_eq!(changed.event_count, 2);
        assert_eq!(changed.last_24h.total_tokens, 300);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn appends_only_new_bytes_for_growing_token_logs() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp dir");
        let path = root.join("rollout-growing.jsonl");
        fs::write(
            &path,
            format!("{}\n", event_line("2026-04-28T06:00:00Z", 100, 100)),
        )
        .expect("write initial log");

        let mut cached = parse_token_session_file(&path).expect("parse initial log");
        let initial_length = cached.fingerprint.length;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open growing log");
        writeln!(file, "{}", event_line("2026-04-28T06:01:00Z", 300, 200))
            .expect("append token event");
        drop(file);

        let metadata = fs::metadata(&path).expect("read updated metadata");
        let fingerprint = LogFileFingerprint {
            length: metadata.len(),
            modified_at: metadata.modified().ok(),
        };
        assert!(append_token_session_file(&path, &fingerprint, &mut cached)
            .expect("append growing log"));
        assert!(cached.fingerprint.length > initial_length);
        assert_eq!(cached.fingerprint.length, metadata.len());
        assert_eq!(cached.parsed.events.len(), 2);
        assert_eq!(
            cached
                .parsed
                .latest_session
                .expect("latest session")
                .total
                .total_tokens,
            300
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn excludes_replayed_parent_history_from_forked_sessions() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions").join("2026").join("07").join("13");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        fs::write(
            sessions.join("rollout-parent.jsonl"),
            [
                session_meta_line(
                    "2026-07-13T10:00:00Z",
                    "parent-session",
                    None,
                    "/tmp/fork-project",
                    "gpt-5.5",
                ),
                event_line("2026-07-13T10:01:00Z", 100, 100),
                event_line("2026-07-13T10:02:00Z", 300, 200),
            ]
            .join("\n"),
        )
        .expect("write parent log");
        fs::write(
            sessions.join("rollout-child.jsonl"),
            [
                session_meta_line(
                    "2026-07-14T14:00:00Z",
                    "child-session",
                    Some("parent-session"),
                    "/tmp/fork-project",
                    "gpt-5.5",
                ),
                event_line("2026-07-14T14:01:00Z", 100, 100),
                event_line("2026-07-14T14:02:00Z", 300, 200),
                event_line("2026-07-14T14:03:00Z", 350, 50),
            ]
            .join("\n"),
        )
        .expect("write child log");

        let now = parse_timestamp("2026-07-15T00:00:00Z").expect("parse now");
        let token_snapshot = scan_codex_token_usage_roots(&[root.join("sessions")], now);
        assert_eq!(token_snapshot.event_count, 3);
        assert_eq!(token_snapshot.last_7d.total_tokens, 350);
        assert_eq!(
            token_snapshot
                .latest_session
                .expect("latest fork session")
                .total
                .total_tokens,
            50
        );

        let analytics_snapshot = scan_codex_cost_analytics_roots_with_progress(
            &[root.join("sessions")],
            now,
            None,
            |_| {},
        );
        assert_eq!(analytics_snapshot.event_count, 3);
        assert_eq!(analytics_snapshot.total.total_tokens, 350);
        assert_eq!(analytics_snapshot.last_7d.total_tokens, 350);
        assert_eq!(
            analytics_snapshot
                .heatmap
                .iter()
                .map(|bucket| bucket.tokens)
                .sum::<u64>(),
            350
        );
        let child = analytics_snapshot
            .sessions
            .iter()
            .find(|session| session.session_id == "child-session")
            .expect("child session");
        assert_eq!(child.event_count, 1);
        assert_eq!(child.total.total_tokens, 50);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_child_identity_when_replayed_history_contains_parent_session_meta() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions").join("2026").join("07").join("25");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        let parent_meta = session_meta_line(
            "2026-07-24T10:00:00Z",
            "parent-session",
            None,
            "/tmp/fork-project",
            "gpt-5.5",
        );
        fs::write(
            sessions.join("rollout-parent.jsonl"),
            [
                parent_meta.clone(),
                event_line("2026-07-24T10:01:00Z", 100, 100),
                event_line("2026-07-24T10:02:00Z", 300, 200),
            ]
            .join("\n"),
        )
        .expect("write parent log");
        fs::write(
            sessions.join("rollout-child.jsonl"),
            [
                session_meta_line(
                    "2026-07-25T06:00:00Z",
                    "child-session",
                    Some("parent-session"),
                    "/tmp/fork-project",
                    "gpt-5.5",
                ),
                parent_meta,
                event_line("2026-07-25T06:01:00Z", 100, 100),
                event_line("2026-07-25T06:02:00Z", 300, 200),
                event_line("2026-07-25T06:03:00Z", 350, 50),
            ]
            .join("\n"),
        )
        .expect("write nested fork log");

        let now = parse_timestamp("2026-07-25T12:00:00Z").expect("parse now");
        let token_snapshot = scan_codex_token_usage_roots(&[root.join("sessions")], now);
        assert_eq!(token_snapshot.event_count, 3);
        assert_eq!(token_snapshot.last_7d.total_tokens, 350);
        assert_eq!(
            token_snapshot
                .latest_session
                .expect("latest child session")
                .total
                .total_tokens,
            50
        );

        let analytics_snapshot = scan_codex_cost_analytics_roots_with_progress(
            &[root.join("sessions")],
            now,
            None,
            |_| {},
        );
        assert_eq!(analytics_snapshot.event_count, 3);
        assert_eq!(analytics_snapshot.total.total_tokens, 350);
        let child = analytics_snapshot
            .sessions
            .iter()
            .find(|session| session.session_id == "child-session")
            .expect("child session identity must come from the file header");
        assert_eq!(child.parent_session_id.as_deref(), Some("parent-session"));
        assert_eq!(child.event_count, 1);
        assert_eq!(child.total.total_tokens, 50);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_nested_forks_against_each_direct_parent_raw_record_stream() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions").join("2026").join("07").join("25");
        fs::create_dir_all(&sessions).expect("create sessions dir");

        let a_records = vec![
            session_meta_line(
                "2026-07-23T10:00:00Z",
                "session-a",
                None,
                "/tmp/fork-project",
                "gpt-5.5",
            ),
            event_line("2026-07-23T10:01:00Z", 100, 100),
            event_line("2026-07-23T10:02:00Z", 300, 200),
        ];
        let mut b_records = vec![session_meta_line(
            "2026-07-24T10:00:00Z",
            "session-b",
            Some("session-a"),
            "/tmp/fork-project",
            "gpt-5.5",
        )];
        b_records.extend(a_records.clone());
        b_records.push(event_line("2026-07-24T10:03:00Z", 350, 50));
        let mut c_records = vec![session_meta_line(
            "2026-07-25T06:00:00Z",
            "session-c",
            Some("session-b"),
            "/tmp/fork-project",
            "gpt-5.5",
        )];
        c_records.extend(b_records.clone());
        c_records.push(event_line("2026-07-25T06:04:00Z", 375, 25));

        fs::write(sessions.join("rollout-a.jsonl"), a_records.join("\n")).expect("write session A");
        fs::write(sessions.join("rollout-b.jsonl"), b_records.join("\n")).expect("write session B");
        fs::write(sessions.join("rollout-c.jsonl"), c_records.join("\n")).expect("write session C");

        let now = parse_timestamp("2026-07-25T12:00:00Z").expect("parse now");
        let token_snapshot = scan_codex_token_usage_roots(&[root.join("sessions")], now);
        assert_eq!(token_snapshot.unresolved_fork_count, 0);
        assert_eq!(token_snapshot.event_count, 4);
        assert_eq!(token_snapshot.last_7d.total_tokens, 375);
        assert_eq!(
            token_snapshot
                .latest_session
                .expect("latest nested child")
                .total
                .total_tokens,
            25
        );

        let analytics_snapshot = scan_codex_cost_analytics_roots_with_progress(
            &[root.join("sessions")],
            now,
            None,
            |_| {},
        );
        assert_eq!(analytics_snapshot.unresolved_fork_count, 0);
        assert_eq!(analytics_snapshot.event_count, 4);
        assert_eq!(analytics_snapshot.total.total_tokens, 375);
        assert_eq!(
            analytics_snapshot
                .sessions
                .iter()
                .find(|session| session.session_id == "session-c")
                .expect("session C")
                .total
                .total_tokens,
            25
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fork_record_matching_resynchronizes_after_insertions_and_omissions() {
        let parent = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let child_with_insertion = vec![1, 2, 99, 3, 4, 5, 6, 7, 8, 200, 201];
        let child_with_omission = vec![1, 2, 4, 5, 6, 7, 8, 200, 201];

        assert_eq!(
            matching_record_prefix(&child_with_insertion, &parent, FORK_MATCH_RESYNC_WINDOW),
            9
        );
        assert_eq!(
            matching_record_prefix(&child_with_omission, &parent, FORK_MATCH_RESYNC_WINDOW),
            7
        );
    }

    #[test]
    fn fork_record_matching_stops_without_a_confirming_branch_anchor() {
        let parent = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let child = vec![1, 2, 3, 90, 5, 91, 92, 93];

        assert_eq!(
            matching_record_prefix(&child, &parent, FORK_MATCH_RESYNC_WINDOW),
            3
        );
    }

    #[test]
    fn fork_record_hash_ignores_replayed_response_defaults_and_ids() {
        let parent = r#"{"timestamp":"2026-07-09T20:58:47Z","type":"response_item","payload":{"type":"reasoning","id":"parent-id","encrypted_content":"cipher","summary":[]}}"#;
        let child = r#"{"timestamp":"2026-07-24T22:04:45Z","type":"response_item","payload":{"type":"reasoning","id":"child-id","content":null,"encrypted_content":"cipher","summary":[]}}"#;

        assert_eq!(fast_record_hash(parent), fast_record_hash(child));
    }

    #[test]
    fn keeps_missing_parent_usage_separate_from_confirmed_totals() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        fs::write(
            sessions.join("rollout-orphan-child.jsonl"),
            [
                session_meta_line(
                    "2026-07-25T06:00:00Z",
                    "orphan-child",
                    Some("missing-parent"),
                    "/tmp/orphan-project",
                    "gpt-5.5",
                ),
                event_line("2026-07-25T06:01:00Z", 100, 100),
            ]
            .join("\n"),
        )
        .expect("write orphan fork");

        let now = parse_timestamp("2026-07-25T12:00:00Z").expect("parse now");
        let token_snapshot = scan_codex_token_usage_roots(&[sessions.clone()], now);
        assert_eq!(token_snapshot.unresolved_fork_count, 1);
        assert_eq!(token_snapshot.event_count, 0);
        assert_eq!(token_snapshot.last_7d.total_tokens, 0);

        let analytics_snapshot =
            scan_codex_cost_analytics_roots_with_progress(&[sessions], now, None, |_| {});
        assert_eq!(analytics_snapshot.unresolved_fork_count, 1);
        assert_eq!(analytics_snapshot.event_count, 0);
        assert_eq!(analytics_snapshot.total.total_tokens, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn does_not_use_agent_parent_as_a_history_inheritance_edge() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        let agent_meta = serde_json::json!({
            "timestamp": "2026-07-25T06:00:00Z",
            "type": "session_meta",
            "payload": {
                "id": "agent-child",
                "parent_thread_id": "spawning-thread",
                "cwd": "/tmp/agent-project",
                "model": "gpt-5.5"
            }
        })
        .to_string();
        fs::write(
            sessions.join("rollout-agent-child.jsonl"),
            [agent_meta, event_line("2026-07-25T06:01:00Z", 100, 100)].join("\n"),
        )
        .expect("write agent child");

        let now = parse_timestamp("2026-07-25T12:00:00Z").expect("parse now");
        let token_snapshot = scan_codex_token_usage_roots(&[sessions.clone()], now);
        assert_eq!(token_snapshot.unresolved_fork_count, 0);
        assert_eq!(token_snapshot.last_7d.total_tokens, 100);

        let analytics_snapshot =
            scan_codex_cost_analytics_roots_with_progress(&[sessions], now, None, |_| {});
        assert_eq!(analytics_snapshot.unresolved_fork_count, 0);
        assert_eq!(analytics_snapshot.total.total_tokens, 100);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn starts_owned_usage_at_zero_when_inherited_context_has_no_token_snapshot() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        let parent_meta = session_meta_line(
            "2026-07-24T10:00:00Z",
            "context-parent",
            None,
            "/tmp/context-project",
            "gpt-5.5",
        );
        let copied_context = serde_json::json!({
            "timestamp": "2026-07-24T10:00:01Z",
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "copied context"}]
            }
        })
        .to_string();
        fs::write(
            sessions.join("rollout-context-parent.jsonl"),
            [parent_meta.clone(), copied_context.clone()].join("\n"),
        )
        .expect("write context parent");

        let mut child_meta = serde_json::from_str::<Value>(&session_meta_line(
            "2026-07-25T06:00:00Z",
            "context-child",
            Some("context-parent"),
            "/tmp/context-project",
            "gpt-5.5",
        ))
        .expect("parse child metadata");
        child_meta["payload"]["parent_thread_id"] = Value::String("context-parent".to_string());
        fs::write(
            sessions.join("rollout-context-child.jsonl"),
            [
                child_meta.to_string(),
                parent_meta,
                copied_context,
                event_line("2026-07-25T06:01:00Z", 100, 100),
            ]
            .join("\n"),
        )
        .expect("write context child");

        let now = parse_timestamp("2026-07-25T12:00:00Z").expect("parse now");
        let token_snapshot = scan_codex_token_usage_roots(&[sessions.clone()], now);
        assert_eq!(token_snapshot.unresolved_usage_event_count, 0);
        assert_eq!(token_snapshot.last_7d.total_tokens, 100);

        let analytics_snapshot =
            scan_codex_cost_analytics_roots_with_progress(&[sessions], now, None, |_| {});
        assert_eq!(analytics_snapshot.unresolved_usage_event_count, 0);
        assert_eq!(analytics_snapshot.total.total_tokens, 100);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_cached_child_when_its_parent_file_appears_later() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        let parent_records = vec![
            session_meta_line(
                "2026-07-24T10:00:00Z",
                "late-parent",
                None,
                "/tmp/late-parent-project",
                "gpt-5.5",
            ),
            event_line("2026-07-24T10:01:00Z", 100, 100),
        ];
        let mut child_records = vec![session_meta_line(
            "2026-07-25T06:00:00Z",
            "cached-child",
            Some("late-parent"),
            "/tmp/late-parent-project",
            "gpt-5.5",
        )];
        child_records.extend(parent_records.clone());
        child_records.push(event_line("2026-07-25T06:01:00Z", 150, 50));
        fs::write(
            sessions.join("rollout-cached-child.jsonl"),
            child_records.join("\n"),
        )
        .expect("write child first");

        let now = parse_timestamp("2026-07-25T12:00:00Z").expect("parse now");
        let mut token_cache = TokenUsageCache::default();
        let first_token = scan_codex_token_usage_roots_with_cache(
            std::slice::from_ref(&sessions),
            now,
            &mut token_cache,
        );
        assert_eq!(first_token.unresolved_fork_count, 1);

        let mut analytics_cache = CostAnalyticsCache::default();
        let first_analytics = scan_codex_cost_analytics_roots_with_cache(
            std::slice::from_ref(&sessions),
            now,
            None,
            &mut analytics_cache,
            |_| {},
        );
        assert_eq!(first_analytics.unresolved_fork_count, 1);

        fs::write(
            sessions.join("rollout-late-parent.jsonl"),
            parent_records.join("\n"),
        )
        .expect("write late parent");

        let second_token = scan_codex_token_usage_roots_with_cache(
            std::slice::from_ref(&sessions),
            now + 1,
            &mut token_cache,
        );
        assert_eq!(second_token.unresolved_fork_count, 0);
        assert_eq!(second_token.last_7d.total_tokens, 150);

        let second_analytics = scan_codex_cost_analytics_roots_with_cache(
            &[sessions],
            now + 1,
            None,
            &mut analytics_cache,
            |_| {},
        );
        assert_eq!(second_analytics.unresolved_fork_count, 0);
        assert_eq!(second_analytics.total.total_tokens, 150);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn converts_heatmap_buckets_to_the_event_local_time() {
        let utc_date_time =
            OffsetDateTime::parse("2026-07-13T14:00:00Z", &Rfc3339).expect("parse UTC timestamp");
        let shanghai_offset = UtcOffset::from_hms(8, 0, 0).expect("create UTC+8 offset");

        assert_eq!(
            heatmap_bucket_key_with_offset(utc_date_time, shanghai_offset),
            (1, 22)
        );
    }

    #[test]
    fn previous_complete_date_range_excludes_today() {
        let today =
            Date::from_calendar_date(2026, time::Month::July, 23).expect("create current date");
        let (start, end) = previous_complete_date_range(today);

        assert_eq!(
            start,
            Date::from_calendar_date(2026, time::Month::July, 16).expect("create start date")
        );
        assert_eq!(
            end,
            Date::from_calendar_date(2026, time::Month::July, 22).expect("create end date")
        );
    }

    #[test]
    fn cost_window_uses_complete_days_while_heatmap_keeps_today() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions");
        fs::create_dir_all(&sessions).expect("create sessions dir");

        let now_utc = OffsetDateTime::now_utc();
        let local_offset = UtcOffset::local_offset_at(now_utc).unwrap_or(UtcOffset::UTC);
        let today = now_utc.to_offset(local_offset).date();
        let now = today
            .with_hms(12, 0, 0)
            .expect("create current time")
            .assume_offset(local_offset);
        let first_complete_day = (today - time::Duration::days(7))
            .with_hms(12, 0, 0)
            .expect("create complete-day time")
            .assume_offset(local_offset);
        let current_day = today
            .with_hms(11, 0, 0)
            .expect("create current-day time")
            .assume_offset(local_offset);
        let complete_day_timestamp = first_complete_day
            .format(&Rfc3339)
            .expect("format complete-day timestamp");
        let current_day_timestamp = current_day
            .format(&Rfc3339)
            .expect("format current-day timestamp");

        fs::write(
            sessions.join("rollout-natural-day-window.jsonl"),
            [
                session_meta_line(
                    &complete_day_timestamp,
                    "natural-day-session",
                    None,
                    "/tmp/natural-day-project",
                    "gpt-5.5",
                ),
                analytics_token_line(&complete_day_timestamp, 100, 0, 0),
                analytics_token_line(&current_day_timestamp, 200, 0, 0),
            ]
            .join("\n"),
        )
        .expect("write analytics log");

        let snapshot = scan_codex_cost_analytics_roots_with_progress(
            &[sessions],
            now.unix_timestamp(),
            None,
            |_| {},
        );

        assert_eq!(snapshot.total.total_tokens, 300);
        assert_eq!(snapshot.last_7d.total_tokens, 100);
        assert_eq!(snapshot.budget_period_cost_usd, snapshot.total_cost_usd);
        assert!(snapshot.budget_period_cost_usd > snapshot.last_7d_cost_usd);
        assert_eq!(
            snapshot
                .heatmap
                .iter()
                .map(|bucket| bucket.tokens)
                .sum::<u64>(),
            300
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
        let mut stale_cache = serde_json::from_str::<Value>(&cache).expect("parse cache json");
        for version in [8, 9] {
            stale_cache["version"] = Value::from(version);
            assert!(
                parse_codex_cost_analytics_cache(&stale_cache.to_string(), Some(1.0))
                    .expect("stale cache parse")
                    .is_none()
            );
        }
        let cached = parse_codex_cost_analytics_cache(&cache, Some(1.0))
            .expect("cache parse")
            .expect("cache snapshot");
        assert_eq!(cached.event_count, snapshot.event_count);
        assert_eq!(cached.weekly_budget_usd, Some(1.0));
        assert_eq!(cached.weekly_budget_alert, "ok");

        assert!(
            (snapshot.total_cost_usd
                - snapshot
                    .projects
                    .iter()
                    .map(|project| project.cost_usd)
                    .sum::<f64>())
            .abs()
                < 0.000001
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reuses_appends_reparses_and_evicts_cost_analytics_cache() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions").join("2026").join("07").join("21");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        let path = sessions.join("rollout-cost-cache.jsonl");
        let initial_lines = [
            session_meta_line(
                "2026-07-21T01:00:00Z",
                "cost-cache-session",
                None,
                "/tmp/cost-cache-project",
                "gpt-5.6-sol",
            ),
            serde_json::json!({
                "timestamp": "2026-07-21T01:00:01Z",
                "type": "event_msg",
                "payload": {
                    "type": "user_message",
                    "message": "Initial prompt"
                }
            })
            .to_string(),
            analytics_token_line("2026-07-21T01:00:02Z", 1_000, 100, 2_000),
        ];
        fs::write(&path, format!("{}\n", initial_lines.join("\n")))
            .expect("write initial analytics log");

        let roots = [root.join("sessions"), root.join("archived_sessions")];
        let now = parse_timestamp("2026-07-21T02:00:00Z").expect("parse now");
        let mut cache = CostAnalyticsCache::default();
        let initial =
            scan_codex_cost_analytics_roots_with_cache(&roots, now, None, &mut cache, |_| {});
        assert_eq!(initial.event_count, 1);
        assert_eq!(initial.total.total_tokens, 3_000);
        assert_eq!(cache.last_scan.reparsed_files, 1);

        let unchanged =
            scan_codex_cost_analytics_roots_with_cache(&roots, now, None, &mut cache, |_| {});
        assert_eq!(unchanged, initial);
        assert_eq!(cache.last_scan.reused_files, 1);
        assert_eq!(cache.last_scan.appended_files, 0);
        assert_eq!(cache.last_scan.reparsed_files, 0);

        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open growing analytics log");
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "timestamp": "2026-07-21T01:01:00Z",
                "type": "event_msg",
                "payload": {
                    "type": "user_message",
                    "message": "Appended prompt"
                }
            })
        )
        .expect("append prompt");
        writeln!(
            file,
            "{}",
            analytics_token_line("2026-07-21T01:01:01Z", 2_000, 200, 4_000)
        )
        .expect("append token event");
        drop(file);

        let appended =
            scan_codex_cost_analytics_roots_with_cache(&roots, now, None, &mut cache, |_| {});
        assert_eq!(appended.event_count, 2);
        assert_eq!(appended.total.total_tokens, 9_000);
        assert_eq!(cache.last_scan.appended_files, 1);
        assert_eq!(cache.last_scan.reparsed_files, 0);
        assert_eq!(
            cache
                .files
                .get(&path)
                .expect("cached appended analytics log")
                .fingerprint
                .length,
            fs::metadata(&path).expect("read appended metadata").len()
        );
        let appended_unchanged =
            scan_codex_cost_analytics_roots_with_cache(&roots, now, None, &mut cache, |_| {});
        assert_eq!(appended_unchanged, appended);
        assert_eq!(cache.last_scan.reused_files, 1);
        assert_eq!(cache.last_scan.appended_files, 0);
        assert_eq!(cache.last_scan.reparsed_files, 0);

        let mut fresh_cache = CostAnalyticsCache::default();
        let freshly_parsed =
            scan_codex_cost_analytics_roots_with_cache(&roots, now, None, &mut fresh_cache, |_| {});
        assert_eq!(appended, freshly_parsed);

        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                session_meta_line(
                    "2026-07-21T01:02:00Z",
                    "rewritten-session",
                    None,
                    "/tmp/rewritten-project",
                    "gpt-5.6-terra",
                ),
                analytics_token_line("2026-07-21T01:02:01Z", 9_000, 900, 1_000)
            ),
        )
        .expect("rewrite analytics log");
        let rewritten =
            scan_codex_cost_analytics_roots_with_cache(&roots, now, None, &mut cache, |_| {});
        assert_eq!(rewritten.event_count, 1);
        assert_eq!(rewritten.total.total_tokens, 10_000);
        assert_eq!(rewritten.sessions[0].session_id, "rewritten-session");
        assert_eq!(cache.last_scan.reparsed_files, 1);
        assert_eq!(cache.last_scan.appended_files, 0);

        fs::remove_file(&path).expect("delete analytics log");
        let deleted =
            scan_codex_cost_analytics_roots_with_cache(&roots, now, None, &mut cache, |_| {});
        assert_eq!(deleted.source_path_count, 0);
        assert_eq!(deleted.event_count, 0);
        assert!(cache.files.is_empty());
        assert_eq!(cache.last_scan.evicted_files, 1);

        let _ = fs::remove_dir_all(root);
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let sequence = TEMP_DIR_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "codex-tools-token-usage-{}-{nanos}-{sequence}",
            std::process::id(),
        ))
    }
}
