use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use uuid::Uuid;

#[cfg(feature = "desktop")]
use tauri::AppHandle;
#[cfg(feature = "desktop")]
use tauri::Manager;

use crate::auth::account_variant_key;
use crate::auth::extract_auth;
use crate::auth::read_current_codex_auth_optional;
use crate::models::dedupe_account_variants;
use crate::models::AccountsStore;
use crate::models::StoredAccount;
use crate::utils::now_unix_seconds;
use crate::utils::set_private_permissions;
use crate::utils::short_account;

const LAST_GOOD_BACKUP_FILE_NAME: &str = "accounts.json.last-good.json";
const PREVIOUS_GOOD_BACKUP_FILE_NAME: &str = "accounts.json.prev-good.json";

#[derive(Clone)]
struct RecoveryCandidate {
    source: String,
    modified_at: i64,
    store: AccountsStore,
}

#[cfg(feature = "desktop")]
pub(crate) fn load_store(app: &AppHandle) -> Result<AccountsStore, String> {
    load_store_from_path(&account_store_path(app)?)
}

#[cfg(feature = "desktop")]
pub(crate) fn save_store(app: &AppHandle, store: &AccountsStore) -> Result<(), String> {
    save_store_to_path(&account_store_path(app)?, store)
}

/// 启动时自动同步当前登录账号：
/// 若本机已有 `~/.codex/auth.json` 且相同“账号 + 套餐态”不在列表中，则自动写入存储。
#[cfg(feature = "desktop")]
pub(crate) fn sync_current_auth_account_on_startup(app: &AppHandle) -> Result<(), String> {
    sync_current_auth_account_on_startup_in_path(&account_store_path(app)?)
}

pub(crate) fn load_store_from_path(path: &Path) -> Result<AccountsStore, String> {
    if !path.exists() {
        return Ok(AccountsStore::default());
    }

    let raw = fs::read_to_string(path)
        .map_err(|e| format!("读取账号存储文件失败 {}: {e}", path.display()))?;

    match serde_json::from_str::<AccountsStore>(&raw) {
        Ok(store) => Ok(normalize_loaded_store(path, store)),
        Err(primary_err) => {
            if let Some((recovered, recovered_sources)) =
                recover_store_from_available_sources(path, &raw)
            {
                log::warn!(
                    "账号存储文件格式无效，已从可恢复数据重建 {}: {}; 来源: {}",
                    path.display(),
                    primary_err,
                    recovered_sources.join(", ")
                );
                if let Err(backup_err) = backup_corrupted_store_file(path, &raw) {
                    log::warn!(
                        "重建前备份损坏账号存储文件失败 {}: {}",
                        path.display(),
                        backup_err
                    );
                }
                if let Err(repair_err) = write_store_file(path, &recovered) {
                    return Err(format!(
                        "账号存储文件恢复后重写失败 {}: {}; {}",
                        path.display(),
                        primary_err,
                        repair_err
                    ));
                }
                return Ok(normalize_loaded_store(path, recovered));
            }

            if let Err(backup_err) = backup_corrupted_store_file(path, &raw) {
                log::warn!(
                    "账号存储文件损坏，备份失败 {}: {}",
                    path.display(),
                    backup_err
                );
            }

            let fallback = AccountsStore::default();
            if let Err(repair_err) = write_store_file(path, &fallback) {
                return Err(format!(
                    "账号存储文件格式无效且修复失败 {}: {}; {}",
                    path.display(),
                    primary_err,
                    repair_err
                ));
            }

            log::warn!(
                "账号存储文件格式无效，已重建默认存储 {}: {}",
                path.display(),
                primary_err
            );
            Ok(normalize_loaded_store(path, fallback))
        }
    }
}

pub(crate) fn save_store_to_path(path: &Path, store: &AccountsStore) -> Result<(), String> {
    write_store_file(path, store)
}

