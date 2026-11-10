use serde_json::{json, Value};
use std::{fs, path::Path};

use super::Snapshot;
use crate::claude::store::write_json;

fn merge_spaces(source: &Path, target: &Path) -> Result<usize, String> {
    if !source.exists() {
        return Ok(0);
    }
    let source: Value = serde_json::from_slice(&fs::read(source).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut target_value = if target.exists() {
        serde_json::from_slice::<Value>(&fs::read(target).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
    } else {
        json!({"spaces":[]})
    };
    let incoming = source["spaces"]
        .as_array()
        .ok_or("Claude spaces.json 格式错误")?;
    let existing = target_value["spaces"]
        .as_array_mut()
        .ok_or("Claude spaces.json 格式错误")?;
    let mut added = 0;
    for item in incoming {
        if !existing.iter().any(|v| v["id"] == item["id"]) {
            existing.push(item.clone());
            added += 1;
        }
    }
    if added > 0 {
        write_json(target, &target_value)?;
    }
    Ok(added)
}

fn copy_missing_records(source: &Path, target: &Path) -> Result<usize, String> {
    if !source.exists() {
        return Ok(0);
    }
    let mut added = 0;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        let to = target.join(entry.file_name());
        if file_type.is_dir() {
            added += copy_missing_records(&entry.path(), &to)?;
        } else if file_type.is_file() && !to.exists() {
            let from = entry.path();
            let extension = from.extension().and_then(|s| s.to_str()).unwrap_or("");
            if !matches!(extension, "json" | "jsonl" | "idx") {
                continue;
            }
            fs::create_dir_all(target).map_err(|e| e.to_string())?;
            fs::copy(&from, &to).map_err(|e| e.to_string())?;
            crate::utils::set_private_permissions(&to);
            added += 1;
        }
    }
    Ok(added)
}

fn scoped_dir(root: &Path, kind: &str, account: &str, org: &str) -> std::path::PathBuf {
    let full = root.join(kind).join(account).join(org);
    if full.exists() || kind == "claude-code-sessions" {
        return full;
    }
    let short = |s: &str| {
        if s.len() == 36 {
            s[..8].to_string()
        } else {
            s.to_string()
        }
    };
    let compact = root.join(kind).join(short(account)).join(short(org));
    if compact.exists() {
        compact
    } else {
        full
    }
}

pub fn preserve(root: &Path, source: &Snapshot, target: &Snapshot) -> Result<usize, String> {
    if source.account_uuid == target.account_uuid && source.org_uuid == target.org_uuid {
        return Ok(0);
    }
    let mut count = copy_missing_records(
        &scoped_dir(
            root,
            "claude-code-sessions",
            &source.account_uuid,
            &source.org_uuid,
        ),
        &scoped_dir(
            root,
            "claude-code-sessions",
            &target.account_uuid,
            &target.org_uuid,
        ),
    )?;
    count += merge_spaces(
        &scoped_dir(
            root,
            "local-agent-mode-sessions",
            &source.account_uuid,
            &source.org_uuid,
        )
        .join("spaces.json"),
        &scoped_dir(
            root,
            "local-agent-mode-sessions",
            &target.account_uuid,
            &target.org_uuid,
        )
        .join("spaces.json"),
    )?;
    let preferences_path = root.join("claude_desktop_config.json");
    if preferences_path.exists() {
        let mut preferences: Value =
            serde_json::from_slice(&fs::read(&preferences_path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let sections = preferences
            .pointer_mut("/preferences/epitaxyPrefs/dframe-code-sections")
            .and_then(Value::as_object_mut);
        if let Some(sections) = sections {
            let source_key = format!("{}/{}", source.account_uuid, source.org_uuid);
            let target_key = format!("{}/{}", target.account_uuid, target.org_uuid);
            if let Some(incoming) = sections.get(&source_key).cloned() {
                if let Some(existing) = sections.get_mut(&target_key) {
                    if let (Some(from), Some(to)) = (
                        incoming["sections"].as_array(),
                        existing["sections"].as_array_mut(),
                    ) {
                        for section in from {
                            if !to.iter().any(|v| v["id"] == section["id"]) {
                                to.push(section.clone());
                            }
                        }
                    }
                } else {
                    sections.insert(target_key, incoming);
                }
                write_json(&preferences_path, &preferences)?;
            }
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::super::cookies::Cookies;
    use super::*;
    fn snapshot(account: &str) -> Snapshot {
        Snapshot {
            account_uuid: account.into(),
            org_uuid: "org".into(),
            auth_config: Default::default(),
            cookies: Cookies {
                columns: vec![],
                rows: vec![],
            },
        }
    }
    #[test]
    fn desktop_switch_merges_records_and_keeps_existing_projects() {
        let root =
            std::env::temp_dir().join(format!("claude-desktop-projects-{}", uuid::Uuid::new_v4()));
        let source = root.join("claude-code-sessions/source/org");
        let target = root.join("claude-code-sessions/target/org");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(
            source.join("old.json"),
            br#"{"cwd":"/existing/workspace","sessionId":"old"}"#,
        )
        .unwrap();
        fs::write(source.join("shared.json"), b"source version").unwrap();
        fs::write(target.join("shared.json"), b"target version").unwrap();
        fs::write(target.join("own.json"), b"own project").unwrap();
        let prefs = json!({"preferences":{"unrelated":true,"epitaxyPrefs":{"dframe-code-sections":{"source/org":{"sections":[{"id":"a","name":"Old project"}]},"target/org":{"sections":[{"id":"b","name":"Existing target project"}]}}}}});
        write_json(&root.join("claude_desktop_config.json"), &prefs).unwrap();
        assert_eq!(
            preserve(&root, &snapshot("source"), &snapshot("target")).unwrap(),
            1
        );
        assert_eq!(
            fs::read(target.join("old.json")).unwrap(),
            fs::read(source.join("old.json")).unwrap()
        );
        assert_eq!(
            fs::read(target.join("shared.json")).unwrap(),
            b"target version"
        );
        assert!(target.join("own.json").exists());
        let after: Value =
            serde_json::from_slice(&fs::read(root.join("claude_desktop_config.json")).unwrap())
                .unwrap();
        assert_eq!(after["preferences"]["unrelated"], true);
        assert_eq!(
            after["preferences"]["epitaxyPrefs"]["dframe-code-sections"]["target/org"]["sections"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            preserve(&root, &snapshot("source"), &snapshot("target")).unwrap(),
            0
        );
        fs::remove_dir_all(root).unwrap();
    }
}
