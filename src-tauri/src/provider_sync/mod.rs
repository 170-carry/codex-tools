use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use rusqlite::Connection;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use toml_edit::value;
use toml_edit::DocumentMut;
use uuid::Uuid;

use crate::app_paths;
use crate::utils;

const BACKUP_ROOT_NAME: &str = "backups_state/provider-sync";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const STATE_DB_FILE_NAME: &str = "state_5.sqlite";
const DEFAULT_PROVIDER: &str = "openai";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentProvider {
    pub(crate) id: String,
    pub(crate) source: String,
    pub(crate) base_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderScopeCounts {
    pub(crate) sessions: BTreeMap<String, u64>,
    pub(crate) archived_sessions: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SqliteProviderCounts {
    pub(crate) threads: BTreeMap<String, u64>,
    pub(crate) total_threads: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupSummary {
    pub(crate) available: bool,
    pub(crate) backup_count: usize,
    pub(crate) latest_backup_id: Option<String>,
    pub(crate) backup_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderStatus {
    pub(crate) codex_home: String,
    pub(crate) current_provider: CurrentProvider,
    pub(crate) configured_providers: Vec<String>,
    pub(crate) rollout_counts: ProviderScopeCounts,
    pub(crate) encrypted_content_counts: ProviderScopeCounts,
    pub(crate) unreadable_rollout_files: Vec<String>,
    pub(crate) sqlite_present: bool,
    pub(crate) sqlite_path: Option<String>,
    pub(crate) sqlite_counts: Option<SqliteProviderCounts>,
    pub(crate) backup_summary: BackupSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderSyncResult {
    pub(crate) codex_home: String,
    pub(crate) provider: String,
    pub(crate) rollout_files_scanned: usize,
    pub(crate) rollout_files_changed: usize,
    pub(crate) sqlite_present: bool,
    pub(crate) sqlite_threads_changed: u64,
    pub(crate) backup_id: Option<String>,
    pub(crate) backup_path: Option<String>,
    pub(crate) encrypted_content_counts: ProviderScopeCounts,
    pub(crate) unreadable_rollout_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderRestoreResult {
    pub(crate) codex_home: String,
    pub(crate) backup_id: String,
    pub(crate) restored_rollout_files: usize,
    pub(crate) restored_sqlite: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderPruneResult {
    pub(crate) codex_home: String,
    pub(crate) kept: usize,
    pub(crate) removed: usize,
}

#[derive(Debug, Clone, Copy)]
enum RolloutScope {
    Sessions,
    ArchivedSessions,
}

#[derive(Debug, Clone)]
struct SessionFileChange {
    path: PathBuf,
    updated_text: String,
}

#[derive(Debug, Default)]
struct SessionScan {
    provider_counts: ProviderScopeCounts,
    encrypted_content_counts: ProviderScopeCounts,
    changes: Vec<SessionFileChange>,
    unreadable_files: Vec<String>,
    rollout_files_scanned: usize,
    thread_ids: BTreeSet<String>,
    user_event_thread_ids: BTreeSet<String>,
    thread_cwd_by_id: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct SqliteSchema {
    id_column: String,
    has_model_provider: bool,
    has_user_event: bool,
    has_cwd: bool,
}

#[derive(Debug)]
struct BackupHandle {
    id: String,
    path: PathBuf,
    manifest: ProviderSyncBackupManifest,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderSyncBackupManifest {
    id: String,
    provider: String,
    created_at: i64,
    codex_home: String,
    rollout_files: Vec<BackupFileEntry>,
    sqlite: Option<BackupFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupFileEntry {
    source_path: String,
    backup_path: String,
    existed: bool,
}

pub(crate) fn get_status(codex_home: Option<&Path>) -> Result<ProviderStatus, String> {
    let codex_home = resolve_codex_home(codex_home)?;
    let config_text = read_config_text(&codex_home)?;
    let current_provider = read_current_provider_from_config_text(&config_text);
    let scan = scan_rollout_files(&codex_home, current_provider.id.as_str())?;
    let sqlite_path = sqlite_path(&codex_home).filter(|path| path.is_file());

    Ok(ProviderStatus {
        codex_home: codex_home.to_string_lossy().to_string(),
        current_provider,
        configured_providers: configured_provider_ids(&config_text),
        rollout_counts: scan.provider_counts,
        encrypted_content_counts: scan.encrypted_content_counts,
        unreadable_rollout_files: scan.unreadable_files,
        sqlite_present: sqlite_path.is_some(),
        sqlite_path: sqlite_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        sqlite_counts: read_sqlite_provider_counts(&codex_home)?,
        backup_summary: get_backup_summary(&codex_home)?,
    })
}

pub(crate) fn sync_current_provider(
    codex_home: Option<&Path>,
) -> Result<ProviderSyncResult, String> {
    let codex_home = resolve_codex_home(codex_home)?;
    let config_text = read_config_text(&codex_home)?;
    let current_provider = read_current_provider_from_config_text(&config_text);
    sync_provider_in_home(&codex_home, current_provider.id.as_str(), false)
}

pub(crate) fn sync_provider(
    codex_home: Option<&Path>,
    provider: &str,
) -> Result<ProviderSyncResult, String> {
    let codex_home = resolve_codex_home(codex_home)?;
    sync_provider_in_home(&codex_home, provider, true)
}

pub(crate) fn switch_provider(
    codex_home: Option<&Path>,
    provider: &str,
) -> Result<ProviderSyncResult, String> {
    let codex_home = resolve_codex_home(codex_home)?;
    let provider = normalize_provider_id(provider)?;
    let config_text = read_config_text(&codex_home)?;
    ensure_configured_provider(&config_text, provider.as_str())?;

    let config_path = config_path(&codex_home);
    let mut document = parse_config_or_default(Some(config_text.as_str()));
    document["model_provider"] = value(provider.as_str());
    write_bytes_atomically(&config_path, document.to_string().as_bytes())?;

    sync_provider_in_home(&codex_home, provider.as_str(), false)
}

pub(crate) fn restore_backup(
    codex_home: Option<&Path>,
    backup_id: Option<&str>,
) -> Result<ProviderRestoreResult, String> {
    let codex_home = resolve_codex_home(codex_home)?;
    let backup_id = match backup_id {
        Some(value) => value.to_string(),
        None => latest_backup_id(&codex_home)?
            .ok_or_else(|| "没有可恢复的 provider 同步备份".to_string())?,
    };
    let manifest = read_backup_manifest(&codex_home, backup_id.as_str())?;
    let mut restored_rollout_files = 0;
    for entry in &manifest.rollout_files {
        restore_backup_entry(entry)?;
        restored_rollout_files += 1;
    }
    let mut restored_sqlite = false;
    if let Some(entry) = manifest.sqlite.as_ref() {
        restore_backup_entry(entry)?;
        restored_sqlite = true;
    }

    Ok(ProviderRestoreResult {
        codex_home: codex_home.to_string_lossy().to_string(),
        backup_id,
        restored_rollout_files,
        restored_sqlite,
    })
}

pub(crate) fn prune_backups(
    codex_home: Option<&Path>,
    keep: usize,
) -> Result<ProviderPruneResult, String> {
    let codex_home = resolve_codex_home(codex_home)?;
    let mut backup_ids = list_backup_ids(&codex_home)?;
    backup_ids.sort();
    backup_ids.reverse();

    let mut removed = 0;
    for backup_id in backup_ids.iter().skip(keep) {
        let path = backup_root(&codex_home).join(backup_id);
        fs::remove_dir_all(&path)
            .map_err(|error| format!("删除 provider 同步备份失败 {}: {error}", path.display()))?;
        removed += 1;
    }

    Ok(ProviderPruneResult {
        codex_home: codex_home.to_string_lossy().to_string(),
        kept: backup_ids.len().min(keep),
        removed,
    })
}

fn sync_provider_in_home(
    codex_home: &Path,
    provider: &str,
    explicit_provider: bool,
) -> Result<ProviderSyncResult, String> {
    let provider = normalize_provider_id(provider)?;
    let config_text = read_config_text(codex_home)?;
    if explicit_provider {
        ensure_configured_provider(&config_text, provider.as_str())?;
    }

    let scan = scan_rollout_files(codex_home, provider.as_str())?;
    let sqlite_present = sqlite_path(codex_home)
        .map(|path| path.is_file())
        .unwrap_or(false);
    let sqlite_threads_changed = count_sqlite_threads_to_update(codex_home, provider.as_str())?;
    let backup = create_backup(
        codex_home,
        provider.as_str(),
        &scan.changes,
        sqlite_present && sqlite_threads_changed > 0,
    )?;

    // SQLite owns Codex's history index. Keep its update in a transaction while
    // rollout JSONL files are rewritten, so either both stores advance or the DB rolls back.
    let sqlite_conn = if sqlite_present && sqlite_threads_changed > 0 {
        let conn = open_sqlite_connection(codex_home, "更新 Codex 历史 provider")?;
        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(|error| sqlite_error("锁定 Codex 历史索引", error))?;
        apply_sqlite_provider_update(&conn, provider.as_str(), &scan)?;
        Some(conn)
    } else {
        None
    };

    let mut written_rollout_paths = Vec::new();
    for change in &scan.changes {
        if let Err(error) = write_bytes_atomically(&change.path, change.updated_text.as_bytes()) {
            rollback_sqlite(sqlite_conn.as_ref());
            restore_written_rollouts(backup.as_ref(), &written_rollout_paths);
            return Err(error);
        }
        written_rollout_paths.push(change.path.clone());
    }

    if let Some(conn) = sqlite_conn.as_ref() {
        if let Err(error) = conn.execute_batch("COMMIT") {
            restore_written_rollouts(backup.as_ref(), &written_rollout_paths);
            return Err(sqlite_error("提交 Codex 历史 provider 更新", error));
        }
    }

    Ok(ProviderSyncResult {
        codex_home: codex_home.to_string_lossy().to_string(),
        provider,
        rollout_files_scanned: scan.rollout_files_scanned,
        rollout_files_changed: scan.changes.len(),
        sqlite_present,
        sqlite_threads_changed,
        backup_id: backup.as_ref().map(|backup| backup.id.clone()),
        backup_path: backup
            .as_ref()
            .map(|backup| backup.path.to_string_lossy().to_string()),
        encrypted_content_counts: scan.encrypted_content_counts,
        unreadable_rollout_files: scan.unreadable_files,
    })
}

fn resolve_codex_home(codex_home: Option<&Path>) -> Result<PathBuf, String> {
    match codex_home {
        Some(path) => Ok(path.to_path_buf()),
        None => app_paths::codex_dir(),
    }
}

fn normalize_provider_id(provider: &str) -> Result<String, String> {
    let provider = provider.trim();
    if provider.is_empty() {
        return Err("provider 不能为空".to_string());
    }
    Ok(provider.to_string())
}

fn config_path(codex_home: &Path) -> PathBuf {
    codex_home.join("config.toml")
}

fn sqlite_path(codex_home: &Path) -> Option<PathBuf> {
    Some(codex_home.join(STATE_DB_FILE_NAME))
}

fn read_config_text(codex_home: &Path) -> Result<String, String> {
    let path = config_path(codex_home);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!(
            "读取 Codex config.toml 失败 {}: {error}",
            path.display()
        )),
    }
}

fn parse_config_or_default(current_config: Option<&str>) -> DocumentMut {
    current_config
        .and_then(|raw| raw.parse::<DocumentMut>().ok())
        .unwrap_or_default()
}

fn read_current_provider_from_config_text(config_text: &str) -> CurrentProvider {
    let document = parse_config_or_default(Some(config_text));
    let id = document
        .get("model_provider")
        .and_then(|item| item.as_str())
        .unwrap_or(DEFAULT_PROVIDER)
        .to_string();
    let base_url = document
        .get("openai_base_url")
        .and_then(|item| item.as_str())
        .map(str::to_string);
    let source = if document
        .get("model_provider")
        .and_then(|item| item.as_str())
        .is_some()
    {
        "config".to_string()
    } else {
        "default".to_string()
    };

    CurrentProvider {
        id,
        source,
        base_url,
    }
}

fn configured_provider_ids(config_text: &str) -> Vec<String> {
    let document = parse_config_or_default(Some(config_text));
    let mut ids = vec![DEFAULT_PROVIDER.to_string()];
    if let Some(table) = document
        .get("model_providers")
        .and_then(|item| item.as_table())
    {
        ids.extend(table.iter().map(|(key, _)| key.to_string()));
    }
    ids.sort();
    ids.dedup();
    ids
}

fn ensure_configured_provider(config_text: &str, provider: &str) -> Result<(), String> {
    let providers = configured_provider_ids(config_text);
    if providers.iter().any(|candidate| candidate == provider) {
        return Ok(());
    }
    Err(format!(
        "config.toml 中没有配置 provider `{provider}`，请先在 [model_providers.{provider}] 中配置后再同步"
    ))
}

fn scan_rollout_files(codex_home: &Path, target_provider: &str) -> Result<SessionScan, String> {
    let mut scan = SessionScan::default();
    scan_rollout_root(
        &codex_home.join("sessions"),
        RolloutScope::Sessions,
        target_provider,
        &mut scan,
    )?;
    scan_rollout_root(
        &codex_home.join("archived_sessions"),
        RolloutScope::ArchivedSessions,
        target_provider,
        &mut scan,
    )?;
    Ok(scan)
}

fn scan_rollout_root(
    root: &Path,
    scope: RolloutScope,
    target_provider: &str,
    scan: &mut SessionScan,
) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    let mut files = Vec::new();
    collect_jsonl_files(root, &mut files)?;
    for path in files {
        scan.rollout_files_scanned += 1;
        if let Err(error) = scan_rollout_file(&path, scope, target_provider, scan) {
            scan.unreadable_files
                .push(format!("{}: {error}", path.to_string_lossy()));
        }
    }
    Ok(())
}

fn collect_jsonl_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|error| format!("读取 Codex sessions 目录失败 {}: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取 Codex sessions 条目失败: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, files)?;
        } else if path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("jsonl"))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    Ok(())
}

fn scan_rollout_file(
    path: &Path,
    scope: RolloutScope,
    target_provider: &str,
    scan: &mut SessionScan,
) -> Result<(), String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("读取 rollout 文件失败 {}: {error}", path.display()))?;
    let Some((first_line, separator, rest)) = split_first_jsonl_line(raw.as_str()) else {
        return Ok(());
    };
    let mut root: Value = serde_json::from_str(first_line)
        .map_err(|error| format!("解析 session_meta 失败: {error}"))?;
    if root.get("type").and_then(Value::as_str) != Some("session_meta") {
        return Ok(());
    }
    let Some(payload) = root.get_mut("payload").and_then(Value::as_object_mut) else {
        return Ok(());
    };

    let existing_provider = payload
        .get("model_provider")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PROVIDER)
        .to_string();
    increment_provider_count(&mut scan.provider_counts, scope, existing_provider.as_str());
    if raw.contains("encrypted_content") {
        increment_provider_count(
            &mut scan.encrypted_content_counts,
            scope,
            existing_provider.as_str(),
        );
    }
    if let Some(id) = payload.get("id").and_then(Value::as_str) {
        scan.thread_ids.insert(id.to_string());
        if rollout_has_user_event(raw.as_str()) {
            scan.user_event_thread_ids.insert(id.to_string());
        }
        if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
            if !cwd.trim().is_empty() {
                scan.thread_cwd_by_id
                    .insert(id.to_string(), cwd.to_string());
            }
        }
    }

    let needs_rewrite = payload
        .get("model_provider")
        .and_then(Value::as_str)
        .map(|provider| provider != target_provider)
        .unwrap_or(true);
    if needs_rewrite {
        // Codex filters history by this rollout metadata; make every migrated
        // session advertise the active provider instead of the stale custom one.
        payload.insert(
            "model_provider".to_string(),
            Value::String(target_provider.to_string()),
        );
        let mut updated_text = serde_json::to_string(&root)
            .map_err(|error| format!("序列化 session_meta 失败: {error}"))?;
        updated_text.push_str(separator);
        updated_text.push_str(rest);
        scan.changes.push(SessionFileChange {
            path: path.to_path_buf(),
            updated_text,
        });
    }

    Ok(())
}

fn split_first_jsonl_line(raw: &str) -> Option<(&str, &str, &str)> {
    if raw.is_empty() {
        return None;
    }
    if let Some(index) = raw.find('\n') {
        let first = &raw[..index];
        let rest = &raw[index + 1..];
        if let Some(stripped) = first.strip_suffix('\r') {
            Some((stripped, "\r\n", rest))
        } else {
            Some((first, "\n", rest))
        }
    } else {
        Some((raw.strip_suffix('\r').unwrap_or(raw), "", ""))
    }
}

fn rollout_has_user_event(raw: &str) -> bool {
    raw.contains("\"role\":\"user\"")
        || raw.contains("\"role\": \"user\"")
        || raw.contains("\"type\":\"user_message\"")
        || raw.contains("\"type\": \"user_message\"")
}

fn increment_provider_count(counts: &mut ProviderScopeCounts, scope: RolloutScope, provider: &str) {
    let bucket = match scope {
        RolloutScope::Sessions => &mut counts.sessions,
        RolloutScope::ArchivedSessions => &mut counts.archived_sessions,
    };
    *bucket.entry(provider.to_string()).or_insert(0) += 1;
}

fn read_sqlite_provider_counts(codex_home: &Path) -> Result<Option<SqliteProviderCounts>, String> {
    let Some(path) = sqlite_path(codex_home) else {
        return Ok(None);
    };
    if !path.is_file() {
        return Ok(None);
    }
    let conn = open_sqlite_connection(codex_home, "读取 Codex 历史 provider")?;
    let Some(schema) = read_sqlite_schema(&conn)? else {
        return Ok(Some(SqliteProviderCounts::default()));
    };
    if !schema.has_model_provider {
        return Ok(Some(SqliteProviderCounts::default()));
    }

    let mut statement = conn
        .prepare("SELECT COALESCE(model_provider, '') AS provider, COUNT(*) FROM threads GROUP BY COALESCE(model_provider, '')")
        .map_err(|error| sqlite_error("读取 Codex 历史 provider 统计", error))?;
    let rows = statement
        .query_map([], |row| {
            let provider: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((provider, count))
        })
        .map_err(|error| sqlite_error("读取 Codex 历史 provider 统计", error))?;
    let mut counts = SqliteProviderCounts::default();
    for row in rows {
        let (provider, count) =
            row.map_err(|error| sqlite_error("读取 Codex 历史 provider 统计", error))?;
        let count = count.max(0) as u64;
        let provider = if provider.is_empty() {
            "<empty>".to_string()
        } else {
            provider
        };
        counts.total_threads += count;
        counts.threads.insert(provider, count);
    }
    Ok(Some(counts))
}

fn count_sqlite_threads_to_update(codex_home: &Path, provider: &str) -> Result<u64, String> {
    let Some(path) = sqlite_path(codex_home) else {
        return Ok(0);
    };
    if !path.is_file() {
        return Ok(0);
    }
    let conn = open_sqlite_connection(codex_home, "检查 Codex 历史 provider")?;
    let Some(schema) = read_sqlite_schema(&conn)? else {
        return Ok(0);
    };
    if !schema.has_model_provider {
        return Ok(0);
    }
    conn.query_row(
        "SELECT COUNT(*) FROM threads WHERE model_provider IS NULL OR model_provider != ?1",
        [provider],
        |row| {
            let count: i64 = row.get(0)?;
            Ok(count.max(0) as u64)
        },
    )
    .map_err(|error| sqlite_error("检查 Codex 历史 provider", error))
}

fn open_sqlite_connection(codex_home: &Path, action: &str) -> Result<Connection, String> {
    let path = sqlite_path(codex_home).ok_or_else(|| "无法解析 state_5.sqlite 路径".to_string())?;
    let conn = Connection::open(&path).map_err(|error| sqlite_error(action, error))?;
    conn.busy_timeout(Duration::from_millis(500))
        .map_err(|error| sqlite_error(action, error))?;
    Ok(conn)
}

fn read_sqlite_schema(conn: &Connection) -> Result<Option<SqliteSchema>, String> {
    let mut statement = conn
        .prepare("PRAGMA table_info(threads)")
        .map_err(|error| sqlite_error("读取 Codex 历史索引结构", error))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| sqlite_error("读取 Codex 历史索引结构", error))?;
    let mut columns = BTreeSet::new();
    for row in rows {
        columns.insert(row.map_err(|error| sqlite_error("读取 Codex 历史索引结构", error))?);
    }
    if columns.is_empty() {
        return Ok(None);
    }
    let id_column = if columns.contains("id") {
        "id".to_string()
    } else if columns.contains("thread_id") {
        "thread_id".to_string()
    } else {
        return Ok(None);
    };
    Ok(Some(SqliteSchema {
        id_column,
        has_model_provider: columns.contains("model_provider"),
        has_user_event: columns.contains("has_user_event"),
        has_cwd: columns.contains("cwd"),
    }))
}

