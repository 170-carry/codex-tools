//! Read local App task state and verify native continuation without taking a writer lock.

use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub(crate) struct RolloutSnapshot {
    offset: u64,
    last_turn_id: Option<String>,
    incomplete: bool,
    last_activity_at: i64,
    session_id: Option<String>,
    thread_id: Option<String>,
    progress_records: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct LocalContinueTarget {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) rollout_path: PathBuf,
    pub(crate) snapshot: RolloutSnapshot,
}

fn record_time(value: &Value) -> Option<i64> {
    time::OffsetDateTime::parse(
        value.get("timestamp")?.as_str()?,
        &time::format_description::well_known::Rfc3339,
    )
    .ok()
    .map(|timestamp| timestamp.unix_timestamp())
}

impl RolloutSnapshot {
    fn observe(&mut self, record: &Value) {
        if let Some(timestamp) = record_time(record) {
            self.last_activity_at = self.last_activity_at.max(timestamp);
        }
        let payload = &record["payload"];
        if record["type"] == "response_item"
            && (matches!(payload["type"].as_str(), Some("reasoning" | "function_call" | "custom_tool_call"))
                || (payload["type"] == "message" && payload["role"] == "assistant"))
        {
            self.progress_records += 1;
        }
        if record["type"] == "session_meta" && self.thread_id.is_none() {
            self.thread_id = payload["id"].as_str().map(str::to_string);
            self.session_id = payload["session_id"].as_str().map(str::to_string);
        }
        if record["type"] != "event_msg" {
            return;
        }
        match payload["type"].as_str().unwrap_or_default() {
            "task_started" => {
                self.last_turn_id = payload["turn_id"].as_str().map(str::to_string);
                self.incomplete = true;
            }
            "task_complete" | "task_completed" | "turn_aborted" => {
                let turn_id = payload["turn_id"].as_str();
                // Replayed parent history or a late event for a previous turn
                // must never override the current task's state.
                if turn_id.is_some() && self.last_turn_id.as_deref() != turn_id {
                    return;
                }
                self.incomplete = payload["type"] == "turn_aborted"
                    || !payload["error"].is_null();
            }
            _ => {}
        }
    }
}

fn read_records(
    path: &Path,
    offset: &mut u64,
    mut observe: impl FnMut(&Value),
) -> Result<(), String> {
    let file = File::open(path).map_err(|error| format!("无法读取任务记录: {error}"))?;
    if file.metadata().map_err(|error| error.to_string())?.len() < *offset {
        return Err("任务记录在发送期间被替换，未重复发送继续消息".to_string());
    }
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(*offset)).map_err(|error| error.to_string())?;
    let mut line = Vec::new();
    loop {
        line.clear();
        let count = reader.read_until(b'\n', &mut line).map_err(|error| error.to_string())?;
        if count == 0 || !line.ends_with(b"\n") {
            // Keep a partially written line for the next poll.
            break;
        }
        *offset += count as u64;
        if let Ok(record) = serde_json::from_slice::<Value>(&line) {
            observe(&record);
        }
    }
    Ok(())
}

fn read_snapshot(path: &Path) -> Result<RolloutSnapshot, String> {
    let mut snapshot = RolloutSnapshot::default();
    let mut offset = 0;
    read_records(path, &mut offset, |record| snapshot.observe(record))?;
    snapshot.offset = offset;
    Ok(snapshot)
}

pub(crate) fn collect_local_targets(
    now: i64,
    window_minutes: u32,
) -> Result<Vec<LocalContinueTarget>, String> {
    collect_from_dir(&crate::app_paths::codex_dir()?, now, window_minutes)
}

