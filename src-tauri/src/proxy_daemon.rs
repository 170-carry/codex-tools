use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::runtime::Builder;
use tokio::sync::Mutex;

use crate::proxy_service::new_proxy_storage_context;
use crate::proxy_service::start_api_proxy_with_runtime;
use crate::proxy_service::stop_api_proxy_with_runtime;
use crate::state::ApiProxyRuntimeHandle;
use crate::store::account_store_path_from_data_dir;
use crate::store::sync_current_auth_account_on_startup_in_path;

#[derive(Debug, Clone)]
pub struct ProxyDaemonOptions {
    pub data_dir: PathBuf,
    pub host: String,
    pub port: Option<u16>,
    pub sync_current_auth: bool,
}

pub fn run_cli_from_env() -> Result<(), String> {
    let options = match parse_cli_args(env::args().collect())? {
        CliAction::Run(options) => options,
        CliAction::PrintHelp(text) => {
            println!("{text}");
            return Ok(());
        }
    };
    let runtime = Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("创建 proxyd 运行时失败: {error}"))?;

    runtime.block_on(run_proxy_daemon(options))
}

pub async fn run_proxy_daemon(options: ProxyDaemonOptions) -> Result<(), String> {
    let store_lock = Arc::new(Mutex::new(()));
    let runtime_slot = Mutex::<Option<ApiProxyRuntimeHandle>>::new(None);

    if options.sync_current_auth {
        let store_path = account_store_path_from_data_dir(&options.data_dir);
        sync_current_auth_account_on_startup_in_path(&store_path)?;
    }

    let storage = new_proxy_storage_context(options.data_dir.clone(), store_lock, false);
    let status =
        start_api_proxy_with_runtime(&storage, &runtime_slot, options.port, &options.host).await?;
    let port = status
        .port
        .ok_or_else(|| "代理已启动，但未返回监听端口".to_string())?;

    println!("codex-tools-proxyd started");
    println!("data_dir={}", options.data_dir.display());
    println!("listen=http://{}:{port}/v1", options.host);
    if let Some(api_key) = status.api_key.as_deref() {
        // 仅打印前 8 位，避免完整 API Key 写入 systemd journal / 日志文件
        let preview = &api_key[..api_key.len().min(8)];
        println!("api_key_preview={preview}... (full key stored in data_dir/api-proxy.key)");
    }
    println!("upstream=codex");

    wait_for_shutdown_signal().await?;

    let _ = stop_api_proxy_with_runtime(&storage, &runtime_slot).await?;
    println!("codex-tools-proxyd stopped");
    Ok(())
}

enum CliAction {
    Run(ProxyDaemonOptions),
    PrintHelp(String),
}

fn parse_cli_args(args: Vec<String>) -> Result<CliAction, String> {
    let program = args
        .first()
        .cloned()
        .unwrap_or_else(|| "codex-tools-proxyd".to_string());

    let mut host = "0.0.0.0".to_string();
    let mut port = None;
    let mut data_dir = default_daemon_data_dir()?;
    let mut sync_current_auth = true;

    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "serve" => {
                index += 1;
            }
            "--data-dir" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("缺少 --data-dir 参数值\n\n{}", usage_text(&program)))?;
                data_dir = PathBuf::from(value);
                index += 2;
            }
            "--host" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("缺少 --host 参数值\n\n{}", usage_text(&program)))?;
                host = value.to_string();
                index += 2;
            }
            "--port" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("缺少 --port 参数值\n\n{}", usage_text(&program)))?;
                let parsed = value
                    .parse::<u16>()
                    .map_err(|_| format!("无效端口: {value}\n\n{}", usage_text(&program)))?;
                port = Some(parsed);
                index += 2;
            }
            "--no-sync-current-auth" => {
                sync_current_auth = false;
                index += 1;
            }
            "--help" | "-h" => {
                return Ok(CliAction::PrintHelp(usage_text(&program)));
            }
            unknown => {
                return Err(format!("未知参数: {unknown}\n\n{}", usage_text(&program)));
            }
        }
    }

    Ok(CliAction::Run(ProxyDaemonOptions {
        data_dir,
        host,
        port,
        sync_current_auth,
    }))
}

fn default_daemon_data_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法读取 HOME 目录".to_string())?;
    Ok(home.join(".codex-tools-proxyd"))
}

async fn wait_for_shutdown_signal() -> Result<(), String> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::signal;
        use tokio::signal::unix::SignalKind;

        let mut terminate = signal(SignalKind::terminate())
            .map_err(|error| format!("监听 SIGTERM 失败: {error}"))?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result.map_err(|error| format!("监听 Ctrl+C 失败: {error}"))?;
            }
            _ = terminate.recv() => {}
        }
        Ok(())
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .map_err(|error| format!("监听 Ctrl+C 失败: {error}"))
    }
}

fn usage_text(program: &str) -> String {
    format!(
        "Usage:\n  {program} serve [--data-dir PATH] [--host HOST] [--port PORT] [--no-sync-current-auth]\n\nDefaults:\n  --data-dir ~/.codex-tools-proxyd\n  --host 0.0.0.0\n  --port 8787"
    )
}