pub(crate) fn sync_current_auth_account_on_startup_in_path(path: &Path) -> Result<(), String> {
    let auth_json = match read_current_codex_auth_optional()? {
        Some(value) => value,
        None => return Ok(()),
    };

    let extracted = match extract_auth(&auth_json) {
        Ok(value) => value,
        Err(err) => {
            log::warn!("跳过启动自动导入当前账号: {err}");
            return Ok(());
        }
    };

    let mut store = load_store_from_path(path)?;
    let extracted_variant_key = account_variant_key(
        &extracted.principal_id,
        &extracted.account_id,
        extracted.plan_type.as_deref(),
    );
    let already_exists = store
        .accounts
        .iter()
        .any(|account| account.variant_key() == extracted_variant_key);
    if already_exists {
        return Ok(());
    }

    let now = now_unix_seconds();
    let label = extracted
        .email
        .clone()
        .unwrap_or_else(|| format!("Codex {}", short_account(&extracted.account_id)));

    let stored = StoredAccount {
        id: Uuid::new_v4().to_string(),
        label,
        principal_id: Some(extracted.principal_id),
        email: extracted.email,
        account_id: extracted.account_id,
        plan_type: extracted.plan_type,
        auth_json,
        added_at: now,
        updated_at: now,
        usage: None,
        usage_error: None,
    };
    store.accounts.push(stored);
    save_store_to_path(path, &store)?;
    Ok(())
}

#[cfg(feature = "desktop")]
fn account_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {e}"))?;
    Ok(account_store_path_from_data_dir(&dir))
}

pub(crate) fn account_store_path_from_data_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("accounts.json")
}

fn write_store_file(path: &Path, store: &AccountsStore) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析存储目录 {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("创建存储目录失败 {}: {e}", parent.display()))?;

    let serialized =
        serde_json::to_string_pretty(store).map_err(|e| format!("序列化账号存储失败: {e}"))?;
    write_file_atomically(path, serialized.as_bytes())?;
    if let Err(err) = write_store_shadow_backups(path, serialized.as_bytes()) {
        log::warn!("写入账号存储滚动备份失败 {}: {}", path.display(), err);
    }
    Ok(())
}

fn normalize_loaded_store(path: &Path, mut store: AccountsStore) -> AccountsStore {
    let mut changed = false;

    for account in &mut store.accounts {
        if account
            .principal_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            account.principal_id = Some(account.principal_key());
            changed = true;
        }
    }

    if dedupe_account_variants(&mut store.accounts) {
        log::warn!("账号存储存在重复账号变体，已自动合并 {}", path.display());
        changed = true;
    }

    if changed {
        if let Err(repair_err) = write_store_file(path, &store) {
            log::warn!(
                "修正账号存储后重写文件失败 {}: {}",
                path.display(),
                repair_err
            );
        }
    }

    store
}