fn collect_from_dir(
    codex_dir: &Path,
    now: i64,
    window_minutes: u32,
) -> Result<Vec<LocalContinueTarget>, String> {
    let mut databases = std::fs::read_dir(codex_dir)
        .map_err(|error| format!("无法读取 Codex 任务目录: {error}"))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let version = name.strip_prefix("state_")?.strip_suffix(".sqlite")?.parse::<u32>().ok()?;
            Some((version, entry.path()))
        })
        .collect::<Vec<_>>();
    databases.sort_by_key(|(version, _)| std::cmp::Reverse(*version));
    let database = databases.first().ok_or("未找到 Codex 本地任务数据库")?;
    let connection = Connection::open_with_flags(
        &database.1,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("无法只读打开 Codex 任务数据库: {error}"))?;
    connection.busy_timeout(Duration::from_secs(2)).map_err(|error| error.to_string())?;
    let has_custom_name = {
        let mut columns = connection.prepare("PRAGMA table_info(threads)").map_err(|error| error.to_string())?;
        let names = columns.query_map([], |row| row.get::<_, String>(1)).map_err(|error| error.to_string())?;
        names.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?.iter().any(|name| name == "name")
    };
    let label_expression = if has_custom_name { "COALESCE(NULLIF(name, ''), title)" } else { "title" };
    let mut query = connection
        .prepare(&format!("SELECT id, {label_expression}, rollout_path FROM threads WHERE source = 'vscode' AND archived = 0 ORDER BY updated_at DESC"))
        .map_err(|error| format!("无法读取 Codex 任务索引: {error}"))?;
    let rows = query
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))
        .map_err(|error| error.to_string())?;
    let cutoff = now.saturating_sub(i64::from(window_minutes.clamp(1, 1440)) * 60);
    let mut targets = Vec::new();
    for row in rows {
        let (id, title, path) = row.map_err(|error| error.to_string())?;
        if uuid::Uuid::parse_str(&id).is_err() {
            continue;
        }
        let rollout_path = PathBuf::from(path);
        let snapshot = match read_snapshot(&rollout_path) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                log::warn!("AUTO_CONTINUE thread={id} action=skip_unreadable error={error}");
                continue;
            }
        };
        if snapshot.thread_id.as_deref() != Some(&id)
            || snapshot.last_turn_id.is_none()
        {
            continue;
        }
        targets.push(LocalContinueTarget {
            label: if title.trim().is_empty() { id.clone() } else { title },
            id,
            rollout_path,
            snapshot,
        });
    }
    targets.sort_by_key(|target| std::cmp::Reverse(target.snapshot.last_activity_at));
    let mut seen = std::collections::HashSet::new();
    targets.retain(|target| seen.insert(target.snapshot.session_id.clone().unwrap_or_else(|| target.id.clone())));
    // Deduplicate before filtering completed tasks: a newer completed thread
    // in the same session supersedes an older interrupted copy.
    targets.retain(|target| target.snapshot.incomplete && target.snapshot.last_activity_at >= cutoff);
    Ok(targets)
}

/// Refresh immediately before sending so a completed task is not restarted and
/// activity that happened while navigating cannot falsely acknowledge our send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetState {
    NeedsContinue,
    Completed,
    Replaced,
    Running,
}

pub(crate) fn refresh_snapshot(target: &mut LocalContinueTarget) -> Result<TargetState, String> {
    let mut snapshot = target.snapshot.clone();
    let mut offset = snapshot.offset;
    read_records(&target.rollout_path, &mut offset, |record| snapshot.observe(record))?;
    snapshot.offset = offset;
    if snapshot.thread_id.as_deref() != Some(&target.id) {
        return Err("任务记录与目标会话不一致".to_string());
    }
    let same_turn = snapshot.last_turn_id == target.snapshot.last_turn_id;
    let progressing = snapshot.progress_records > target.snapshot.progress_records;
    target.snapshot = snapshot;
    // The user or App may already have started a replacement turn while the
    // composer was loading. Do not append another continue to that new task.
    Ok(if !target.snapshot.incomplete {
        TargetState::Completed
    } else if !same_turn {
        TargetState::Replaced
    } else if progressing {
        TargetState::Running
    } else {
        TargetState::NeedsContinue
    })
}

fn text_is_continue(text: &str) -> bool {
    let text = text.trim();
    if text == "继续" { return true; }
    // Desktop user messages can include App-provided context before the actual request.
    // Only accept the complete request, never a mention in context or quoted history.
    text.rsplit_once("\n## My request:\n")
        .is_some_and(|(context, request)| context.trim_end().ends_with('>') && request.trim() == "继续")
}