fn apply_sqlite_provider_update(
    conn: &Connection,
    provider: &str,
    scan: &SessionScan,
) -> Result<(), String> {
    let Some(schema) = read_sqlite_schema(conn)? else {
        return Ok(());
    };
    if schema.has_model_provider {
        conn.execute(
            "UPDATE threads SET model_provider = ?1 WHERE model_provider IS NULL OR model_provider != ?1",
            [provider],
        )
        .map_err(|error| sqlite_error("更新 Codex 历史 provider", error))?;
    }

    let id_column = quote_sqlite_identifier(schema.id_column.as_str())?;
    if schema.has_user_event {
        let sql = format!("UPDATE threads SET has_user_event = 1 WHERE {id_column} = ?1");
        for thread_id in &scan.user_event_thread_ids {
            conn.execute(sql.as_str(), [thread_id.as_str()])
                .map_err(|error| sqlite_error("更新 Codex 历史 user event 标记", error))?;
        }
    }
    if schema.has_cwd {
        let sql = format!(
            "UPDATE threads SET cwd = ?1 WHERE {id_column} = ?2 AND (cwd IS NULL OR cwd = '')"
        );
        for (thread_id, cwd) in &scan.thread_cwd_by_id {
            conn.execute(sql.as_str(), [cwd.as_str(), thread_id.as_str()])
                .map_err(|error| sqlite_error("更新 Codex 历史 cwd", error))?;
        }
    }

    Ok(())
}

