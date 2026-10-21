//! 在 Codex App 的原会话内续跑最近未完成的任务。
//!
//! Windows App 未提供控制 socket：只读本地任务索引/rollout，导航到目标会话，
//! 等待标题、预填“继续”的输入框及发送控件就绪，再复核原回合仍未完成后提交一次。
//! 必须从同一 rollout 确认“继续”消息和新的 task_started，才计为续跑成功。
//! 其他平台通过 App 常驻 app-server 的控制协议读取状态并追加 turn。
//!
//! 只处理 `source = vscode` 的用户会话和指定活动时间窗口（默认 30 分钟）；
//! 已完成、CLI、子代理和过期历史任务不恢复。不会另起无头执行会话。

use std::process::Stdio;
use std::time::Duration;

use serde_json::json;
use serde_json::Value;


const CONTINUE_PROMPT: &str = "继续";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const RESUME_TIMEOUT: Duration = Duration::from_secs(60);
const TURN_START_TIMEOUT: Duration = Duration::from_secs(120);
/// 单个任务允许执行的最长时间；续跑会一直等到它结束再退出。
/// 安全上限：进程最多保持这么久以支撑任务执行；
/// 任务通常远早于此结束，超时不代表失败。
const TURN_RUN_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);

/// App 内一条会话的续跑结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThreadVerdict {
    /// 最后一个 turn 已完成，不需要继续。
    Completed,
    /// 最后一个 turn 未完成（中断/失败/进行中/停滞），需要继续。
    Incomplete,
    /// 最后一个 turn 因额度耗尽失败：换号后可以再试，但同一账号不要反复重试。
    UsageLimited,
}

#[derive(Debug, Clone)]
pub(crate) struct AppThread {
    pub(crate) id: String,
    /// 会话树标识；同一 session 的多个 thread 只续跑一次。
    pub(crate) session_key: String,
    pub(crate) name: String,
    /// 任务最后一次活动的 unix 秒。
    ///
    /// 取以下来源的最大值：线程 `updatedAt`、最后一个 turn 的
    /// `completedAt`/`startedAt`、以及 rollout 日志最后一条记录的时间戳。
    /// 单独任何一个都不可靠：App 重启后 `updatedAt` 可能停在旧值，
    /// 而旧 turn 的结束时间又可能被重新写入。
    pub(crate) last_activity_at: i64,
    pub(crate) verdict: ThreadVerdict,
}

impl AppThread {
    fn label(&self) -> String {
        if self.name.trim().is_empty() {
            self.id.clone()
        } else {
            self.name.clone()
        }
    }
}

/// 判断一个 turn 状态是否算“未完成”。
pub(crate) fn turn_is_incomplete(status: &str) -> bool {
    matches!(status, "interrupted" | "failed" | "inProgress")
}

/// 从 `thread/read` 返回的线程对象里取出判定结果。
pub(crate) fn classify_thread_value(thread: &Value) -> Option<AppThread> {
    let id = thread.get("id")?.as_str()?.to_string();
    // 只处理 Codex App 的会话；CLI / exec / 子代理一律跳过。
    if thread.get("source").and_then(Value::as_str) != Some("vscode") {
        return None;
    }
    if thread
        .get("parentThreadId")
        .is_some_and(|value| !value.is_null())
    {
        return None;
    }
    let name = thread
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let updated_at = thread
        .get("updatedAt")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let turns = thread
        .get("turns")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let (verdict, mut last_activity_at) = match turns.last() {
        Some(last) => {
            let status = last
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("completed");
            let verdict = if turn_is_usage_limited(last) {
                ThreadVerdict::UsageLimited
            } else if turn_is_incomplete(status) {
                ThreadVerdict::Incomplete
            } else {
                ThreadVerdict::Completed
            };
            let completed = last.get("completedAt").and_then(Value::as_i64);
            let started = last.get("startedAt").and_then(Value::as_i64);
            (verdict, completed.into_iter().chain(started).max().unwrap_or(0))
        }
        // 没有任何 turn 的会话没有可续跑的内容。
        None => (ThreadVerdict::Completed, 0),
    };
    last_activity_at = last_activity_at.max(updated_at);
    if let Some(path) = thread.get("path").and_then(Value::as_str) {
        if let Some(logged_at) = last_rollout_timestamp(std::path::Path::new(path)) {
            last_activity_at = last_activity_at.max(logged_at);
        }
    }
    let session_key = thread
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| id.clone());
    Some(AppThread {
        id,
        session_key,
        name,
        last_activity_at,
        verdict,
    })
}