fn is_continue_message(record: &Value) -> bool {
    let payload = &record["payload"];
    if record["type"] == "event_msg" && payload["type"] == "user_message" {
        return payload["message"].as_str().is_some_and(text_is_continue);
    }
    if record["type"] != "response_item" || payload["type"] != "message" || payload["role"] != "user" {
        return false;
    }
    payload["content"].as_array().is_some_and(|content| {
        let text = content.iter().filter_map(|part| part["text"].as_str()).collect::<Vec<_>>().join("");
        text_is_continue(&text)
    })
}

#[derive(Default)]
struct SendConfirmation {
    new_turn_id: Option<String>,
    user_message: bool,
    failure: Option<String>,
}

impl SendConfirmation {
    fn observe(&mut self, record: &Value, previous_turn: Option<&str>) {
        self.user_message |= is_continue_message(record);
        if record["type"] != "event_msg" {
            return;
        }
        let payload = &record["payload"];
        let turn_id = payload["turn_id"].as_str();
        if payload["type"] == "task_started" && turn_id.is_some() && turn_id != previous_turn {
            self.new_turn_id = turn_id.map(str::to_string);
        }
        if self.new_turn_id.as_deref() != turn_id || turn_id.is_none() {
            return;
        }
        if payload["type"] == "turn_aborted" {
            self.failure = Some("继续消息对应的新回合已被中断".to_string());
        } else if matches!(payload["type"].as_str(), Some("task_complete" | "task_completed")) && !payload["error"].is_null() {
            self.failure = Some("继续消息对应的新回合立即执行失败，请查看 Codex 任务错误".to_string());
        }
    }
}