fn quote_sqlite_identifier(identifier: &str) -> Result<String, String> {
    if identifier
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        Ok(format!("\"{identifier}\""))
    } else {
        Err(format!("不支持的 SQLite 字段名: {identifier}"))
    }
}

fn sqlite_error(action: &str, error: rusqlite::Error) -> String {
    let message = error.to_string();
    let lowered = message.to_ascii_lowercase();
    if lowered.contains("locked") || lowered.contains("busy") {
        format!("{action}失败：state_5.sqlite 正在被 Codex 占用，请关闭 Codex/Codex App 后重试。原始错误: {message}")
    } else {
        format!("{action}失败: {message}")
    }
}

fn rollback_sqlite(conn: Option<&Connection>) {
    if let Some(conn) = conn {
        let _ = conn.execute_batch("ROLLBACK");
    }
}

fn create_backup(
    codex_home: &Path,
    provider: &str,
    changes: &[SessionFileChange],
    backup_sqlite: bool,
) -> Result<Option<BackupHandle>, String> {
    if changes.is_empty() && !backup_sqlite {
        return Ok(None);
    }

    let id = format!("{}-{}", utils::now_unix_seconds(), Uuid::new_v4());
    let path = backup_root(codex_home).join(&id);
    let rollout_backup_dir = path.join("rollouts");
    fs::create_dir_all(&rollout_backup_dir).map_err(|error| {
        format!(
            "创建 provider 同步备份目录失败 {}: {error}",
            rollout_backup_dir.display()
        )
    })?;
    let mut manifest = ProviderSyncBackupManifest {
        id: id.clone(),
        provider: provider.to_string(),
        created_at: utils::now_unix_seconds(),
        codex_home: codex_home.to_string_lossy().to_string(),
        rollout_files: Vec::new(),
        sqlite: None,
    };

    for (index, change) in changes.iter().enumerate() {
        let backup_path = rollout_backup_dir.join(format!("{index}.jsonl"));
        fs::copy(&change.path, &backup_path).map_err(|error| {
            format!("备份 Codex rollout 失败 {}: {error}", change.path.display())
        })?;
        manifest.rollout_files.push(BackupFileEntry {
            source_path: change.path.to_string_lossy().to_string(),
            backup_path: backup_path.to_string_lossy().to_string(),
            existed: true,
        });
    }

    if backup_sqlite {
        let source_path = codex_home.join(STATE_DB_FILE_NAME);
        let backup_path = path.join(STATE_DB_FILE_NAME);
        fs::copy(&source_path, &backup_path).map_err(|error| {
            format!("备份 Codex 历史索引失败 {}: {error}", source_path.display())
        })?;
        manifest.sqlite = Some(BackupFileEntry {
            source_path: source_path.to_string_lossy().to_string(),
            backup_path: backup_path.to_string_lossy().to_string(),
            existed: true,
        });
    }

    let manifest_path = path.join(MANIFEST_FILE_NAME);
    let manifest_text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("序列化 provider 同步备份清单失败: {error}"))?;
    write_bytes_atomically(&manifest_path, manifest_text.as_bytes())?;
    Ok(Some(BackupHandle { id, path, manifest }))
}