/// 读取 rollout 日志最后一条记录的时间戳，作为“任务真正停在哪一刻”的依据。
///
/// 线程元数据的 `updatedAt` 在 App 重启后可能停在旧值，
/// 而日志会忠实记录每一次 `task_started`/`token_count`/`task_complete`。
fn last_rollout_timestamp(path: &std::path::Path) -> Option<i64> {
    use std::io::Read;
    use std::io::Seek;
    use std::io::SeekFrom;

    const TAIL_BYTES: u64 = 64 * 1024;
    let metadata = std::fs::metadata(path).ok()?;
    let length = metadata.len();
    let start = length.saturating_sub(TAIL_BYTES);
    let mut file = std::fs::File::open(path).ok()?;
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).ok()?;
    let text = String::from_utf8_lossy(&buffer);
    // 从中间截断时第一行可能不完整，从后往前找第一条能解析的记录。
    for line in text.lines().rev() {
        let line = line.trim();
        if !line.starts_with('{') || !line.contains("\"timestamp\"") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(timestamp) = value.get("timestamp").and_then(Value::as_str) else {
            continue;
        };
        if let Some(parsed) = parse_rfc3339_seconds(timestamp) {
            return Some(parsed);
        }
    }
    None
}

fn parse_rfc3339_seconds(text: &str) -> Option<i64> {
    time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339)
        .ok()
        .map(|value| value.unix_timestamp())
}

/// 判断一个 turn 是否因额度耗尽而失败。
///
/// 这类失败即使标记为 `failed` 也不该在同一账号上反复重试：
/// 换号后任务才可能成功，因此单独归类，由调用方决定是否继续。
pub(crate) fn turn_is_usage_limited(turn: &Value) -> bool {
    let Some(error) = turn.get("error") else {
        return false;
    };
    if error
        .get("codexErrorInfo")
        .and_then(Value::as_str)
        .is_some_and(|info| info.eq_ignore_ascii_case("usageLimitExceeded"))
    {
        return true;
    }
    error
        .get("message")
        .and_then(Value::as_str)
        .map(|message| {
            let lower = message.to_ascii_lowercase();
            lower.contains("usage limit") || lower.contains("usage_limit")
        })
        .unwrap_or(false)
}

/// 在给定时间窗口内筛出需要继续的 App 会话，按最近活动时间倒序。
pub(crate) fn select_threads_to_continue(
    threads: Vec<AppThread>,
    now: i64,
    window_minutes: u32,
) -> Vec<AppThread> {
    let window_seconds = i64::from(window_minutes.max(1)) * 60;
    let cutoff = now.saturating_sub(window_seconds);
    let mut selected = Vec::new();
    // Codex 有时会为同一个 session 生成多个 thread 条目（分叉或重建），
    // 它们指向同一份会话历史，重复续跑会发两次“继续”。
    let mut seen_sessions: std::collections::HashSet<String> = std::collections::HashSet::new();
    // UsageLimited 也纳入：自动切换刚换了有额度的账号，
    // 之前因额度耗尽失败的任务正是最该重试的。
    let mut candidates = threads
        .into_iter()
        .filter(|thread| {
            matches!(
                thread.verdict,
                ThreadVerdict::Incomplete | ThreadVerdict::UsageLimited
            )
        })
        .filter(|thread| thread.last_activity_at >= cutoff)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.last_activity_at.cmp(&left.last_activity_at));
    for thread in candidates {
        if seen_sessions.insert(thread.session_key.clone()) {
            selected.push(thread);
        }
    }
    selected
}

/// 一个最小的 app-server 客户端：拉起 `codex app-server`，按 JSON-RPC 收发。

/// 续跑结果，供上层拼状态文案。
pub(crate) struct ContinueOutcome {
    pub(crate) continued: Vec<String>,
    pub(crate) failed: Vec<String>,
    pub(crate) skipped: Vec<String>,
    pub(crate) scanned: usize,
    pub(crate) window_minutes: u32,
}