pub(crate) fn wait_for_new_turn(
    target: &LocalContinueTarget,
    timeout: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    let mut offset = target.snapshot.offset;
    let mut confirmation = SendConfirmation::default();
    let mut confirmed_at = None;
    loop {
        read_records(&target.rollout_path, &mut offset, |record| {
            confirmation.observe(record, target.snapshot.last_turn_id.as_deref());
        })?;
        if let Some(error) = confirmation.failure {
            return Err(error);
        }
        if confirmation.user_message {
            if let Some(turn_id) = confirmation.new_turn_id.as_ref() {
                let started = *confirmed_at.get_or_insert_with(Instant::now);
                // Allow promptly written quota/auth failures to arrive before
                // reporting acceptance. Later task failures remain App results.
                if timeout.is_zero() || started.elapsed() >= Duration::from_secs(1) || Instant::now() >= deadline {
                    return Ok(turn_id.clone());
                }
            }
        }
        if Instant::now() >= deadline {
            return Err("已尝试发送，但未确认目标任务产生新的继续回合；为避免重复消息未再次发送".to_string());
        }
        std::thread::sleep(Duration::from_millis(250).min(deadline.saturating_duration_since(Instant::now())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn event(kind: &str, turn: &str) -> Value {
        json!({"timestamp":"2026-10-21T08:00:00Z", "type":"event_msg", "payload":{"type":kind,"turn_id":turn}})
    }

    #[test]
    fn completion_errors_and_late_parent_events_preserve_latest_turn() {
        let mut state = RolloutSnapshot::default();
        state.observe(&event("task_started", "current"));
        state.observe(&event("task_complete", "parent"));
        assert!(state.incomplete);
        let mut failure = event("task_complete", "current");
        failure["payload"]["error"] = json!({"message":"usage limit"});
        state.observe(&failure);
        assert!(state.incomplete);
        state.observe(&event("task_complete", "current"));
        assert!(!state.incomplete);
        state.observe(&event("turn_aborted", "current"));
        assert!(state.incomplete);
    }

    #[test]
    fn send_confirmation_requires_new_turn_and_actual_continue_message() {
        let mut state = SendConfirmation::default();
        state.observe(&event("task_started", "old"), Some("old"));
        assert!(state.new_turn_id.is_none());
        state.observe(&event("task_started", "new"), Some("old"));
        assert!(!state.user_message);
        state.observe(&json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"继续"}]}}), Some("old"));
        assert!(!state.user_message);
        state.observe(&json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"继续"}]}}), Some("old"));
        assert!(state.user_message);
        assert_eq!(state.new_turn_id.as_deref(), Some("new"));
        state.observe(&event("turn_aborted", "new"), Some("old"));
        assert!(state.failure.is_some());
    }

    #[test]
    fn incomplete_appended_line_is_retried_without_losing_the_acknowledgement() {
        let path = std::env::temp_dir().join(format!("codex-continue-{}.jsonl", uuid::Uuid::new_v4()));
        let mut file = File::create(&path).unwrap();
        write!(file, "{}", event("task_started", "new")).unwrap();
        let mut offset = 0;
        let mut count = 0;
        read_records(&path, &mut offset, |_| count += 1).unwrap();
        assert_eq!((offset, count), (0, 0));
        writeln!(file).unwrap();
        read_records(&path, &mut offset, |_| count += 1).unwrap();
        assert_eq!(count, 1);
        read_records(&path, &mut offset, |_| count += 1).unwrap();
        assert_eq!(count, 1);
        drop(file);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn local_collection_uses_latest_database_and_only_recent_incomplete_app_tasks() {
        let root = std::env::temp_dir().join(format!("codex-local-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let database = root.join("state_5.sqlite");
        let connection = Connection::open(&database).unwrap();
        connection.execute_batch("CREATE TABLE threads (id TEXT, title TEXT, rollout_path TEXT, source TEXT, archived INTEGER, updated_at INTEGER)").unwrap();
        // An old schema file must not win over the latest database.
        std::fs::write(root.join("state_4.sqlite"), b"old schema").unwrap();
        let now = time::OffsetDateTime::parse("2026-10-21T08:10:00Z", &time::format_description::well_known::Rfc3339).unwrap().unix_timestamp();
        let mut expected = None;
        for (index, source, archived, status, timestamp, session) in [
            (0, "vscode", 0, "task_started", "2026-10-21T08:00:00Z", "shared"),
            (1, "vscode", 0, "task_started", "2026-10-21T08:05:00Z", "shared"),
            (2, "vscode", 0, "task_complete", "2026-10-21T08:06:00Z", "done"),
            (3, "cli", 0, "task_started", "2026-10-21T08:06:00Z", "cli"),
            (4, "vscode", 1, "task_started", "2026-10-21T08:06:00Z", "archived"),
            (5, "vscode", 0, "task_started", "2026-10-20T08:06:00Z", "stale"),
        ] {
            let id = uuid::Uuid::new_v4().to_string();
            if index == 1 { expected = Some(id.clone()); }
            let path = root.join(format!("rollout-{index}.jsonl"));
            let mut started = event("task_started", "turn");
            started["timestamp"] = json!(timestamp);
            let mut file = File::create(&path).unwrap();
            writeln!(file, "{}", json!({"type":"session_meta","payload":{"id":id,"session_id":session}})).unwrap();
            writeln!(file, "{started}").unwrap();
            if status == "task_complete" { writeln!(file, "{}", event(status, "turn")).unwrap(); }
            connection.execute("INSERT INTO threads VALUES (?1, ?2, ?3, ?4, ?5, 0)", rusqlite::params![id, format!("task-{index}"), path.to_str().unwrap(), source, archived]).unwrap();
        }
        drop(connection);
        let targets = collect_from_dir(&root, now, 30).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(Some(&targets[0].id), expected.as_ref());
        assert_eq!(targets[0].label, "task-1");
        let connection = Connection::open(&database).unwrap();
        connection.execute_batch("ALTER TABLE threads ADD COLUMN name TEXT").unwrap();
        connection.execute("UPDATE threads SET name = 'Renamed task' WHERE id = ?1", [expected.unwrap()]).unwrap();
        drop(connection);
        assert_eq!(collect_from_dir(&root, now, 30).unwrap()[0].label, "Renamed task");
        let mut latest = std::fs::OpenOptions::new().append(true).open(root.join("rollout-1.jsonl")).unwrap();
        writeln!(latest, "{}", event("task_complete", "turn")).unwrap();
        drop(latest);
        assert!(collect_from_dir(&root, now, 30).unwrap().is_empty(), "a completed latest copy must supersede the older incomplete thread");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verification_reads_only_new_records_and_completed_tasks_are_not_restarted() {
        let path = std::env::temp_dir().join(format!("codex-confirm-{}.jsonl", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{}", json!({"type":"session_meta","payload":{"id":id}})).unwrap();
        writeln!(file, "{}", event("task_started", "old")).unwrap();
        let mut target = LocalContinueTarget { id, label: "target".into(), rollout_path: path.clone(), snapshot: read_snapshot(&path).unwrap() };
        assert_eq!(refresh_snapshot(&mut target).unwrap(), TargetState::NeedsContinue);
        assert!(wait_for_new_turn(&target, Duration::ZERO).is_err());
        writeln!(file, "{}", event("task_started", "new")).unwrap();
        writeln!(file, "{}", json!({"type":"event_msg","payload":{"type":"user_message","message":"继续"}})).unwrap();
        assert_eq!(wait_for_new_turn(&target, Duration::ZERO).unwrap(), "new");
        assert_eq!(refresh_snapshot(&mut target).unwrap(), TargetState::Replaced, "a replacement turn must not be sent another continue");
        writeln!(file, "{}", event("task_complete", "new")).unwrap();
        assert_eq!(refresh_snapshot(&mut target).unwrap(), TargetState::Completed);
        drop(file);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn desktop_context_wrapped_continue_is_confirmed_but_mentions_are_not() {
        let wrapped = "<in-app-browser-context>Current page</in-app-browser-context>\n\n## My request:\n继续\n";
        let record = json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":wrapped}]}});
        assert!(is_continue_message(&record));
        assert!(text_is_continue("继续\n"));
        assert!(!text_is_continue("<context>继续</context>\n\n## My request:\n停止"));
        assert!(!text_is_continue("Example:\n## My request:\n继续"));
        assert!(!text_is_continue("继续，然后删除文件"));
        let mut assistant = record.clone();
        assistant["payload"]["role"] = json!("assistant");
        assert!(!is_continue_message(&assistant));
    }

    #[test]
    fn progress_during_readiness_is_detected_without_treating_navigation_as_execution() {
        let path = std::env::temp_dir().join(format!("codex-progress-{}.jsonl", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{}", json!({"type":"session_meta","payload":{"id":id}})).unwrap();
        writeln!(file, "{}", event("task_started", "original")).unwrap();
        let mut target = LocalContinueTarget { id, label: "target".into(), rollout_path: path.clone(), snapshot: read_snapshot(&path).unwrap() };
        writeln!(file, "{}", event("thread_settings_applied", "original")).unwrap();
        assert_eq!(refresh_snapshot(&mut target).unwrap(), TargetState::NeedsContinue);
        writeln!(file, "{}", json!({"type":"response_item","payload":{"type":"reasoning","summary":[]}})).unwrap();
        assert_eq!(refresh_snapshot(&mut target).unwrap(), TargetState::Running);
        drop(file);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn waiting_helper_exits_on_task_changes_even_when_it_never_emits_ready() {
        use crate::auto_switch_continue::{wait_for_windows_continue_helper, WindowsSubmission};
        use std::process::Stdio;

        for (kind, expected) in [("task_started", TargetState::Replaced), ("task_complete", TargetState::Completed)] {
            let path = std::env::temp_dir().join(format!("codex-waiting-{}.jsonl", uuid::Uuid::new_v4()));
            let id = uuid::Uuid::new_v4().to_string();
            let mut file = File::create(&path).unwrap();
            writeln!(file, "{}", json!({"type":"session_meta","payload":{"id":id}})).unwrap();
            writeln!(file, "{}", event("task_started", "original")).unwrap();
            let mut target = LocalContinueTarget { id, label: "target".into(), rollout_path: path.clone(), snapshot: read_snapshot(&path).unwrap() };
            // This disposable helper has no UI APIs and never reports READY.
            let child = crate::utils::new_background_command("powershell.exe")
                .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 20"])
                .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
                .spawn().unwrap();
            let writer = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(150));
                let turn = if kind == "task_started" { "replacement" } else { "original" };
                writeln!(file, "{}", event(kind, turn)).unwrap();
            });
            let started = Instant::now();
            let result = wait_for_windows_continue_helper(&mut target, child, Duration::from_secs(3)).unwrap();
            assert!(matches!(result, WindowsSubmission::NoLongerPending(state) if state == expected));
            assert!(started.elapsed() < Duration::from_secs(3), "must cancel without waiting for READY or timeout");
            writer.join().unwrap();
            std::fs::remove_file(path).unwrap();
        }
    }
}