fn write_file_atomically(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析存储目录 {}", path.display()))?;
    let temp_path = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("accounts.json"),
        Uuid::new_v4()
    ));

    let write_result = (|| -> Result<(), String> {
        let mut temp_file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|e| format!("创建临时存储文件失败 {}: {e}", temp_path.display()))?;
        temp_file
            .write_all(contents)
            .map_err(|e| format!("写入临时存储文件失败 {}: {e}", temp_path.display()))?;
        temp_file
            .sync_all()
            .map_err(|e| format!("刷新临时存储文件失败 {}: {e}", temp_path.display()))?;
        drop(temp_file);
        set_private_permissions(&temp_path);

        #[cfg(target_family = "unix")]
        {
            fs::rename(&temp_path, path).map_err(|e| {
                format!(
                    "替换账号存储文件失败 {} -> {}: {e}",
                    temp_path.display(),
                    path.display()
                )
            })?;

            let parent_dir = fs::File::open(parent)
                .map_err(|e| format!("打开存储目录失败 {}: {e}", parent.display()))?;
            parent_dir
                .sync_all()
                .map_err(|e| format!("刷新存储目录失败 {}: {e}", parent.display()))?;
        }

        #[cfg(not(target_family = "unix"))]
        {
            if path.exists() {
                fs::remove_file(path)
                    .map_err(|e| format!("移除旧账号存储文件失败 {}: {e}", path.display()))?;
            }
            fs::rename(&temp_path, path).map_err(|e| {
                format!(
                    "替换账号存储文件失败 {} -> {}: {e}",
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

fn write_store_shadow_backups(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析存储目录 {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("创建存储目录失败 {}: {e}", parent.display()))?;

    let latest_backup = parent.join(LAST_GOOD_BACKUP_FILE_NAME);
    let previous_backup = parent.join(PREVIOUS_GOOD_BACKUP_FILE_NAME);

    if latest_backup.exists() {
        let latest_contents = fs::read(&latest_backup)
            .map_err(|e| format!("读取最新备份失败 {}: {e}", latest_backup.display()))?;
        fs::write(&previous_backup, latest_contents)
            .map_err(|e| format!("写入上一个备份失败 {}: {e}", previous_backup.display()))?;
        set_private_permissions(&previous_backup);
    }

    fs::write(&latest_backup, contents)
        .map_err(|e| format!("写入最新备份失败 {}: {e}", latest_backup.display()))?;
    set_private_permissions(&latest_backup);
    Ok(())
}

fn recover_store_from_available_sources(
    path: &Path,
    raw: &str,
) -> Option<(AccountsStore, Vec<String>)> {
    let candidates = collect_recovery_candidates(path, raw);
    if candidates.is_empty() {
        return None;
    }

    let best = candidates.iter().max_by_key(|candidate| {
        (
            usize::from(!candidate.store.accounts.is_empty()),
            candidate.store.accounts.len(),
            candidate.modified_at,
        )
    })?;

    let mut merged_accounts = Vec::new();
    let mut recovered_sources = Vec::new();
    for candidate in &candidates {
        if !candidate.store.accounts.is_empty() {
            recovered_sources.push(candidate.source.clone());
        }
        merged_accounts.extend(candidate.store.accounts.clone());
    }
    dedupe_account_variants(&mut merged_accounts);

    if merged_accounts.is_empty() {
        return None;
    }

    let mut recovered = best.store.clone();
    recovered.accounts = merged_accounts;
    Some((recovered, recovered_sources))
}

fn collect_recovery_candidates(path: &Path, raw: &str) -> Vec<RecoveryCandidate> {
    let mut candidates = parse_store_candidates_from_text(
        raw,
        format!("{} (current damaged file)", path.display()),
        file_modified_at(path),
    );

    let Some(parent) = path.parent() else {
        return candidates;
    };

    let Ok(entries) = fs::read_dir(parent) else {
        return candidates;
    };

    for entry in entries.flatten() {
        let candidate_path = entry.path();
        if candidate_path == path || !candidate_path.is_file() {
            continue;
        }
        if !is_store_backup_candidate(&candidate_path) {
            continue;
        }

        let Ok(candidate_raw) = fs::read_to_string(&candidate_path) else {
            continue;
        };
        candidates.extend(parse_store_candidates_from_text(
            &candidate_raw,
            candidate_path.display().to_string(),
            file_modified_at(&candidate_path),
        ));
    }

    candidates
}

fn parse_store_candidates_from_text(
    raw: &str,
    source: String,
    modified_at: i64,
) -> Vec<RecoveryCandidate> {
    let mut candidates = Vec::new();

    if let Ok(store) = serde_json::from_str::<AccountsStore>(raw) {
        candidates.push(RecoveryCandidate {
            source,
            modified_at,
            store,
        });
        return candidates;
    }

    let mut stream = serde_json::Deserializer::from_str(raw).into_iter::<AccountsStore>();
    let mut recovered_index = 0usize;
    while let Some(result) = stream.next() {
        match result {
            Ok(store) => {
                recovered_index += 1;
                candidates.push(RecoveryCandidate {
                    source: format!("{source}#{recovered_index}"),
                    modified_at,
                    store,
                });
            }
            Err(_) => break,
        }
    }

    if candidates.is_empty() {
        if let Ok(accounts) = serde_json::from_str::<Vec<StoredAccount>>(raw) {
            candidates.push(RecoveryCandidate {
                source,
                modified_at,
                store: AccountsStore {
                    version: 1,
                    accounts,
                    settings: Default::default(),
                },
            });
        }
    }

    candidates
}

fn is_store_backup_candidate(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    name.starts_with("accounts.")
        || name.starts_with("accounts.json.")
        || name.starts_with(".accounts.json.tmp-")
}

fn file_modified_at(path: &Path) -> i64 {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_secs() as i64)
        })
        .unwrap_or_default()
}

fn backup_corrupted_store_file(path: &Path, raw: &str) -> Result<PathBuf, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析存储目录 {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("创建存储目录失败 {}: {e}", parent.display()))?;

    let backup_path = parent.join(format!("accounts.corrupt-{}.json", now_unix_seconds()));
    fs::write(&backup_path, raw)
        .map_err(|e| format!("写入损坏备份文件失败 {}: {e}", backup_path.display()))?;
    set_private_permissions(&backup_path);
    Ok(backup_path)
}

#[cfg(test)]
mod tests {
    use super::load_store_from_path;
    use super::save_store_to_path;
    use super::LAST_GOOD_BACKUP_FILE_NAME;
    use super::PREVIOUS_GOOD_BACKUP_FILE_NAME;
    use crate::models::AccountsStore;
    use crate::models::StoredAccount;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("codex-tools-store-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn sample_store(label: &str, account_id: &str, updated_at: i64) -> AccountsStore {
        AccountsStore {
            version: 1,
            accounts: vec![StoredAccount {
                id: format!("id-{label}"),
                label: label.to_string(),
                principal_id: Some(format!("{label}@example.com")),
                email: Some(format!("{label}@example.com")),
                account_id: account_id.to_string(),
                plan_type: Some("team".to_string()),
                auth_json: json!({ "kind": label }),
                added_at: updated_at - 1,
                updated_at,
                usage: None,
                usage_error: None,
            }],
            settings: Default::default(),
        }
    }

    #[test]
    fn load_store_recovers_from_backup_candidates_instead_of_resetting() {
        let dir = temp_dir();
        let store_path = dir.join("accounts.json");
        fs::write(&store_path, "{not valid json").expect("write damaged store");

        let backup_path = dir.join("accounts.json.manual-backup-1");
        let backup_store = sample_store("restored", "workspace-1", 10);
        fs::write(
            &backup_path,
            serde_json::to_string_pretty(&backup_store).expect("serialize backup"),
        )
        .expect("write backup");

        let loaded = load_store_from_path(&store_path).expect("recover store");

        assert_eq!(loaded.accounts.len(), 1);
        assert_eq!(loaded.accounts[0].label, "restored");

        let persisted: AccountsStore =
            serde_json::from_str(&fs::read_to_string(&store_path).expect("read repaired store"))
                .expect("parse repaired store");
        assert_eq!(persisted.accounts.len(), 1);
        assert_eq!(persisted.accounts[0].label, "restored");
    }

    #[test]
    fn save_store_writes_rolling_good_backups() {
        let dir = temp_dir();
        let store_path = dir.join("accounts.json");

        let first = sample_store("first", "workspace-1", 10);
        save_store_to_path(&store_path, &first).expect("save first");

        let latest_backup = dir.join(LAST_GOOD_BACKUP_FILE_NAME);
        assert!(latest_backup.exists());

        let second = sample_store("second", "workspace-2", 20);
        save_store_to_path(&store_path, &second).expect("save second");

        let previous_backup = dir.join(PREVIOUS_GOOD_BACKUP_FILE_NAME);
        assert!(previous_backup.exists());

        let previous: AccountsStore =
            serde_json::from_str(&fs::read_to_string(&previous_backup).expect("read previous"))
                .expect("parse previous");
        let latest: AccountsStore =
            serde_json::from_str(&fs::read_to_string(&latest_backup).expect("read latest"))
                .expect("parse latest");

        assert_eq!(previous.accounts[0].label, "first");
        assert_eq!(latest.accounts[0].label, "second");
    }

    #[test]
    fn load_store_backfills_missing_principal_id() {
        let dir = temp_dir();
        let store_path = dir.join("accounts.json");
        let legacy_store = AccountsStore {
            version: 1,
            accounts: vec![StoredAccount {
                id: "legacy".to_string(),
                label: "legacy".to_string(),
                principal_id: None,
                email: Some("legacy@example.com".to_string()),
                account_id: "workspace-1".to_string(),
                plan_type: Some("team".to_string()),
                auth_json: json!({ "kind": "legacy" }),
                added_at: 1,
                updated_at: 1,
                usage: None,
                usage_error: None,
            }],
            settings: Default::default(),
        };
        fs::write(
            &store_path,
            serde_json::to_string_pretty(&legacy_store).expect("serialize legacy store"),
        )
        .expect("write legacy store");

        let loaded = load_store_from_path(&store_path).expect("load legacy store");

        assert_eq!(
            loaded.accounts[0].principal_id.as_deref(),
            Some("legacy@example.com")
        );
    }
}