/// 通过 App 常驻实例续跑一条会话。
///
/// 关键区别（实测）：
/// - **已加载**的线程（App 正在显示）直接 `turn/start` 即可，
///   不需要 `thread/resume`，也不会触发 `active writer`。
/// - **未加载**的线程才需要先 `thread/resume`；但若 App 持有写锁，
///   resume 会被拒绝，此时只能跳过并提示用户手动点击。
fn resume_via_app_control(
    client: &mut crate::app_control_socket::ControlSocket,
    thread_id: &str,
    already_loaded: bool,
) -> Result<(), String> {
    if !already_loaded {
        client.call(
            "thread/resume",
            json!({ "threadId": thread_id, "excludeTurns": true }),
            RESUME_TIMEOUT,
        )?;
    }
    let result = client.call(
        "turn/start",
        json!({
            "threadId": thread_id,
            "input": [{
                "type": "text",
                "text": CONTINUE_PROMPT,
                "text_elements": [],
            }],
        }),
        TURN_START_TIMEOUT,
    )?;
    let turn_id = result
        .get("turn")
        .and_then(|turn| turn.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    log::info!(
        "AUTO_CONTINUE thread={thread_id} turn={} loaded={already_loaded} action=queued_in_app",
        turn_id.unwrap_or_default()
    );
    Ok(())
}

/// 读取 App 当前已加载的线程 id 列表。
fn loaded_thread_ids(
    client: &mut crate::app_control_socket::ControlSocket,
) -> std::collections::HashSet<String> {
    match client.call("thread/loaded/list", json!({}), REQUEST_TIMEOUT) {
        Ok(result) => result
            .get("data")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        Err(error) => {
            log::warn!("读取已加载线程失败: {error}");
            std::collections::HashSet::new()
        }
    }
}

/// 通过 App 控制通道读取全部 App 会话状态。
fn collect_app_threads_via_control(
    control: &mut crate::app_control_socket::ControlSocket,
) -> Result<Vec<AppThread>, String> {
    let result = control.call(
        "thread/list",
        json!({ "limit": 50, "sortDirection": "desc" }),
        REQUEST_TIMEOUT,
    )?;
    let threads = result
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut collected = Vec::new();
    for thread in threads {
        let Some(id) = thread.get("id").and_then(Value::as_str) else {
            continue;
        };
        if thread.get("source").and_then(Value::as_str) != Some("vscode") {
            continue;
        }
        if thread
            .get("parentThreadId")
            .is_some_and(|value| !value.is_null())
        {
            continue;
        }
        let detail = match control.call(
            "thread/read",
            json!({ "threadId": id, "includeTurns": true }),
            REQUEST_TIMEOUT,
        ) {
            Ok(value) => value,
            Err(error) => {
                log::warn!("读取线程 {id} 状态失败: {error}");
                continue;
            }
        };
        if let Some(thread_value) = detail.get("thread") {
            if let Some(app_thread) = classify_thread_value(thread_value) {
                collected.push(app_thread);
            }
        }
    }
    Ok(collected)
}

/// 命令行 worker 使用与自动切换相同的续跑和确认流程。
pub(crate) fn run_continue_worker(window_minutes: u32) -> Result<String, String> {
    let now = crate::utils::now_unix_seconds();
    let outcome = continue_incomplete_threads(None, now, window_minutes)?;
    Ok(format!(
        "扫描 {} 条 App 会话，已确认续跑 {} 条，失败 {} 条{}",
        outcome.scanned,
        outcome.continued.len(),
        outcome.failed.len(),
        if outcome.failed.is_empty() {
            String::new()
        } else {
            format!("：{}", outcome.failed.join("；"))
        }
    ))
}

/// 确认 App 的 app-server 正在使用当前 `auth.json` 的账号。
///
/// 切换账号只改写 `auth.json`，常驻 app-server 可能仍缓存旧凭证。
/// 不匹配时直接报错，避免在旧账号上白白消耗额度。
fn app_server_matches_current_auth(
    control: &mut crate::app_control_socket::ControlSocket,
) -> Result<bool, String> {
    let current = crate::auth::read_current_codex_auth_optional()
        .ok()
        .flatten()
        .and_then(|auth| crate::auth::extract_auth(&auth).ok())
        .map(|auth| auth.account_id);
    let Some(current_account_id) = current else {
        return Ok(true);
    };
    let result = control.call("account/read", json!({}), REQUEST_TIMEOUT)?;
    let server_account_id = result
        .get("account")
        .and_then(|account| account.get("workspaceRouting"))
        .and_then(|routing| routing.get("chatgptAccountId"))
        .and_then(Value::as_str);
    Ok(match server_account_id {
        Some(server_id) => server_id == current_account_id,
        None => true,
    })
}

fn run_continue_via_app_control(now: i64, window_minutes: u32) -> Result<String, String> {
    let mut control = crate::app_control_socket::ControlSocket::connect_or_start(None)?;
    control.call(
        "initialize",
        json!({
            "clientInfo": {
                "name": "codex-tools-auto-switch",
                "title": "Codex Tools",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": Value::Null,
        }),
        REQUEST_TIMEOUT,
    )?;

    if !app_server_matches_current_auth(&mut control).unwrap_or(true) {
        return Err(
            "Codex App 仍在使用切换前的账号凭证，请重启 Codex App 后再续跑".to_string(),
        );
    }

    let threads = collect_app_threads_via_control(&mut control)?;
    let scanned = threads.len();
    let targets = select_threads_to_continue(threads, now, window_minutes);
    let loaded = loaded_thread_ids(&mut control);

    let mut text = format!(
        "通过 Codex App 实例续跑，扫描 {scanned} 条 App 会话，{window_minutes} 分钟窗口内待续跑 {} 条
",
        targets.len()
    );
    for thread in targets {
        let label = thread.label();
        let already_loaded = loaded.contains(&thread.id);
        match resume_via_app_control(&mut control, &thread.id, already_loaded) {
            Ok(()) => {
                log::info!("AUTO_CONTINUE thread={} action=queued_in_app", thread.id);
                text.push_str(&format!("  已继续: {label}
"));
            }
            Err(error) => {
                log::warn!("AUTO_CONTINUE thread={} action=failed error={error}", thread.id);
                text.push_str(&format!("  失败: {label}：{error}
"));
            }
        }
    }
    Ok(text)
}

/// 启动一个独立后台进程执行续跑，立即返回。
///
/// 主程序（自动切换流程）不需要等任务跑完，也不会因为自身退出而打断任务。
pub(crate) fn spawn_continue_worker(
    configured_path: Option<&str>,
    window_minutes: u32,
) -> Result<(), String> {
    // 必须启动 Codex Tools 自身（而不是 codex CLI），
    // `continue-worker` 是本工具的内部子命令。
    let exe = std::env::current_exe()
        .map_err(|error| format!("无法定位 Codex Tools 可执行文件: {error}"))?;
    let mut command = crate::utils::new_background_command(exe);
    command
        .arg("continue-worker")
        .arg("--window-minutes")
        .arg(window_minutes.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("启动续跑进程失败: {error}"))
}

/// 找出窗口内未完成的 App 会话并逐条续跑。
pub(crate) fn continue_incomplete_threads(
    configured_path: Option<&str>,
    now: i64,
    window_minutes: u32,
) -> Result<ContinueOutcome, String> {
    continue_incomplete_threads_with(configured_path, now, window_minutes, false)
}

/// 只列出将要续跑的会话，不真正执行；用于确认筛选规则。
pub(crate) fn preview_incomplete_threads(
    configured_path: Option<&str>,
    now: i64,
    window_minutes: u32,
) -> Result<ContinueOutcome, String> {
    continue_incomplete_threads_with(configured_path, now, window_minutes, true)
}

fn continue_incomplete_threads_with(
    configured_path: Option<&str>,
    now: i64,
    window_minutes: u32,
    dry_run: bool,
) -> Result<ContinueOutcome, String> {
    #[cfg(target_os = "windows")]
    {
        return continue_local_windows_threads(configured_path, now, window_minutes, dry_run);
    }
    #[cfg(not(target_os = "windows"))]
    continue_incomplete_threads_via_control(configured_path, now, window_minutes, dry_run)
}

fn continue_incomplete_threads_via_control(
    configured_path: Option<&str>,
    now: i64,
    window_minutes: u32,
    dry_run: bool,
) -> Result<ContinueOutcome, String> {
    let mut control = crate::app_control_socket::ControlSocket::connect_or_start(configured_path)?;
    control.call(
        "initialize",
        json!({
            "clientInfo": {
                "name": "codex-tools-auto-switch",
                "title": "Codex Tools",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": Value::Null,
        }),
        REQUEST_TIMEOUT,
    )?;

    let threads = collect_app_threads_via_control(&mut control)?;
    let scanned = threads.len();
    let targets = select_threads_to_continue(threads, now, window_minutes);
    if dry_run {
        return Ok(ContinueOutcome {
            continued: targets.into_iter().map(|thread| thread.label()).collect(),
            failed: Vec::new(),
            skipped: Vec::new(),
            scanned,
            window_minutes,
        });
    }

    let loaded = loaded_thread_ids(&mut control);
    let mut continued = Vec::new();
    let mut failed = Vec::new();
    for thread in targets {
        let label = thread.label();
        let already_loaded = loaded.contains(&thread.id);
        match resume_via_app_control(&mut control, &thread.id, already_loaded) {
            Ok(()) => {
                log::info!("AUTO_CONTINUE thread={} action=continued", thread.id);
                continued.push(label);
            }
            Err(error) => {
                log::warn!("AUTO_CONTINUE thread={} action=failed error={error}", thread.id);
                failed.push(format!("{label}：{error}"));
            }
        }
    }
    Ok(ContinueOutcome {
        continued,
        failed,
        skipped: Vec::new(),
        scanned,
        window_minutes,
    })
}

/// 供命令行自检使用：打印窗口内将被续跑的会话。
pub(crate) fn describe_preview(window_minutes: u32) -> String {
    let now = crate::utils::now_unix_seconds();
    match preview_incomplete_threads(None, now, window_minutes) {
        Ok(outcome) => {
            let mut text = format!(
                "扫描 {} 条 App 会话，{} 分钟窗口内待续跑 {} 条：
",
                outcome.scanned,
                outcome.window_minutes,
                outcome.continued.len()
            );
            if outcome.continued.is_empty() {
                text.push_str("  (无)
");
            }
            for label in &outcome.continued {
                text.push_str(&format!("  - {label}
"));
            }
            text
        }
        Err(error) => format!("检查失败: {error}
"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread_json(id: &str, source: &str, last_status: &str, updated_at: i64) -> Value {
        json!({
            "id": id,
            "sessionId": id,
            "source": source,
            "name": id,
            "updatedAt": updated_at,
            "path": Value::Null,
            "parentThreadId": Value::Null,
            "turns": [
                { "id": "t1", "status": "completed", "completedAt": updated_at - 100 },
                { "id": "t2", "status": last_status, "completedAt": updated_at },
            ],
        })
    }

    #[test]
    fn conversation_link_accepts_only_a_canonical_thread_uuid() {
        let id = "019a0123-4567-7abc-8def-0123456789ab";
        assert_eq!(continue_thread_uri(id).unwrap(), format!("codex://threads/{id}?prompt=%E7%BB%A7%E7%BB%AD"));
        assert!(continue_thread_uri("thread?prompt=unrelated").is_err());
        assert!(continue_thread_uri("../other-thread").is_err());
    }

    #[test]
    fn incomplete_means_interrupted_failed_or_in_progress() {
        assert!(turn_is_incomplete("interrupted"));
        assert!(turn_is_incomplete("failed"));
        assert!(turn_is_incomplete("inProgress"));
        assert!(!turn_is_incomplete("completed"));
    }

    #[test]
    fn cli_threads_are_ignored() {
        let cli = thread_json("cli-1", "cli", "interrupted", 1_000);
        assert!(classify_thread_value(&cli).is_none());

        let app = thread_json("app-1", "vscode", "interrupted", 1_000);
        let parsed = classify_thread_value(&app).expect("app thread should parse");
        assert_eq!(parsed.verdict, ThreadVerdict::Incomplete);
    }

    #[test]
    fn stale_updated_at_is_corrected_by_the_log_tail() {
        // 真实场景：Taskora 的 updatedAt 停在 01:32，但日志最后活动到 02:07。
        // 时间窗口必须按日志时间算，否则刚停止的任务会被当成过期任务跳过。
        let dir = std::env::temp_dir().join(format!(
            "codex-tools-auto-continue-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("rollout.jsonl");
        std::fs::write(
            &path,
            [
                r#"{"timestamp":"2026-10-21T02:00:00.000Z","type":"event_msg","payload":{"type":"task_started"}}"#,
                r#"{"timestamp":"2026-10-21T02:07:10.000Z","type":"event_msg","payload":{"type":"task_complete"}}"#,
            ]
            .join("
"),
        )
        .expect("write rollout");

        let now = 1_792_520_000; // 2026-10-21T02:13:20Z
        let mut value = thread_json("taskora", "vscode", "failed", now - 2_400);
        value["path"] = json!(path.to_string_lossy());
        let thread = classify_thread_value(&value).expect("thread should parse");
        // 日志里的 02:07:10 比 updatedAt 新，应被采用。
        assert_eq!(
            thread.last_activity_at,
            parse_rfc3339_seconds("2026-10-21T02:07:10.000Z").unwrap()
        );
        assert_eq!(select_threads_to_continue(vec![thread], now, 30).len(), 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn activity_time_takes_the_latest_available_source() {
        // 三个来源取最大：updatedAt、turn 的结束时间、日志尾部。
        // 这里 turn 结束时间比 updatedAt 新，应采信 turn。
        let now = 10_000;
        let mut value = thread_json("stale", "vscode", "interrupted", now - 3_600);
        value["turns"][1]["completedAt"] = json!(now - 10);
        let thread = classify_thread_value(&value).expect("thread should parse");
        assert_eq!(thread.last_activity_at, now - 10);
    }

    #[test]
    fn usage_limit_failures_are_recognized() {
        let limited = json!({
            "status": "failed",
            "error": {
                "message": "You've hit your usage limit. Try again at 5:47 AM.",
                "codexErrorInfo": "usageLimitExceeded",
            },
        });
        assert!(turn_is_usage_limited(&limited));

        let other = json!({
            "status": "failed",
            "error": { "message": "network error", "codexErrorInfo": "other" },
        });
        assert!(!turn_is_usage_limited(&other));

        let ok = json!({ "status": "completed", "error": null });
        assert!(!turn_is_usage_limited(&ok));
    }

    #[test]
    fn usage_limited_turn_is_retried_after_switching_accounts() {
        // 因额度耗尽失败的任务，在换到有额度的账号后应该继续。
        let mut value = thread_json("limited", "vscode", "failed", 9_000);
        value["turns"][1]["error"] = json!({
            "message": "You've hit your usage limit.",
            "codexErrorInfo": "usageLimitExceeded",
        });
        let thread = classify_thread_value(&value).expect("thread should parse");
        assert_eq!(thread.verdict, ThreadVerdict::UsageLimited);
        assert_eq!(select_threads_to_continue(vec![thread], 10_000, 30).len(), 1);
    }

    #[test]
    fn duplicate_threads_for_the_same_session_are_collapsed() {
        // Codex 可能为同一 session 生成多个 thread 条目（分叉/重建）。
        // 只保留最近活动的那条，避免重复发送“继续”。
        let now = 10_000;
        let mut newer = thread_json("thread-new", "vscode", "interrupted", now - 60);
        newer["sessionId"] = json!("shared-session");
        let mut older = thread_json("thread-old", "vscode", "interrupted", now - 600);
        older["sessionId"] = json!("shared-session");

        let threads = vec![
            classify_thread_value(&newer).unwrap(),
            classify_thread_value(&older).unwrap(),
        ];
        let selected = select_threads_to_continue(threads, now, 30);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "thread-new");
    }

    #[test]
    fn completed_thread_is_not_selected() {
        let threads = vec![
            classify_thread_value(&thread_json("done", "vscode", "completed", 1_000)).unwrap(),
            classify_thread_value(&thread_json("stopped", "vscode", "interrupted", 1_000)).unwrap(),
        ];
        let selected = select_threads_to_continue(threads, 1_000, 30);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "stopped");
    }

    #[test]
    fn threads_older_than_window_are_skipped() {
        let now = 10_000;
        let threads = vec![
            // 5 分钟前：窗口内
            classify_thread_value(&thread_json("recent", "vscode", "interrupted", now - 300))
                .unwrap(),
            // 45 分钟前：超出默认 30 分钟
            classify_thread_value(&thread_json("old", "vscode", "interrupted", now - 2_700))
                .unwrap(),
        ];
        let selected = select_threads_to_continue(threads.clone(), now, 30);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "recent");

        // 窗口放宽到 60 分钟后，两条都要续跑。
        let widened = select_threads_to_continue(threads, now, 60);
        assert_eq!(widened.len(), 2);
    }

    #[test]
    fn selection_is_ordered_by_most_recent_activity() {
        let now = 10_000;
        let threads = vec![
            classify_thread_value(&thread_json("older", "vscode", "interrupted", now - 900))
                .unwrap(),
            classify_thread_value(&thread_json("newer", "vscode", "interrupted", now - 60))
                .unwrap(),
        ];
        let selected = select_threads_to_continue(threads, now, 30);
        assert_eq!(
            selected.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            vec!["newer", "older"]
        );
    }

    #[test]
    fn subagent_threads_are_ignored() {
        let mut value = thread_json("sub", "vscode", "interrupted", 1_000);
        value["parentThreadId"] = json!("parent-thread");
        assert!(classify_thread_value(&value).is_none());
    }
}

/// Windows Codex builds do not expose the local daemon control socket. Open the
/// existing conversation with a prefilled prompt and wait for its own composer.
#[cfg(target_os = "windows")]
pub(crate) enum WindowsSubmission {
    Submitted,
    NoLongerPending(crate::auto_switch_local::TargetState),
}

#[cfg(target_os = "windows")]
fn submit_continue_to_windows_thread(
    target: &mut crate::auto_switch_local::LocalContinueTarget,
    app_path: Option<&std::path::Path>,
) -> Result<WindowsSubmission, String> {
    use base64::Engine;

    let uri = continue_thread_uri(&target.id)?;
    let script = include_str!("auto_switch_continue_windows.ps1");
    // Keep the command line short: the embedded script can exceed Windows'
    // command-line limit once converted to UTF-16 Base64. Stdin stays reserved
    // for the READY/SUBMIT handshake.
    let bootstrap = "& ([scriptblock]::Create($env:CODEX_TOOLS_CONTINUE_SCRIPT))";
    let encoded = base64::engine::general_purpose::STANDARD.encode(
        bootstrap.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>(),
    );
    let child = crate::utils::new_background_command("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-STA", "-EncodedCommand", &encoded])
        .env("CODEX_TOOLS_CONTINUE_URI", uri)
        .env("CODEX_TOOLS_CONTINUE_TITLE", &target.label)
        .env("CODEX_TOOLS_CONTINUE_PROMPT", CONTINUE_PROMPT)
        .env("CODEX_TOOLS_CONTINUE_SCRIPT", script)
        .env("CODEX_TOOLS_CONTINUE_APP_PATH", app_path.unwrap_or_else(|| std::path::Path::new("")))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("启动 Codex 任务就绪检查失败: {error}"))?;
    wait_for_windows_continue_helper(target, child, Duration::from_secs(125))
}

#[cfg(target_os = "windows")]
pub(crate) fn wait_for_windows_continue_helper(
    target: &mut crate::auto_switch_local::LocalContinueTarget,
    mut child: std::process::Child,
    timeout: Duration,
) -> Result<WindowsSubmission, String> {
    use std::io::{BufRead, Read, Write};

    // Drain both pipes while the child is running, including PowerShell error output.
    let stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let (sender, receiver) = std::sync::mpsc::channel();
    let stdout_reader = std::thread::spawn(move || {
        let mut lines = Vec::new();
        for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = sender.send(line.clone());
            lines.push(line);
        }
        lines
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let deadline = std::time::Instant::now() + timeout;
    let mut authorized = false;
    let mut no_longer_pending = None;
    let mut exit_status = None;
    let mut next_refresh = std::time::Instant::now();
    let mut last_stage = String::new();
    let result = (|| {
        loop {
            while let Ok(line) = receiver.try_recv() {
                if line.starts_with("TRACE ") {
                    log::info!("AUTO_CONTINUE thread={} {}", target.id, line);
                    last_stage = line;
                    continue;
                }
                if line.trim() != "READY" { continue; }
                if authorized {
                    return Err("任务就绪检查返回了重复的发送请求，已停止".to_string());
                }
                // Navigation can take minutes. Only the original, still-incomplete
                // turn may receive a follow-up after its composer has loaded.
                let state = crate::auto_switch_local::refresh_snapshot(target)?;
                let incomplete = state == crate::auto_switch_local::TargetState::NeedsContinue;
                if !incomplete { no_longer_pending = Some(state); }
                authorized = incomplete;
                let mut input = child.stdin.take().ok_or("任务就绪检查输入通道已关闭")?;
                writeln!(input, "{}", if incomplete { "SUBMIT" } else { "CANCEL" })
                    .and_then(|_| input.flush())
                    .map_err(|error| format!("发送任务就绪确认失败: {error}"))?;
            }
            // A running or completed task may never expose Send. Observe its
            // rollout while waiting, rather than only after the READY handshake.
            if no_longer_pending.is_some() { return Ok(()); }
            if !authorized && std::time::Instant::now() >= next_refresh {
                let state = crate::auto_switch_local::refresh_snapshot(target)?;
                if state != crate::auto_switch_local::TargetState::NeedsContinue {
                    no_longer_pending = Some(state);
                    return Ok(());
                }
                next_refresh = std::time::Instant::now() + Duration::from_millis(500);
            }
            if let Some(status) = child.try_wait()
                .map_err(|error| format!("检查 Codex 任务就绪进程失败: {error}"))?
            {
                exit_status = Some(status);
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(format!("等待 Codex 目标任务输入框就绪超时：{last_stage}，未再次发送"));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    })();
    if exit_status.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let lines = stdout_reader.join().unwrap_or_default();
    let errors = stderr_reader.join().unwrap_or_default();
    result?;
    if let Some(state) = no_longer_pending {
        return Ok(WindowsSubmission::NoLongerPending(state));
    }
    if exit_status.is_some_and(|status| status.success()) && authorized && lines.iter().any(|line| line.trim() == "SUBMITTED") {
        Ok(WindowsSubmission::Submitted)
    } else {
        let error = String::from_utf8_lossy(&errors);
        let detail = error.lines().filter(|line| !line.trim().is_empty()).take(3)
            .collect::<Vec<_>>().join(" ").chars().take(500).collect::<String>();
        Err(format!("Codex 目标任务输入框尚未就绪或发送结果不明：{detail}；{last_stage}"))
    }
}

fn continue_thread_uri(thread_id: &str) -> Result<String, String> {
    let id = uuid::Uuid::parse_str(thread_id)
        .map_err(|_| "Codex 任务 ID 不是有效的 UUID".to_string())?;
    Ok(format!("codex://threads/{id}?prompt=%E7%BB%A7%E7%BB%AD"))
}

#[cfg(target_os = "windows")]
fn continue_local_windows_threads(
    configured_path: Option<&str>,
    now: i64,
    window_minutes: u32,
    dry_run: bool,
) -> Result<ContinueOutcome, String> {
    let targets = crate::auto_switch_local::collect_local_targets(now, window_minutes)?;
    let app_path = crate::cli::find_configured_codex_app_path(configured_path)
        .or_else(crate::cli::find_codex_app_path);
    let mut outcome = ContinueOutcome {
        continued: Vec::new(),
        failed: Vec::new(),
        skipped: Vec::new(),
        scanned: targets.len(),
        window_minutes,
    };
    for mut target in targets {
        if dry_run {
            outcome.continued.push(target.label);
            continue;
        }
        log::info!("AUTO_CONTINUE thread={} action=waiting_for_composer", target.id);
        let submitted = submit_continue_to_windows_thread(&mut target, app_path.as_deref());
        if let Ok(WindowsSubmission::NoLongerPending(state)) = submitted {
            log::info!("AUTO_CONTINUE thread={} action=no_longer_pending state={state:?}", target.id);
            let reason = match state {
                crate::auto_switch_local::TargetState::Completed => "任务已完成",
                crate::auto_switch_local::TargetState::Replaced => "已产生新回合",
                crate::auto_switch_local::TargetState::Running => "任务已恢复执行",
                crate::auto_switch_local::TargetState::NeedsContinue => unreachable!(),
            };
            outcome.skipped.push(format!("{}（{reason}）", target.label));
            continue;
        }
        // Even a failed native invocation can have submitted before its response
        // was lost. Verify once; never retry an uncertain submission.
        match crate::auto_switch_local::wait_for_new_turn(&target, Duration::from_secs(30)) {
            Ok(turn_id) => {
                log::info!("AUTO_CONTINUE thread={} turn={turn_id} action=confirmed_in_app", target.id);
                outcome.continued.push(target.label);
            }
            Err(error) => {
                let detail = match submitted {
                    Ok(_) => error,
                    Err(submit_error) => format!("{submit_error}；{error}"),
                };
                log::warn!("AUTO_CONTINUE thread={} action=failed error={detail}", target.id);
                outcome.failed.push(format!("{}：{detail}", target.label));
            }
        }
    }
    Ok(outcome)
}