fn restore_written_rollouts(backup: Option<&BackupHandle>, written_paths: &[PathBuf]) {
    let Some(backup) = backup else {
        return;
    };
    for entry in &backup.manifest.rollout_files {
        let source_path = PathBuf::from(&entry.source_path);
        if written_paths.iter().any(|path| path == &source_path) {
            let _ = restore_backup_entry(entry);
        }
    }
}

fn restore_backup_entry(entry: &BackupFileEntry) -> Result<(), String> {
    let source_path = PathBuf::from(&entry.source_path);
    if entry.existed {
        let backup = fs::read(&entry.backup_path).map_err(|error| {
            format!("读取 provider 同步备份失败 {}: {error}", entry.backup_path)
        })?;
        write_bytes_atomically(&source_path, &backup)
    } else {
        match fs::remove_file(&source_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "移除 provider 同步恢复目标失败 {}: {error}",
                source_path.display()
            )),
        }
    }
}

fn backup_root(codex_home: &Path) -> PathBuf {
    codex_home.join(BACKUP_ROOT_NAME)
}

fn get_backup_summary(codex_home: &Path) -> Result<BackupSummary, String> {
    let backup_dir = backup_root(codex_home);
    let backup_ids = list_backup_ids(codex_home)?;
    Ok(BackupSummary {
        available: !backup_ids.is_empty(),
        backup_count: backup_ids.len(),
        latest_backup_id: backup_ids.iter().max().cloned(),
        backup_dir: backup_dir.to_string_lossy().to_string(),
    })
}

