mod cookies;
pub use cookies::session_header;
mod process;
mod workspaces;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

const AUTH_KEYS: &[&str] = &[
    "lastKnownAccountUuid",
    "oauth:tokenCache",
    "oauth:tokenCacheV2",
    "oauth:staleSessionLatch",
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub account_uuid: String,
    pub org_uuid: String,
    pub auth_config: Map<String, Value>,
    pub cookies: cookies::Cookies,
}

pub fn data_dir() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("无法找到应用配置目录")?
        .join("Claude"))
}

pub fn capture(root: &Path) -> Result<Snapshot, String> {
    let config: Value = serde_json::from_slice(
        &fs::read(root.join("config.json"))
            .map_err(|e| format!("请先登录 Claude 桌面 App: {e}"))?,
    )
    .map_err(|e| e.to_string())?;
    let account_uuid = config["lastKnownAccountUuid"]
        .as_str()
        .ok_or("Claude 桌面 App 尚未登录")?
        .to_string();
    let cookies = cookies::read(&root.join("Cookies"))?;
    if !cookies::has_session(&cookies) {
        return Err("Claude 桌面 App 未找到登录会话，请先登录".into());
    }
    let org_uuid = match cookies::active_org(&cookies)? {
        Some(org) => org,
        None => {
            let root = root.join("claude-code-sessions").join(&account_uuid);
            let orgs = fs::read_dir(root)
                .map_err(|_| "请先在 Claude 桌面 App 打开一个 Code 工作区")?
                .filter_map(Result::ok)
                .filter(|e| e.path().is_dir())
                .collect::<Vec<_>>();
            if orgs.len() != 1 {
                return Err("无法确定 Claude 桌面 App 当前组织".into());
            }
            orgs[0].file_name().to_string_lossy().to_string()
        }
    };
    Ok(Snapshot {
        account_uuid,
        org_uuid,
        cookies,
        auth_config: AUTH_KEYS
            .iter()
            .filter_map(|key| config.get(*key).map(|v| (key.to_string(), v.clone())))
            .collect(),
    })
}

fn restore(root: &Path, snapshot: &Snapshot) -> Result<(), String> {
    let path = root.join("config.json");
    let mut config: Value = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let object = config.as_object_mut().ok_or("Claude 桌面配置格式错误")?;
    for key in AUTH_KEYS {
        object.remove(*key);
    }
    object.extend(snapshot.auth_config.clone());
    cookies::restore(&root.join("Cookies"), &snapshot.cookies)?;
    crate::claude::store::write_json(&path, &config)
}

pub fn switch(root: &Path, backup: &Path, target: &Snapshot) -> Result<Snapshot, String> {
    let executable = process::stop()?;
    let result = (|| {
        let source = capture(root)?;
        crate::claude::store::write_json(&backup.join("login.json"), &source)?;
        let preferences = root.join("claude_desktop_config.json");
        if preferences.exists() {
            fs::create_dir_all(backup).map_err(|e| e.to_string())?;
            fs::copy(preferences, backup.join("claude_desktop_config.json"))
                .map_err(|e| e.to_string())?;
        }
        crate::utils::set_private_permissions(&backup.join("claude_desktop_config.json"));
        workspaces::preserve(root, &source, target)?;
        if let Err(error) = restore(root, target) {
            let rollback = restore(root, &source);
            return Err(format!(
                "{error}；{}",
                rollback
                    .err()
                    .map(|e| format!("恢复原登录失败: {e}"))
                    .unwrap_or_else(|| "已恢复原登录".into())
            ));
        }
        Ok(source)
    })();
    let launch = process::launch(executable);
    match (result, launch) {
        (Ok(source), Ok(())) => Ok(source),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}
