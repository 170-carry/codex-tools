use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::Path};
use tauri::AppHandle;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    #[serde(default)]
    pub sync: super::sync::SyncStatus,
    pub id: String,
    pub label: String,
    pub email: Option<String>,
    pub credentials: Value,
    pub api_base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub usage: Option<Value>,
    #[serde(default)]
    pub desktop: Option<super::desktop::Snapshot>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub accounts: Vec<Account>,
    pub current_id: Option<String>,
    #[serde(default)]
    pub desktop_current_id: Option<String>,
}

pub fn read(app: &AppHandle) -> Result<Store, String> {
    let path = crate::app_paths::app_data_dir(app)?.join("claude-accounts.json");
    match fs::read(&path) {
        Ok(data) => serde_json::from_slice(&data).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Store::default()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    fs::create_dir_all(path.parent().ok_or("无效路径")?).map_err(|e| e.to_string())?;
    crate::auth::write_auth_file_atomically(
        path,
        &serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
}

pub fn save(app: &AppHandle, store: &Store) -> Result<(), String> {
    write_json(
        &crate::app_paths::app_data_dir(app)?.join("claude-accounts.json"),
        store,
    )
}

pub fn import(value: Value, label: Option<String>) -> Result<Account, String> {
    let credentials = if value.get("claudeAiOauth").is_some() {
        value["claudeAiOauth"].clone()
    } else if value.get("accessToken").is_some() {
        value.clone()
    } else {
        super::oauth::credentials(&value, None)?
    };
    if credentials
        .get("accessToken")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .is_none()
    {
        return Err("文件未包含 Claude accessToken".into());
    }
    let email = value
        .pointer("/account/email_address")
        .or_else(|| value.get("email"))
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(Account {
        sync: Default::default(),
        id: uuid::Uuid::new_v4().to_string(),
        label: label
            .filter(|s| !s.trim().is_empty())
            .or_else(|| email.clone())
            .unwrap_or_else(|| "Claude".into()),
        email,
        credentials,
        api_base_url: None,
        api_key: None,
        model: None,
        usage: None,
        desktop: None,
    })
}

pub fn upsert(
    store: &mut Store,
    mut account: Account,
    replace_id: Option<&str>,
) -> Result<(), String> {
    if let Some(id) = replace_id {
        let old = store
            .accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or("Claude 账号不存在")?;
        if old.email.is_some() && account.email.is_some() && old.email != account.email {
            return Err("授权邮箱与原 Claude 账号不一致".into());
        }
        account.id = old.id.clone();
        account.label = old.label.clone();
        account.desktop = account.desktop.or_else(|| old.desktop.clone());
        account.usage = account.usage.or_else(|| old.usage.clone());
        *old = account;
    } else if let Some(old) = store.accounts.iter_mut().find(|a| {
        (account.credentials["accountUuid"].is_string()
            && a.credentials["accountUuid"] == account.credentials["accountUuid"]
            && a.credentials["orgUuid"] == account.credentials["orgUuid"])
            || (a.email == account.email
                && account.email.is_some()
                && !a.credentials["orgUuid"].is_string()
                && !account.credentials["orgUuid"].is_string())
            || a.credentials["accessToken"]
                .as_str()
                .is_some_and(|v| Some(v) == account.credentials["accessToken"].as_str())
    }) {
        account.id = old.id.clone();
        account.desktop = account.desktop.or_else(|| old.desktop.clone());
        account.usage = account.usage.or_else(|| old.usage.clone());
        *old = account;
    } else {
        store.accounts.push(account);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn reauthorization_keeps_desktop_snapshot_and_account_id() {
        let old = Account { id:"saved".into(), label:"Work".into(), email:Some("a@example.com".into()), credentials:json!({}), desktop:Some(serde_json::from_value(json!({"accountUuid":"a", "orgUuid":"o", "authConfig":{"lastKnownAccountUuid":"a"}, "cookies":{"columns":[],"rows":[]}})).unwrap()), ..Default::default() };
        let mut data = Store {
            accounts: vec![old],
            desktop_current_id: Some("saved".into()),
            ..Default::default()
        };
        let new = import(
            json!({"access_token":"new", "account":{"email_address":"a@example.com"}}),
            None,
        )
        .unwrap();
        upsert(&mut data, new, Some("saved")).unwrap();
        assert_eq!(data.accounts[0].id, "saved");
        assert_eq!(data.accounts[0].label, "Work");
        assert!(data.accounts[0].desktop.is_some());
        assert_eq!(data.desktop_current_id.as_deref(), Some("saved"));
    }
    #[test]
    fn same_email_in_different_organizations_stays_separate() {
        let mut data = Store::default();
        for org in ["personal", "team"] {
            let a=import(json!({"access_token":org, "account":{"uuid":"a","email_address":"a@example.com"}, "organization":{"uuid":org}}),None).unwrap();
            upsert(&mut data, a, None).unwrap();
        }
        assert_eq!(data.accounts.len(), 2);
    }
}