fn latest_backup_id(codex_home: &Path) -> Result<Option<String>, String> {
    Ok(list_backup_ids(codex_home)?.into_iter().max())
}

fn list_backup_ids(codex_home: &Path) -> Result<Vec<String>, String> {
    let root = backup_root(codex_home);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for entry in fs::read_dir(&root)
        .map_err(|error| format!("读取 provider 同步备份目录失败 {}: {error}", root.display()))?
    {
        let entry = entry.map_err(|error| format!("读取 provider 同步备份条目失败: {error}"))?;
        let path = entry.path();
        if path.is_dir() && path.join(MANIFEST_FILE_NAME).is_file() {
            if let Some(id) = path.file_name().and_then(|value| value.to_str()) {
                ids.push(id.to_string());
            }
        }
    }
    Ok(ids)
}

fn read_backup_manifest(
    codex_home: &Path,
    backup_id: &str,
) -> Result<ProviderSyncBackupManifest, String> {
    let path = backup_root(codex_home)
        .join(backup_id)
        .join(MANIFEST_FILE_NAME);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("读取 provider 同步备份清单失败 {}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("解析 provider 同步备份清单失败: {error}"))
}

fn write_bytes_atomically(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析目标目录 {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("创建目标目录失败 {}: {error}", parent.display()))?;

    let temp_path = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("provider-sync"),
        Uuid::new_v4()
    ));

    let write_result = (|| -> Result<(), String> {
        let mut temp_file = utils::private_create_new_options()
            .open(&temp_path)
            .map_err(|error| format!("创建临时文件失败 {}: {error}", temp_path.display()))?;
        temp_file
            .write_all(contents)
            .map_err(|error| format!("写入临时文件失败 {}: {error}", temp_path.display()))?;
        temp_file
            .sync_all()
            .map_err(|error| format!("刷新临时文件失败 {}: {error}", temp_path.display()))?;
        drop(temp_file);
        let _ = utils::set_private_permissions(&temp_path);
        replace_file(&temp_path, path)?;
        let _ = utils::set_private_permissions(path);
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn replace_file(temp_path: &Path, target_path: &Path) -> Result<(), String> {
    #[cfg(target_family = "unix")]
    {
        fs::rename(temp_path, target_path).map_err(|error| {
            format!(
                "替换目标文件失败 {} -> {}: {error}",
                temp_path.display(),
                target_path.display()
            )
        })?;
        if let Some(parent) = target_path.parent() {
            if let Ok(parent_dir) = fs::File::open(parent) {
                let _ = parent_dir.sync_all();
            }
        }
        Ok(())
    }

    #[cfg(not(target_family = "unix"))]
    {
        match fs::rename(temp_path, target_path) {
            Ok(()) => Ok(()),
            Err(first_error) => {
                let restore_path = target_path.with_extension(format!(
                    "{}.provider-sync-restore",
                    target_path
                        .extension()
                        .and_then(|value| value.to_str())
                        .unwrap_or("bak")
                ));
                if target_path.exists() {
                    fs::rename(target_path, &restore_path).map_err(|restore_error| {
                        format!(
                            "替换目标文件失败 {} -> {}: {first_error}; 备份旧文件也失败: {restore_error}",
                            temp_path.display(),
                            target_path.display()
                        )
                    })?;
                }
                match fs::rename(temp_path, target_path) {
                    Ok(()) => {
                        let _ = fs::remove_file(&restore_path);
                        Ok(())
                    }
                    Err(second_error) => {
                        if restore_path.exists() {
                            let _ = fs::rename(&restore_path, target_path);
                        }
                        Err(format!(
                            "替换目标文件失败 {} -> {}: {second_error}",
                            temp_path.display(),
                            target_path.display()
                        ))
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sync_provider;
    use rusqlite::Connection;
    use serde_json::Value;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn unique_test_dir(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "codex-tools-provider-sync-{name}-{}",
            Uuid::new_v4()
        ))
    }

    #[test]
    fn sync_provider_rewrites_rollouts_and_sqlite_with_backup() {
        let sandbox = unique_test_dir("sync");
        let codex_home = sandbox.join("codex");
        let sessions = codex_home
            .join("sessions")
            .join("2026")
            .join("07")
            .join("08");
        fs::create_dir_all(&sessions).expect("create sessions");
        let rollout_path = sessions.join("rollout-test.jsonl");
        fs::write(
            &rollout_path,
            concat!(
                "{\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\",\"cwd\":\"/tmp/project\",\"model_provider\":\"other\"}}\n",
                "{\"type\":\"message\",\"role\":\"user\",\"content\":\"hi\"}\n"
            ),
        )
        .expect("write rollout");

        let state_path = codex_home.join("state_5.sqlite");
        let conn = Connection::open(&state_path).expect("open sqlite");
        conn.execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, model_provider TEXT, has_user_event INTEGER, cwd TEXT)",
            [],
        )
        .expect("create threads");
        conn.execute(
            "INSERT INTO threads (id, model_provider, has_user_event, cwd) VALUES ('thread-1', 'other', 0, '')",
            [],
        )
        .expect("insert thread");
        conn.execute(
            "INSERT INTO threads (id, model_provider, has_user_event, cwd) VALUES ('db-only', 'other', 0, '')",
            [],
        )
        .expect("insert db-only thread");
        drop(conn);

        let result = sync_provider(Some(&codex_home), "openai").expect("sync provider");
        assert_eq!(result.rollout_files_changed, 1);
        assert_eq!(result.sqlite_threads_changed, 2);
        assert!(result.backup_id.is_some());

        let first_line = fs::read_to_string(&rollout_path)
            .expect("read rollout")
            .lines()
            .next()
            .expect("first line")
            .to_string();
        let root: Value = serde_json::from_str(&first_line).expect("parse rollout");
        assert_eq!(
            root.pointer("/payload/model_provider")
                .and_then(Value::as_str),
            Some("openai")
        );

        let conn = Connection::open(&state_path).expect("reopen sqlite");
        let provider: String = conn
            .query_row(
                "SELECT model_provider FROM threads WHERE id = 'thread-1'",
                [],
                |row| row.get(0),
            )
            .expect("read provider");
        let has_user_event: i64 = conn
            .query_row(
                "SELECT has_user_event FROM threads WHERE id = 'thread-1'",
                [],
                |row| row.get(0),
            )
            .expect("read user flag");
        let cwd: String = conn
            .query_row("SELECT cwd FROM threads WHERE id = 'thread-1'", [], |row| {
                row.get(0)
            })
            .expect("read cwd");
        let db_only_provider: String = conn
            .query_row(
                "SELECT model_provider FROM threads WHERE id = 'db-only'",
                [],
                |row| row.get(0),
            )
            .expect("read db-only provider");
        assert_eq!(provider, "openai");
        assert_eq!(has_user_event, 1);
        assert_eq!(cwd, "/tmp/project");
        assert_eq!(db_only_provider, "openai");

        fs::remove_dir_all(sandbox).expect("cleanup");
    }

    #[test]
    fn explicit_sync_rejects_unknown_provider() {
        let sandbox = unique_test_dir("unknown");
        let codex_home = sandbox.join("codex");
        fs::create_dir_all(&codex_home).expect("create codex home");

        let error = sync_provider(Some(&codex_home), "other").expect_err("unknown provider");
        assert!(error.contains("没有配置 provider"));

        fs::remove_dir_all(sandbox).expect("cleanup");
    }
}
