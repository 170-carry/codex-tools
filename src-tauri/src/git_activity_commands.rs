use crate::{
    app_paths,
    git_activity::{self, Config, Report},
};
use serde::Serialize;
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Emitter};

static RUNNING: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);
struct RunningGuard;
impl Drop for RunningGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

#[derive(Serialize)]
pub struct SavedActivity {
    config: Config,
    report: Option<Report>,
}

fn save(path: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temp, data).map_err(|e| e.to_string())?;
    // Windows rename cannot replace an existing destination; retain a recoverable backup.
    let backup = path.with_extension("previous.json");
    if path.exists() {
        fs::copy(path, &backup).map_err(|e| e.to_string())?;
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    if let Err(error) = fs::rename(&temp, path) {
        if backup.exists() {
            let _ = fs::copy(&backup, path);
        }
        return Err(error.to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn get_git_activity(app: AppHandle) -> Result<SavedActivity, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = app_paths::app_data_dir(&app)?.join("git-activity");
        let config = if root.join("config.json").exists() {
            serde_json::from_slice(&fs::read(root.join("config.json")).map_err(|e| e.to_string())?)
                .map_err(|e| format!("统计配置读取失败：{e}"))?
        } else {
            Config::default()
        };
        let report_path = root.join("report.json");
        let report = if report_path.exists() {
            Some(
                serde_json::from_slice(&fs::read(report_path).map_err(|e| e.to_string())?)
                    .map_err(|e| format!("统计快照读取失败：{e}"))?,
            )
        } else {
            None
        };
        Ok(SavedActivity { config, report })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn select_git_activity_folder() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        Ok(rfd::FileDialog::new()
            .set_title("选择 Git 仓库或包含多个仓库的文件夹")
            .pick_folder()
            .map(|p| p.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn import_git_activity_config() -> Result<Option<Config>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let Some(path) = rfd::FileDialog::new()
            .set_title("导入代码统计配置")
            .add_filter("JSON", &["json"])
            .pick_file()
        else {
            return Ok(None);
        };
        let config: Config = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("配置格式错误：{e}"))?;
        config.validate()?;
        Ok(Some(config))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn run_git_activity(app: AppHandle, config: Config) -> Result<Report, String> {
    config.validate()?;
    if RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("已有代码统计正在运行，请等待或取消后再试".into());
    }
    CANCEL.store(false, Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = RunningGuard;
        let root = app_paths::app_data_dir(&app)?.join("git-activity");
        save(
            &root.join("config.json"),
            &serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?,
        )?;
        let report =
            git_activity::collect(config, &root.join("repositories"), &CANCEL, |message| {
                let _ = app.emit("git-activity-progress", message);
            })?;
        if CANCEL.load(Ordering::SeqCst) {
            return Err("已取消统计，原报告保留".into());
        }
        save(
            &root.join("report.json"),
            &serde_json::to_vec(&report).map_err(|e| e.to_string())?,
        )?;
        Ok(report)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cancel_git_activity() {
    CANCEL.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub async fn export_git_activity_html(html: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_title("保存每日提交日历")
            .set_file_name("每日提交日历.html")
            .add_filter("HTML", &["html"])
            .save_file()
        else {
            return Ok(None);
        };
        fs::write(&path, html).map_err(|e| e.to_string())?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| e.to_string())?
}
