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
use crate::models::AccountsStore;
use crate::models::StoredAccount;
use crate::utils::now_unix_seconds;
use crate::utils::set_private_permissions;
use crate::utils::short_account;

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
        Ok(store) => Ok(store),
        Err(primary_err) => {
            let mut stream = serde_json::Deserializer::from_str(&raw).into_iter::<AccountsStore>();
            if let Some(Ok(recovered)) = stream.next() {
                log::warn!(
                    "账号存储文件存在尾随内容，已自动恢复首个 JSON 对象 {}: {}",
                    path.display(),
                    primary_err
                );
                if let Err(repair_err) = write_store_file(path, &recovered) {
                    log::warn!(
                        "恢复后重写账号存储文件失败 {}: {}",
                        path.display(),
                        repair_err
                    );
                }
                return Ok(recovered);
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
            Ok(fallback)
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
    let extracted_variant_key =
        account_variant_key(&extracted.account_id, extracted.plan_type.as_deref());
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
    Ok(())
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
