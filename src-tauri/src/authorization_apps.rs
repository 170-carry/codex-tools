use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationApp {
    path: String,
    name: String,
}

fn app(path: PathBuf) -> AuthorizationApp {
    AuthorizationApp {
        name: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        path: path.to_string_lossy().into_owned(),
    }
}

#[tauri::command]
pub async fn list_authorization_apps() -> Result<Vec<AuthorizationApp>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut paths = Vec::new();
        #[cfg(target_os = "macos")]
        {
            let mut roots = vec![
                PathBuf::from("/Applications"),
                PathBuf::from("/System/Applications"),
            ];
            if let Some(home) = dirs::home_dir() {
                roots.push(home.join("Applications"));
            }
            for root in roots {
                if let Ok(entries) = std::fs::read_dir(root) {
                    paths.extend(
                        entries
                            .flatten()
                            .map(|e| e.path())
                            .filter(|p| p.extension().is_some_and(|s| s == "app")),
                    );
                }
            }
        }
        #[cfg(target_os = "windows")]
        {
            for root in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"]
                .iter()
                .filter_map(std::env::var_os)
            {
                for relative in [
                    "Google/Chrome/Application/chrome.exe",
                    "Microsoft/Edge/Application/msedge.exe",
                    "Mozilla Firefox/firefox.exe",
                    "BraveSoftware/Brave-Browser/Application/brave.exe",
                ] {
                    let path = PathBuf::from(&root).join(relative);
                    if path.is_file() {
                        paths.push(path);
                    }
                }
            }
        }
        #[cfg(target_os = "linux")]
        {
            for name in ["firefox", "google-chrome", "chromium", "brave-browser"] {
                for root in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
                    let path = root.join(name);
                    if path.is_file() {
                        paths.push(path);
                        break;
                    }
                }
            }
        }
        paths.sort();
        paths.dedup();
        let mut apps: Vec<_> = paths.into_iter().map(app).collect();
        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        apps
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn choose_authorization_app() -> Result<Option<AuthorizationApp>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("选择打开授权链接的应用")
            .pick_file()
            .map(app)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_authorization_url(
    url: String,
    application: Option<String>,
) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("仅允许打开 http/https 链接".into());
    }
    let Some(application) = application.filter(|s| !s.is_empty()) else {
        return crate::open_external_url(url);
    };
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        let mut command = {
            let mut c = std::process::Command::new("open");
            c.arg("-a").arg(&application);
            c
        };
        #[cfg(not(target_os = "macos"))]
        let mut command = std::process::Command::new(&application);
        #[cfg(not(target_os = "macos"))]
        {
            command
                .arg(&url)
                .spawn()
                .map_err(|e| format!("打开授权应用失败: {e}"))?;
            Ok(())
        }
        #[cfg(target_os = "macos")]
        {
            let status = command
                .arg(&url)
                .status()
                .map_err(|e| format!("打开授权应用失败: {e}"))?;
            if !status.success() {
                return Err("应用未能打开授权链接".to_string());
            }
            Ok(())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
