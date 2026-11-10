use serde_json::{json, Value};
use std::{fs, path::PathBuf};

fn config_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(path));
    }
    Ok(dirs::home_dir().ok_or("无法读取用户目录")?.join(".claude"))
}

fn read_json(path: &std::path::Path) -> Result<Value, String> {
    match fs::read(path) {
        Ok(data) => serde_json::from_slice(&data).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(target_os = "macos")]
fn keychain_identity() -> Result<(String, String), String> {
    use sha2::{Digest, Sha256};
    let suffix = if std::env::var_os("CLAUDE_CONFIG_DIR").is_some() {
        let path = config_dir()?;
        format!("-{:x}", Sha256::digest(path.to_string_lossy().as_bytes()))[..9].to_string()
    } else {
        String::new()
    };
    let user = std::env::var("USER").map_err(|e| e.to_string())?;
    Ok((format!("Claude Code{suffix}-credentials"), user))
}

pub fn read_credentials() -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    {
        let (service, user) = keychain_identity()?;
        match security_framework::passwords::get_generic_password(&service, &user) {
            Ok(data) => return serde_json::from_slice(&data).map_err(|e| e.to_string()),
            Err(e) if e.code() == -25300 => {}
            Err(e) => return Err(format!("读取 Claude 钥匙串失败: {e}")),
        }
    }
    read_json(&config_dir()?.join(".credentials.json"))
}

fn write_credentials(value: &Value) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let (service, user) = keychain_identity()?;
        let data = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        security_framework::passwords::set_generic_password(&service, &user, &data)
            .map_err(|e| format!("保存 Claude 钥匙串失败: {e}"))?;
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    super::store::write_json(&config_dir()?.join(".credentials.json"), value)
}

pub fn activate(account: &super::store::Account) -> Result<(), String> {
    activate_at(&config_dir()?, account, read_credentials, write_credentials)
}

fn activate_at(
    dir: &std::path::Path,
    account: &super::store::Account,
    read_credentials: impl FnOnce() -> Result<Value, String>,
    write_credentials: impl FnOnce(&Value) -> Result<(), String>,
) -> Result<(), String> {
    let settings_path = dir.join("settings.json");
    let mut settings = read_json(&settings_path)?;
    if settings.get("env").is_none() {
        settings["env"] = json!({});
    }
    let env = settings["env"]
        .as_object_mut()
        .ok_or("Claude settings.env 格式错误")?;
    if let Some(key) = &account.api_key {
        env.remove("ANTHROPIC_AUTH_TOKEN");
        env.insert("ANTHROPIC_API_KEY".into(), json!(key));
        env.insert("ANTHROPIC_BASE_URL".into(), json!(account.api_base_url));
        if let Some(model) = &account.model {
            env.insert("ANTHROPIC_MODEL".into(), json!(model));
        } else {
            env.remove("ANTHROPIC_MODEL");
        }
    } else {
        for key in [
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_MODEL",
        ] {
            env.remove(key);
        }
        let mut local = read_credentials()?;
        local["claudeAiOauth"] = account.credentials.clone();
        write_credentials(&local)?;
    }
    super::store::write_json(&settings_path, &settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn switching_keeps_projects_history_and_non_auth_settings() {
        let root = std::env::temp_dir().join(format!("claude-preserve-{}", uuid::Uuid::new_v4()));
        let dir = root.join(".claude");
        fs::create_dir_all(dir.join("projects/workspace")).unwrap();
        let markers = [
            root.join(".claude.json"),
            dir.join("history.jsonl"),
            dir.join("projects/workspace/session.jsonl"),
            dir.join("projects/index.json"),
        ];
        for path in &markers {
            fs::write(
                path,
                br#"{"projects":{"/my/project":{"trusted":true}},"history":["existing"]}"#,
            )
            .unwrap();
        }
        let before: Vec<_> = markers.iter().map(|p| fs::read(p).unwrap()).collect();
        let settings = json!({"permissions":{"allow":["Read"]}, "hooks":{"SessionStart":[]}, "env":{"OTHER":"preserved", "ANTHROPIC_API_KEY":"old"}});
        super::super::store::write_json(&dir.join("settings.json"), &settings).unwrap();
        let account =
            super::super::store::import(json!({"claudeAiOauth":{"accessToken":"new-token"}}), None)
                .unwrap();
        activate_at(
            &dir,
            &account,
            || Ok(json!({"claudeAiOauth":{"accessToken":"old"}, "mcpOAuth":{"keep":true}})),
            |updated| {
                assert_eq!(updated["mcpOAuth"]["keep"], true);
                assert_eq!(updated["claudeAiOauth"]["accessToken"], "new-token");
                Ok(())
            },
        )
        .unwrap();
        let saved = read_json(&dir.join("settings.json")).unwrap();
        assert_eq!(saved["permissions"], settings["permissions"]);
        assert_eq!(saved["hooks"], settings["hooks"]);
        assert_eq!(saved["env"]["OTHER"], "preserved");
        assert!(saved["env"].get("ANTHROPIC_API_KEY").is_none());
        for (path, expected) in markers.iter().zip(before) {
            assert_eq!(fs::read(path).unwrap(), expected);
        }
        fs::remove_dir_all(root).unwrap();
    }
}

pub fn replace_if_matches(previous: &Value, updated: &Value) -> Result<(), String> {
    let mut saved = read_credentials()?;
    if saved["claudeAiOauth"]["accessToken"] == previous["accessToken"]
        && saved["claudeAiOauth"]["refreshToken"] == previous["refreshToken"]
    {
        saved["claudeAiOauth"] = updated.clone();
        write_credentials(&saved)?;
    }
    Ok(())
}
