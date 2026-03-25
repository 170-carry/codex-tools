use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rfd::FileDialog;
use serde::Serialize;
use tauri::AppHandle;
use tauri::Emitter;
use tauri::Manager;

use crate::models::AccountsStore;
use crate::models::DeployRemoteProxyInput;
use crate::models::RemoteAuthMode;
use crate::models::RemoteProxyStatus;
use crate::models::RemoteServerConfig;
use crate::store::account_store_path_from_data_dir;
use crate::utils::find_command_path;
use crate::utils::new_resolved_command;
use crate::utils::now_unix_seconds;
use crate::utils::prepend_path_entry;

const REMOTE_BINARY_NAME: &str = "codex-tools-proxyd";
const REMOTE_DEPLOY_PROGRESS_EVENT: &str = "remote-deploy-progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteDeployProgressEvent {
    server_id: String,
    label: String,
    stage: String,
    progress: u8,
    detail: Option<String>,
}

#[derive(Debug, Clone)]
struct LocalRustToolchain {
    cargo_bin: PathBuf,
    rustc_bin: Option<PathBuf>,
    rustup_toolchain: Option<String>,
    path_env: Option<OsString>,
}

impl LocalRustToolchain {
    fn resolve() -> Self {
        let cargo_bin = rustup_which("cargo")
            .or_else(|| find_command_path("cargo"))
            .unwrap_or_else(|| PathBuf::from("cargo"));
        let rustc_bin = rustup_which("rustc").or_else(|| find_command_path("rustc"));
        let bin_dir = cargo_bin
            .parent()
            .filter(|_| cargo_bin.is_absolute())
            .or_else(|| {
                rustc_bin
                    .as_deref()
                    .filter(|path| path.is_absolute())
                    .and_then(Path::parent)
            })
            .map(Path::to_path_buf);

        Self {
            cargo_bin,
            rustc_bin,
            rustup_toolchain: rustup_active_toolchain_name(),
            path_env: bin_dir.as_deref().and_then(prepend_path_entry),
        }
    }

    fn apply_to_command(&self, command: &mut Command) {
        if let Some(path_env) = &self.path_env {
            command.env("PATH", path_env);
        }
        if let Some(rustc_bin) = &self.rustc_bin {
            command.env("RUSTC", rustc_bin);
        }
    }

    fn new_cargo_command(&self) -> Command {
        let mut command = Command::new(&self.cargo_bin);
        self.apply_to_command(&mut command);
        command
    }
}

pub(crate) async fn get_remote_proxy_status_internal(
    server: RemoteServerConfig,
) -> Result<RemoteProxyStatus, String> {
    tauri::async_runtime::spawn_blocking(move || get_remote_proxy_status_sync(&server))
        .await
        .map_err(|error| format!("查询远程代理状态失败: {error}"))?
}

pub(crate) async fn deploy_remote_proxy_internal(
    app: &AppHandle,
    input: DeployRemoteProxyInput,
) -> Result<RemoteProxyStatus, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || deploy_remote_proxy_sync(&app, input.server))
        .await
        .map_err(|error| format!("部署远程代理失败: {error}"))?
}

pub(crate) async fn start_remote_proxy_internal(
    server: RemoteServerConfig,
) -> Result<RemoteProxyStatus, String> {
    tauri::async_runtime::spawn_blocking(move || start_remote_proxy_sync(&server))
        .await
        .map_err(|error| format!("启动远程代理失败: {error}"))?
}

pub(crate) async fn stop_remote_proxy_internal(
    server: RemoteServerConfig,
) -> Result<RemoteProxyStatus, String> {
    tauri::async_runtime::spawn_blocking(move || stop_remote_proxy_sync(&server))
        .await
        .map_err(|error| format!("停止远程代理失败: {error}"))?
}

pub(crate) async fn read_remote_proxy_logs_internal(
    server: RemoteServerConfig,
    lines: usize,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || read_remote_proxy_logs_sync(&server, lines))
        .await
        .map_err(|error| format!("读取远程代理日志失败: {error}"))?
}

pub(crate) async fn pick_local_identity_file_internal() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(FileDialog::new()
            .set_title("选择 SSH 私钥文件")
            .pick_file()
            .map(|path| path.to_string_lossy().to_string()))
    })
    .await
    .map_err(|error| format!("打开本地文件选择器失败: {error}"))?
}

pub(crate) async fn is_sshpass_available_internal() -> bool {
    tauri::async_runtime::spawn_blocking(command_sshpass_available)
        .await
        .unwrap_or(false)
}

pub(crate) async fn install_sshpass_internal() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(install_sshpass_sync)
        .await
        .map_err(|error| format!("安装 sshpass 失败: {error}"))?
}

fn deploy_remote_proxy_sync(
    app: &AppHandle,
    server: RemoteServerConfig,
) -> Result<RemoteProxyStatus, String> {
    let server = validate_remote_server(&server)?;
    emit_remote_deploy_progress(
        app,
        &server,
        "validating",
        6,
        Some(format!("{}@{}", server.ssh_user, server.host)),
    );
    ensure_ssh_tools_available_for(&server)?;

    let built_binary = build_linux_binary_for_server(app, &server)?;
    let accounts_json = local_accounts_store_json(app)?;
    let service_name = remote_systemd_service_name(&server);
    let service_content = render_systemd_unit(&server, &service_name);

    let temp_dir = env::temp_dir().join(format!("codex-tools-remote-{}", now_unix_seconds()));
    emit_remote_deploy_progress(
        app,
        &server,
        "preparingFiles",
        52,
        Some(temp_dir.display().to_string()),
    );
    fs::create_dir_all(&temp_dir)
        .map_err(|error| format!("创建远程部署临时目录失败 {}: {error}", temp_dir.display()))?;

    let binary_path = temp_dir.join(REMOTE_BINARY_NAME);
    let accounts_path = temp_dir.join("accounts.json");
    let service_path = temp_dir.join(&service_name);
    fs::copy(&built_binary, &binary_path).map_err(|error| {
        format!(
            "写入远程代理二进制临时文件失败 {}: {error}",
            binary_path.display()
        )
    })?;
    fs::write(&accounts_path, accounts_json).map_err(|error| {
        format!(
            "写入远程账号存储临时文件失败 {}: {error}",
            accounts_path.display()
        )
    })?;
    fs::write(&service_path, service_content).map_err(|error| {
        format!(
            "写入远程 systemd 服务文件失败 {}: {error}",
            service_path.display()
        )
    })?;

    let stage_dir = format!(
        "/tmp/codex-tools-remote-{}-{}",
        sanitize_service_fragment(&server.id),
        now_unix_seconds()
    );

    emit_remote_deploy_progress(
        app,
        &server,
        "preparingFiles",
        58,
        Some(format!("mkdir -p {stage_dir}")),
    );
    run_ssh(&server, &format!("mkdir -p {}", shell_quote(&stage_dir)))?;
    emit_remote_deploy_progress(
        app,
        &server,
        "uploadingBinary",
        64,
        Some(format!("scp {}", binary_path.display())),
    );
    run_scp(
        &server,
        &binary_path,
        &format!("{stage_dir}/{REMOTE_BINARY_NAME}"),
    )?;
    emit_remote_deploy_progress(
        app,
        &server,
        "uploadingAccounts",
        72,
        Some(format!("scp {}", accounts_path.display())),
    );
    run_scp(
        &server,
        &accounts_path,
        &format!("{stage_dir}/accounts.json"),
    )?;
    emit_remote_deploy_progress(
        app,
        &server,
        "uploadingService",
        80,
        Some(format!("scp {service_name}")),
    );
    run_scp(
        &server,
        &service_path,
        &format!("{stage_dir}/{service_name}"),
    )?;

    emit_remote_deploy_progress(
        app,
        &server,
        "installingService",
        90,
        Some("systemctl daemon-reload && enable/start".to_string()),
    );
    let staged_binary_path = join_posix_path(&stage_dir, REMOTE_BINARY_NAME);
    let staged_accounts_path = join_posix_path(&stage_dir, "accounts.json");
    let staged_service_path = join_posix_path(&stage_dir, &service_name);
    let installed_binary_path = join_posix_path(&server.remote_dir, REMOTE_BINARY_NAME);
    let installed_accounts_path = join_posix_path(&server.remote_dir, "accounts.json");
    let installed_service_path = join_posix_path("/etc/systemd/system", &service_name);
    run_root_ssh(
        &server,
        &format!(
            "mkdir -p {dir}; \
mv {stage_bin} {dir_bin}; chmod 700 {dir_bin}; \
mv {stage_accounts} {dir_accounts}; chmod 600 {dir_accounts}; \
mv {stage_service} {service_install}; chmod 644 {service_install}; \
rm -rf {stage_dir}; \
systemctl daemon-reload; \
systemctl enable {unit} >/dev/null 2>&1 || true; \
if systemctl is-active --quiet {unit}; then systemctl restart {unit}; else systemctl start {unit}; fi",
            dir = shell_quote(&server.remote_dir),
            stage_bin = shell_quote(&staged_binary_path),
            dir_bin = shell_quote(&installed_binary_path),
            stage_accounts = shell_quote(&staged_accounts_path),
            dir_accounts = shell_quote(&installed_accounts_path),
            stage_service = shell_quote(&staged_service_path),
            service_install = shell_quote(&installed_service_path),
            stage_dir = shell_quote(&stage_dir),
            unit = shell_quote(&service_name),
        ),
    )?;

    let _ = fs::remove_file(&binary_path);
    let _ = fs::remove_file(&accounts_path);
    let _ = fs::remove_file(&service_path);
    let _ = fs::remove_dir_all(&temp_dir);

    emit_remote_deploy_progress(app, &server, "verifying", 96, Some(service_name.clone()));
    get_remote_proxy_status_sync(&server)
}

fn start_remote_proxy_sync(server: &RemoteServerConfig) -> Result<RemoteProxyStatus, String> {
    let server = validate_remote_server(server)?;
    ensure_ssh_tools_available_for(&server)?;
    let service_name = remote_systemd_service_name(&server);
    run_root_ssh(
        &server,
        &format!("systemctl start {}", shell_quote(&service_name)),
    )?;
    get_remote_proxy_status_sync(&server)
}

fn stop_remote_proxy_sync(server: &RemoteServerConfig) -> Result<RemoteProxyStatus, String> {
    let server = validate_remote_server(server)?;
    ensure_ssh_tools_available_for(&server)?;
    let service_name = remote_systemd_service_name(&server);
    run_root_ssh(
        &server,
        &format!("systemctl stop {}", shell_quote(&service_name)),
    )?;
    get_remote_proxy_status_sync(&server)
}

fn read_remote_proxy_logs_sync(
    server: &RemoteServerConfig,
    lines: usize,
) -> Result<String, String> {
    let server = validate_remote_server(server)?;
    ensure_ssh_tools_available_for(&server)?;
    let service_name = remote_systemd_service_name(&server);
    let safe_lines = lines.clamp(20, 400);
    run_root_ssh(
        &server,
        &format!(
            "if ! command -v journalctl >/dev/null 2>&1; then echo missing_journalctl; exit 41; fi; \
journalctl -u {} -n {} --no-pager",
            shell_quote(&service_name),
            safe_lines
        ),
    )
}

fn get_remote_proxy_status_sync(server: &RemoteServerConfig) -> Result<RemoteProxyStatus, String> {
    let server = validate_remote_server(server)?;
    ensure_ssh_tools_available_for(&server)?;
    let service_name = remote_systemd_service_name(&server);
    let command = format!(
        "DIR={dir}; BIN=\"$DIR/{bin}\"; KEYFILE=\"$DIR/api-proxy.key\"; UNIT={unit}; \
INSTALLED=0; SERVICE_INSTALLED=0; RUNNING=0; ENABLED=0; PID=\"\"; API_KEY=\"\"; \
if [ -x \"$BIN\" ]; then INSTALLED=1; fi; \
if command -v systemctl >/dev/null 2>&1; then \
  if [ -f \"/etc/systemd/system/$UNIT\" ] || [ -f \"/lib/systemd/system/$UNIT\" ]; then SERVICE_INSTALLED=1; fi; \
  ENABLED_STATE=$(systemctl is-enabled \"$UNIT\" 2>/dev/null || true); \
  if [ \"$ENABLED_STATE\" = \"enabled\" ]; then ENABLED=1; fi; \
  ACTIVE_STATE=$(systemctl is-active \"$UNIT\" 2>/dev/null || true); \
  if [ \"$ACTIVE_STATE\" = \"active\" ]; then RUNNING=1; fi; \
  PID=$(systemctl show -p MainPID --value \"$UNIT\" 2>/dev/null || true); \
  if [ \"$PID\" = \"0\" ]; then PID=\"\"; fi; \
fi; \
if [ -f \"$KEYFILE\" ]; then API_KEY=$(cat \"$KEYFILE\" 2>/dev/null || true); fi; \
printf 'installed=%s\\nservice_installed=%s\\nrunning=%s\\nenabled=%s\\npid=%s\\napi_key=%s\\n' \"$INSTALLED\" \"$SERVICE_INSTALLED\" \"$RUNNING\" \"$ENABLED\" \"$PID\" \"$API_KEY\"",
        dir = shell_quote(&server.remote_dir),
        bin = REMOTE_BINARY_NAME,
        unit = shell_quote(&service_name),
    );

    let output = run_ssh(&server, &command)?;
    let mut installed = false;
    let mut service_installed = false;
    let mut running = false;
    let mut enabled = false;
    let mut pid = None;
    let mut api_key = None;

    for line in output.lines() {
        if let Some(value) = line.strip_prefix("installed=") {
            installed = value.trim() == "1";
        } else if let Some(value) = line.strip_prefix("service_installed=") {
            service_installed = value.trim() == "1";
        } else if let Some(value) = line.strip_prefix("running=") {
            running = value.trim() == "1";
        } else if let Some(value) = line.strip_prefix("enabled=") {
            enabled = value.trim() == "1";
        } else if let Some(value) = line.strip_prefix("pid=") {
            pid = value.trim().parse::<u32>().ok();
        } else if let Some(value) = line.strip_prefix("api_key=") {
            let key = value.trim();
            if !key.is_empty() {
                api_key = Some(key.to_string());
            }
        }
    }

    Ok(RemoteProxyStatus {
        installed,
        service_installed,
        running,
        enabled,
        service_name,
        pid,
        base_url: format!("http://{}:{}/v1", server.host, server.listen_port),
        api_key,
        last_error: None,
    })
}

fn validate_remote_server(server: &RemoteServerConfig) -> Result<RemoteServerConfig, String> {
    let mut normalized = server.clone();
    normalized.label = normalized.label.trim().to_string();
    normalized.host = normalized.host.trim().to_string();
    normalized.ssh_user = normalized.ssh_user.trim().to_string();
    normalized.remote_dir = normalized.remote_dir.trim().to_string();
    normalized.identity_file = normalized
        .identity_file
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    normalized.private_key = normalized
        .private_key
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    normalized.password = normalized
        .password
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if normalized.label.is_empty() {
        return Err("远程服务器名称不能为空".to_string());
    }
    if normalized.host.is_empty() {
        return Err("远程服务器 Host 不能为空".to_string());
    }
    if normalized.ssh_user.is_empty() {
        return Err("远程服务器 SSH 用户不能为空".to_string());
    }
    if normalized.remote_dir.is_empty() {
        return Err("远程服务器部署目录不能为空".to_string());
    }
    if normalized.ssh_port == 0 {
        return Err("远程服务器 SSH 端口无效".to_string());
    }
    if normalized.listen_port == 0 {
        return Err("远程服务器代理端口无效".to_string());
    }

    match normalized.auth_mode {
        RemoteAuthMode::KeyContent => {
            if normalized.private_key.is_none() {
                return Err("SSH 私钥内容不能为空".to_string());
            }
        }
        RemoteAuthMode::KeyFile | RemoteAuthMode::KeyPath => {
            if normalized.identity_file.is_none() {
                return Err("SSH 私钥路径不能为空".to_string());
            }
        }
        RemoteAuthMode::Password => {
            if normalized.password.is_none() {
                return Err("SSH 密码不能为空".to_string());
            }
        }
    }

    Ok(normalized)
}

fn build_linux_binary_for_server(
    app: &AppHandle,
    server: &RemoteServerConfig,
) -> Result<PathBuf, String> {
    let cargo_toolchain = ensure_local_rust_toolchain_available(app, server)?;
    emit_remote_deploy_progress(
        app,
        server,
        "detectingPlatform",
        12,
        Some("uname -s && uname -m".to_string()),
    );
    let platform = detect_remote_platform(server)?;
    let workspace_manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest_dir = workspace_manifest_dir.join("proxyd");
    let manifest_path = manifest_dir.join("Cargo.toml");
    let target_dir = proxyd_build_target_dir()?;
    let output_targets = [platform.primary_target, platform.fallback_target];
    let mut attempts = Vec::new();
    let mut cross_available = command_exists("cross");
    let mut zigbuild_available = cargo_subcommand_available("zigbuild", &cargo_toolchain);
    let mut zig_available = command_exists("zig");

    if !cfg!(target_os = "linux") && !cross_available && !(zigbuild_available && zig_available) {
        ensure_linux_build_dependencies_available(app, server, &cargo_toolchain)?;
        cross_available = command_exists("cross");
        zigbuild_available = cargo_subcommand_available("zigbuild", &cargo_toolchain);
        zig_available = command_exists("zig");
    }

    for target in output_targets {
        let binary_path = target_dir
            .join(target)
            .join("release")
            .join(REMOTE_BINARY_NAME);

        if command_exists("rustup") {
            emit_remote_deploy_progress(
                app,
                server,
                "preparingBuilder",
                22,
                Some(format!("rustup target add {target}")),
            );
            let _ = ensure_rust_target(target, &cargo_toolchain);
        }

        if cross_available {
            emit_remote_deploy_progress(
                app,
                server,
                "buildingBinary",
                24,
                Some(format!(
                    "cross build --manifest-path {} --release --target {target}",
                    manifest_path.display()
                )),
            );
            let mut command = new_resolved_command("cross");
            command
                .current_dir(&manifest_dir)
                .env("CARGO_TARGET_DIR", &target_dir)
                .args([
                    "build",
                    "--manifest-path",
                    manifest_path.to_string_lossy().as_ref(),
                    "--release",
                    "--target",
                    target,
                ]);
            match run_local_command(&mut command) {
                Ok(()) if binary_path.exists() => return Ok(binary_path),
                Ok(()) => attempts.push(format!("cross {target}: 未找到构建产物")),
                Err(error) => attempts.push(format!("cross {target}: {error}")),
            }
        }

        if zigbuild_available && zig_available {
            emit_remote_deploy_progress(
                app,
                server,
                "buildingBinary",
                28,
                Some(format!(
                    "cargo zigbuild --manifest-path {} --release --target {target}",
                    manifest_path.display()
                )),
            );
            let mut command = cargo_toolchain.new_cargo_command();
            command
                .current_dir(&manifest_dir)
                .env("CARGO_TARGET_DIR", &target_dir)
                .args([
                    "zigbuild",
                    "--manifest-path",
                    manifest_path.to_string_lossy().as_ref(),
                    "--release",
                    "--target",
                    target,
                ]);
            match run_local_command(&mut command) {
                Ok(()) if binary_path.exists() => return Ok(binary_path),
                Ok(()) => attempts.push(format!("cargo zigbuild {target}: 未找到构建产物")),
                Err(error) => attempts.push(format!("cargo zigbuild {target}: {error}")),
            }
        }

        emit_remote_deploy_progress(
            app,
            server,
            "preparingBuilder",
            30,
            Some(format!("rustup target add {target}")),
        );
        let _ = ensure_rust_target(target, &cargo_toolchain);
        emit_remote_deploy_progress(
            app,
            server,
            "buildingBinary",
            34,
            Some(format!(
                "cargo build --manifest-path {} --release --target {target}",
                manifest_path.display()
            )),
        );
        let mut command = cargo_toolchain.new_cargo_command();
        command
            .current_dir(&manifest_dir)
            .env("CARGO_TARGET_DIR", &target_dir)
            .args([
                "build",
                "--manifest-path",
                manifest_path.to_string_lossy().as_ref(),
                "--release",
                "--target",
                target,
            ]);
        match run_local_command(&mut command) {
            Ok(()) if binary_path.exists() => return Ok(binary_path),
            Ok(()) => attempts.push(format!("cargo build {target}: 未找到构建产物")),
            Err(error) => attempts.push(format!("cargo build {target}: {error}")),
        }
    }

    Err(format!(
        "自动构建 Linux 二进制失败。已尝试自动补齐本机 Linux 构建依赖，但仍未完成交叉编译。{}",
        if attempts.is_empty() {
            String::new()
        } else {
            format!(" 详情: {}", attempts.join(" | "))
        }
    ))
}

fn ensure_local_rust_toolchain_available(
    app: &AppHandle,
    server: &RemoteServerConfig,
) -> Result<LocalRustToolchain, String> {
    let mut cargo_toolchain = LocalRustToolchain::resolve();
    if ensure_program_available(
        &cargo_toolchain.cargo_bin,
        "cargo",
        "未检测到 cargo 命令，请先安装 Rust 工具链。",
    )
    .is_ok()
    {
        return Ok(cargo_toolchain);
    }

    install_rust_toolchain_sync(app, server)?;
    cargo_toolchain = LocalRustToolchain::resolve();
    ensure_program_available(
        &cargo_toolchain.cargo_bin,
        "cargo",
        "自动安装 Rust 工具链后仍未检测到 cargo 命令。",
    )?;
    Ok(cargo_toolchain)
}

fn proxyd_build_target_dir() -> Result<PathBuf, String> {
    let base = dirs::cache_dir().unwrap_or_else(env::temp_dir);
    let target_dir = base.join("codex-tools").join("proxyd-target");
    fs::create_dir_all(&target_dir).map_err(|error| {
        format!(
            "创建 proxyd 构建缓存目录失败 {}: {error}",
            target_dir.display()
        )
    })?;
    Ok(target_dir)
}

fn emit_remote_deploy_progress(
    app: &AppHandle,
    server: &RemoteServerConfig,
    stage: &str,
    progress: u8,
    detail: Option<String>,
) {
    let label = if server.label.trim().is_empty() {
        server.host.clone()
    } else {
        server.label.clone()
    };
    let _ = app.emit(
        REMOTE_DEPLOY_PROGRESS_EVENT,
        RemoteDeployProgressEvent {
            server_id: server.id.clone(),
            label,
            stage: stage.to_string(),
            progress,
            detail,
        },
    );
}

fn detect_remote_platform(server: &RemoteServerConfig) -> Result<RemotePlatform, String> {
    let output = run_ssh(server, "uname -s && uname -m")?;
    let mut lines = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let os_name = lines.next().unwrap_or_default();
    let arch = lines.next().unwrap_or_default();

    if os_name != "Linux" {
        return Err(format!("远程系统不是 Linux，当前检测到的是 {os_name}"));
    }

    match arch {
        "x86_64" | "amd64" => Ok(RemotePlatform {
            primary_target: "x86_64-unknown-linux-musl",
            fallback_target: "x86_64-unknown-linux-gnu",
        }),
        "aarch64" | "arm64" => Ok(RemotePlatform {
            primary_target: "aarch64-unknown-linux-musl",
            fallback_target: "aarch64-unknown-linux-gnu",
        }),
        other => Err(format!("暂不支持的远程 Linux 架构: {other}")),
    }
}

fn render_systemd_unit(server: &RemoteServerConfig, service_name: &str) -> String {
    format!(
        "[Unit]\nDescription=Codex Tools Remote API Proxy ({})\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nWorkingDirectory={dir}\nExecStart={dir}/{bin} serve --data-dir {dir} --host 0.0.0.0 --port {port} --no-sync-current-auth\nRestart=always\nRestartSec=3\nEnvironment=RUST_LOG=info\n\n[Install]\nWantedBy=multi-user.target\n",
        service_name,
        dir = server.remote_dir,
        bin = REMOTE_BINARY_NAME,
        port = server.listen_port,
    )
}

fn local_accounts_store_json(app: &AppHandle) -> Result<String, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法获取应用数据目录: {error}"))?;
    let path = account_store_path_from_data_dir(&data_dir);

    if path.exists() {
        return fs::read_to_string(&path)
            .map_err(|error| format!("读取本地账号存储失败 {}: {error}", path.display()));
    }

    serde_json::to_string_pretty(&AccountsStore::default())
        .map_err(|error| format!("序列化默认账号存储失败: {error}"))
}

fn ensure_ssh_tools_available_for(server: &RemoteServerConfig) -> Result<(), String> {
    ensure_command_available("ssh", "未检测到 ssh 命令，请先安装 OpenSSH。")?;
    ensure_command_available("scp", "未检测到 scp 命令，请先安装 OpenSSH。")?;
    if matches!(server.auth_mode, RemoteAuthMode::Password) {
        ensure_command_available("sshpass", "未检测到 sshpass 命令，请先安装 sshpass。")?;
    }
    Ok(())
}

fn ensure_command_available(command: &str, message: &str) -> Result<(), String> {
    new_resolved_command(command)
        .arg("-V")
        .output()
        .map(|_| ())
        .or_else(|_| {
            new_resolved_command(command)
                .arg("--version")
                .output()
                .map(|_| ())
        })
        .map_err(|_| message.to_string())
}

fn command_sshpass_available() -> bool {
    command_exists("sshpass")
}

fn command_exists(command: &str) -> bool {
    new_resolved_command(command)
        .arg("--version")
        .output()
        .is_ok()
        || new_resolved_command(command).arg("-V").output().is_ok()
}

fn cargo_subcommand_available(subcommand: &str, cargo_toolchain: &LocalRustToolchain) -> bool {
    let standalone = format!("cargo-{subcommand}");
    if command_exists(&standalone) {
        return true;
    }

    let mut long_help = cargo_toolchain.new_cargo_command();
    long_help.arg(subcommand).arg("--help");
    if long_help
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
    {
        return true;
    }

    let mut short_help = cargo_toolchain.new_cargo_command();
    short_help.arg(subcommand).arg("-h");
    short_help
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn install_sshpass_sync() -> Result<(), String> {
    if command_sshpass_available() {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        ensure_command_available(
            "brew",
            "未检测到 Homebrew，请先安装 brew 后再自动安装 sshpass。",
        )?;
        run_install_command(
            "brew",
            &["install", "sshpass"],
            "通过 Homebrew 安装 sshpass 失败",
        )?;
    }

    #[cfg(target_os = "windows")]
    {
        return Err("当前平台暂未内置一键安装 sshpass，请先手动安装。".to_string());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if command_exists("apt-get") {
            run_shell_install_command(
                "sudo apt-get update && sudo apt-get install -y sshpass",
                "通过 apt-get 安装 sshpass 失败",
            )?;
        } else if command_exists("dnf") {
            run_shell_install_command("sudo dnf install -y sshpass", "通过 dnf 安装 sshpass 失败")?;
        } else if command_exists("yum") {
            run_shell_install_command("sudo yum install -y sshpass", "通过 yum 安装 sshpass 失败")?;
        } else if command_exists("pacman") {
            run_shell_install_command(
                "sudo pacman -Sy --noconfirm sshpass",
                "通过 pacman 安装 sshpass 失败",
            )?;
        } else {
            return Err("当前平台暂未内置一键安装 sshpass，请先手动安装。".to_string());
        }
    }

    if !command_sshpass_available() {
        return Err("自动安装 sshpass 后仍未检测到可执行文件。".to_string());
    }

    Ok(())
}

fn install_rust_toolchain_sync(app: &AppHandle, server: &RemoteServerConfig) -> Result<(), String> {
    if command_exists("cargo") {
        return Ok(());
    }

    if command_exists("rustup") {
        emit_remote_deploy_progress(
            app,
            server,
            "preparingBuilder",
            10,
            Some("rustup default stable".to_string()),
        );
        run_install_command(
            "rustup",
            &["default", "stable"],
            "通过 rustup 初始化 Rust 工具链失败",
        )?;
        if command_exists("cargo") {
            return Ok(());
        }
    }

    #[cfg(target_os = "macos")]
    {
        emit_remote_deploy_progress(
            app,
            server,
            "preparingBuilder",
            10,
            Some("brew install rust".to_string()),
        );
        ensure_command_available(
            "brew",
            "未检测到 Homebrew，请先安装 brew 后再自动安装 Rust 工具链。",
        )?;
        run_install_command(
            "brew",
            &["install", "rust"],
            "通过 Homebrew 安装 Rust 工具链失败",
        )?;
    }

    #[cfg(target_os = "windows")]
    {
        return Err("当前平台暂未内置一键安装 Rust 工具链，请先手动安装。".to_string());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if command_exists("apt-get") {
            emit_remote_deploy_progress(
                app,
                server,
                "preparingBuilder",
                10,
                Some("sudo apt-get update && sudo apt-get install -y cargo rustc".to_string()),
            );
            run_shell_install_command(
                "sudo apt-get update && sudo apt-get install -y cargo rustc",
                "通过 apt-get 安装 Rust 工具链失败",
            )?;
        } else if command_exists("dnf") {
            emit_remote_deploy_progress(
                app,
                server,
                "preparingBuilder",
                10,
                Some("sudo dnf install -y cargo rust rustup".to_string()),
            );
            run_shell_install_command(
                "sudo dnf install -y cargo rust rustup",
                "通过 dnf 安装 Rust 工具链失败",
            )?;
        } else if command_exists("yum") {
            emit_remote_deploy_progress(
                app,
                server,
                "preparingBuilder",
                10,
                Some("sudo yum install -y cargo rust rustup".to_string()),
            );
            run_shell_install_command(
                "sudo yum install -y cargo rust rustup",
                "通过 yum 安装 Rust 工具链失败",
            )?;
        } else if command_exists("pacman") {
            emit_remote_deploy_progress(
                app,
                server,
                "preparingBuilder",
                10,
                Some("sudo pacman -Sy --noconfirm rustup cargo".to_string()),
            );
            run_shell_install_command(
                "sudo pacman -Sy --noconfirm rustup cargo",
                "通过 pacman 安装 Rust 工具链失败",
            )?;
        } else {
            return Err("当前平台暂未内置一键安装 Rust 工具链，请先手动安装。".to_string());
        }
    }

    if !command_exists("cargo") {
        return Err("自动安装 Rust 工具链后仍未检测到 cargo 命令。".to_string());
    }

    Ok(())
}

fn ensure_linux_build_dependencies_available(
    app: &AppHandle,
    server: &RemoteServerConfig,
    cargo_toolchain: &LocalRustToolchain,
) -> Result<(), String> {
    if command_exists("cross")
        || (cargo_subcommand_available("zigbuild", cargo_toolchain) && command_exists("zig"))
    {
        return Ok(());
    }

    install_linux_build_dependencies_sync(app, server, cargo_toolchain)?;

    if command_exists("cross")
        || (cargo_subcommand_available("zigbuild", cargo_toolchain) && command_exists("zig"))
    {
        return Ok(());
    }

    Err("自动安装 Linux 构建依赖后仍未检测到可用的 cargo-zigbuild / zig。".to_string())
}

fn install_linux_build_dependencies_sync(
    app: &AppHandle,
    server: &RemoteServerConfig,
    cargo_toolchain: &LocalRustToolchain,
) -> Result<(), String> {
    if !command_exists("zig") {
        install_zig_sync(app, server)?;
    }

    if !cargo_subcommand_available("zigbuild", cargo_toolchain) {
        emit_remote_deploy_progress(
            app,
            server,
            "preparingBuilder",
            20,
            Some("cargo install cargo-zigbuild --locked".to_string()),
        );
        let mut command = cargo_toolchain.new_cargo_command();
        command.args(["install", "cargo-zigbuild", "--locked"]);
        run_local_command(&mut command)
            .map_err(|error| format!("通过 cargo install 安装 cargo-zigbuild 失败: {error}"))?;
    }

    Ok(())
}

fn install_zig_sync(app: &AppHandle, server: &RemoteServerConfig) -> Result<(), String> {
    if command_exists("zig") {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        emit_remote_deploy_progress(
            app,
            server,
            "preparingBuilder",
            16,
            Some("brew install zig".to_string()),
        );
        ensure_command_available(
            "brew",
            "未检测到 Homebrew，请先安装 brew 后再自动安装 Zig。",
        )?;
        run_install_command("brew", &["install", "zig"], "通过 Homebrew 安装 Zig 失败")?;
    }

    #[cfg(target_os = "windows")]
    {
        emit_remote_deploy_progress(
            app,
            server,
            "preparingBuilder",
            16,
            Some("winget install --id zig.zig -e --silent".to_string()),
        );
        ensure_command_available(
            "winget",
            "当前平台暂未内置一键安装 cargo-zigbuild / Zig，请先手动安装。",
        )?;
        run_install_command(
            "winget",
            &["install", "--id", "zig.zig", "-e", "--silent"],
            "通过 winget 安装 Zig 失败",
        )?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if command_exists("apt-get") {
            emit_remote_deploy_progress(
                app,
                server,
                "preparingBuilder",
                16,
                Some("sudo apt-get update && sudo apt-get install -y zig".to_string()),
            );
            run_shell_install_command(
                "sudo apt-get update && sudo apt-get install -y zig",
                "通过 apt-get 安装 Zig 失败",
            )?;
        } else if command_exists("dnf") {
            emit_remote_deploy_progress(
                app,
                server,
                "preparingBuilder",
                16,
                Some("sudo dnf install -y zig".to_string()),
            );
            run_shell_install_command("sudo dnf install -y zig", "通过 dnf 安装 Zig 失败")?;
        } else if command_exists("yum") {
            emit_remote_deploy_progress(
                app,
                server,
                "preparingBuilder",
                16,
                Some("sudo yum install -y zig".to_string()),
            );
            run_shell_install_command("sudo yum install -y zig", "通过 yum 安装 Zig 失败")?;
        } else if command_exists("pacman") {
            emit_remote_deploy_progress(
                app,
                server,
                "preparingBuilder",
                16,
                Some("sudo pacman -Sy --noconfirm zig".to_string()),
            );
            run_shell_install_command(
                "sudo pacman -Sy --noconfirm zig",
                "通过 pacman 安装 Zig 失败",
            )?;
        } else {
            return Err(
                "当前平台暂未内置一键安装 cargo-zigbuild / Zig，请先手动安装。".to_string(),
            );
        }
    }

    if !command_exists("zig") {
        return Err("自动安装 Linux 构建依赖后仍未检测到可用的 cargo-zigbuild / zig。".to_string());
    }

    Ok(())
}

fn ensure_rust_target(target: &str, cargo_toolchain: &LocalRustToolchain) -> Result<(), String> {
    if !command_exists("rustup") {
        return Ok(());
    }

    let mut command = new_resolved_command("rustup");
    command.arg("target").arg("add");
    if let Some(toolchain) = &cargo_toolchain.rustup_toolchain {
        command.arg("--toolchain").arg(toolchain);
    }
    let status = command
        .arg(target)
        .status()
        .map_err(|error| format!("添加 Rust 目标失败: {error}"))?;
    if !status.success() {
        return Err(format!("添加 Rust 目标失败: {target}"));
    }
    Ok(())
}

fn run_local_command(command: &mut Command) -> Result<(), String> {
    let description = format!("{command:?}");
    let output = command
        .output()
        .map_err(|error| format!("执行本地命令失败 {description}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = summarize_command_output(&output.stderr);
    let stdout = summarize_command_output(&output.stdout);
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "命令返回非零状态".to_string()
    };
    Err(detail)
}

fn run_install_command(cmd: &str, args: &[&str], prefix: &str) -> Result<(), String> {
    let output = new_resolved_command(cmd)
        .args(args)
        .output()
        .map_err(|error| format!("{prefix}: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = summarize_command_output(&output.stderr);
    let stdout = summarize_command_output(&output.stdout);
    if !stderr.is_empty() {
        Err(format!("{prefix}: {stderr}"))
    } else if !stdout.is_empty() {
        Err(format!("{prefix}: {stdout}"))
    } else {
        Err(prefix.to_string())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn run_shell_install_command(script: &str, prefix: &str) -> Result<(), String> {
    let output = Command::new("sh")
        .arg("-lc")
        .arg(script)
        .output()
        .map_err(|error| format!("{prefix}: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = summarize_command_output(&output.stderr);
    let stdout = summarize_command_output(&output.stdout);
    if !stderr.is_empty() {
        Err(format!("{prefix}: {stderr}"))
    } else if !stdout.is_empty() {
        Err(format!("{prefix}: {stdout}"))
    } else {
        Err(prefix.to_string())
    }
}

fn summarize_command_output(output: &[u8]) -> String {
    let normalized = String::from_utf8_lossy(output).replace('\r', "\n");
    let lines = normalized
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let important_lines = lines
        .iter()
        .filter(|line| is_key_command_output_line(line))
        .cloned()
        .collect::<Vec<_>>();
    let mut lines = if important_lines.is_empty() {
        lines
    } else {
        important_lines
    };

    if lines.len() > 8 {
        let head = lines.iter().take(4).cloned();
        let tail = lines
            .iter()
            .rev()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev();
        lines = head
            .chain(std::iter::once("...".to_string()))
            .chain(tail)
            .collect();
    }

    let mut summary = lines.join(" | ");
    if summary.len() > 600 {
        summary.truncate(600);
        summary.push_str("...");
    }

    summary
}

fn is_key_command_output_line(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("error")
        || line.starts_with("Caused by:")
        || line.starts_with("note:")
        || line.starts_with("help:")
        || line.starts_with("= note:")
        || line.starts_with("= help:")
        || line.contains("could not compile")
        || line.contains("failed to run custom build command")
        || line.contains("can't find crate for")
        || line.contains("No such file or directory")
}

fn rustup_which(tool: &str) -> Option<PathBuf> {
    let output = new_resolved_command("rustup")
        .args(["which", tool])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn rustup_active_toolchain_name() -> Option<String> {
    let output = new_resolved_command("rustup")
        .args(["show", "active-toolchain"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .map(ToOwned::to_owned)
}

fn ensure_program_available(
    program: &Path,
    fallback_name: &str,
    message: &str,
) -> Result<(), String> {
    if program.is_absolute() {
        if program.is_file() {
            return Ok(());
        }
        return Err(message.to_string());
    }

    ensure_command_available(fallback_name, message)
}

fn run_root_ssh(server: &RemoteServerConfig, script: &str) -> Result<String, String> {
    if server.ssh_user == "root" {
        return run_ssh(server, script);
    }

    match server.auth_mode {
        RemoteAuthMode::Password => {
            let password = server
                .password
                .as_deref()
                .ok_or_else(|| "SSH 密码不能为空".to_string())?;
            run_ssh(
                server,
                &format!(
                    "printf '%s\\n' {password} | sudo -S -p '' sh -lc {script}",
                    password = shell_quote(password),
                    script = shell_quote(script),
                ),
            )
        }
        _ => run_ssh(server, &format!("sudo -n sh -lc {}", shell_quote(script))),
    }
}

fn run_ssh(server: &RemoteServerConfig, script: &str) -> Result<String, String> {
    let auth = PreparedAuth::new(server)?;
    let mut command = auth.new_command("ssh");
    append_ssh_common_args(&mut command, server, false, auth.identity_file_path());
    command
        .arg(remote_target(server))
        .arg(format!("sh -lc {}", shell_quote(script)));

    let output = command
        .output()
        .map_err(|error| format!("执行 ssh 命令失败: {error}"))?;
    if !output.status.success() {
        let stderr = summarize_ssh_output(&output.stderr);
        let stdout = summarize_ssh_output(&output.stdout);
        return Err(
            if is_ssh_auth_failure(&stderr) || is_ssh_auth_failure(&stdout) {
                format!(
                "SSH 登录失败，请检查远程服务器的用户名、认证方式与密码/私钥是否正确。当前目标: {}",
                remote_target(server)
            )
            } else if is_ssh_connection_closed(&stderr) || is_ssh_connection_closed(&stdout) {
                format!(
                "SSH 连接被远程服务器主动关闭，请检查 sshd 是否允许当前用户与认证方式。当前目标: {}",
                remote_target(server)
            )
            } else if !stderr.is_empty() {
                format!("ssh 命令返回非零状态: {stderr}")
            } else if !stdout.is_empty() {
                format!("ssh 命令返回非零状态: {stdout}")
            } else {
                "ssh 命令返回非零状态".to_string()
            },
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_scp(
    server: &RemoteServerConfig,
    local_path: &Path,
    remote_path: &str,
) -> Result<(), String> {
    let auth = PreparedAuth::new(server)?;
    let mut command = auth.new_command("scp");
    append_ssh_common_args(&mut command, server, true, auth.identity_file_path());
    command.arg(local_path);
    command.arg(format!("{}:{}", remote_target(server), remote_path));

    let output = command
        .output()
        .map_err(|error| format!("执行 scp 命令失败: {error}"))?;
    if !output.status.success() {
        let stderr = summarize_ssh_output(&output.stderr);
        let stdout = summarize_ssh_output(&output.stdout);
        return Err(
            if is_ssh_auth_failure(&stderr) || is_ssh_auth_failure(&stdout) {
                format!(
                "SSH 登录失败，请检查远程服务器的用户名、认证方式与密码/私钥是否正确。当前目标: {}",
                remote_target(server)
            )
            } else if is_ssh_connection_closed(&stderr) || is_ssh_connection_closed(&stdout) {
                format!(
                "SSH 连接被远程服务器主动关闭，请检查 sshd 是否允许当前用户与认证方式。当前目标: {}",
                remote_target(server)
            )
            } else if !stderr.is_empty() {
                format!("scp 命令返回非零状态: {stderr}")
            } else if !stdout.is_empty() {
                format!("scp 命令返回非零状态: {stdout}")
            } else {
                "scp 命令返回非零状态".to_string()
            },
        );
    }
    Ok(())
}

fn append_ssh_common_args(
    command: &mut Command,
    server: &RemoteServerConfig,
    scp_mode: bool,
    identity_path: Option<&Path>,
) {
    if scp_mode {
        command.arg("-P").arg(server.ssh_port.to_string());
    } else {
        command.arg("-p").arg(server.ssh_port.to_string());
    }
    command.arg("-o").arg("StrictHostKeyChecking=accept-new");
    match server.auth_mode {
        RemoteAuthMode::Password => {
            command.arg("-o").arg("BatchMode=no");
            command
                .arg("-o")
                .arg("PreferredAuthentications=password,keyboard-interactive");
            command.arg("-o").arg("PubkeyAuthentication=no");
            command.arg("-o").arg("NumberOfPasswordPrompts=1");
        }
        _ => {
            command.arg("-o").arg("BatchMode=yes");
            if let Some(path) = identity_path {
                command.arg("-i").arg(path);
            }
        }
    }
}

fn remote_target(server: &RemoteServerConfig) -> String {
    format!("{}@{}", server.ssh_user, server.host)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn join_posix_path(base: &str, leaf: &str) -> String {
    if base == "/" {
        format!("/{leaf}")
    } else {
        format!("{}/{}", base.trim_end_matches('/'), leaf)
    }
}

fn remote_systemd_service_name(server: &RemoteServerConfig) -> String {
    let raw = if server.id.trim().is_empty() {
        server.label.as_str()
    } else {
        server.id.as_str()
    };
    format!(
        "codex-tools-proxyd-{}.service",
        sanitize_service_fragment(raw)
    )
}

fn sanitize_service_fragment(value: &str) -> String {
    let mut slug = value
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() {
                char.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        slug = "node".to_string();
    }
    if slug.len() > 24 {
        slug.truncate(24);
    }
    slug
}

struct RemotePlatform {
    primary_target: &'static str,
    fallback_target: &'static str,
}

struct PreparedAuth {
    temp_identity_file: Option<PathBuf>,
    password: Option<String>,
}

impl PreparedAuth {
    fn new(server: &RemoteServerConfig) -> Result<Self, String> {
        match server.auth_mode {
            RemoteAuthMode::KeyContent => {
                let private_key = server
                    .private_key
                    .as_deref()
                    .ok_or_else(|| "SSH 私钥内容不能为空".to_string())?;
                let temp_path = env::temp_dir().join(format!(
                    "codex-tools-key-{}-{}.pem",
                    sanitize_service_fragment(&server.id),
                    now_unix_seconds()
                ));
                // 使用 private_create_new_options 确保文件从创建时就是 0o600 权限，
                // 消除 fs::write + set_permissions 两步之间的 TOCTOU 窗口
                {
                    use std::io::Write as _;
                    let mut f = crate::utils::private_create_new_options()
                        .open(&temp_path)
                        .map_err(|error| {
                            format!("写入临时 SSH 私钥文件失败 {}: {error}", temp_path.display())
                        })?;
                    f.write_all(private_key.as_bytes()).map_err(|error| {
                        format!("写入临时 SSH 私钥文件失败 {}: {error}", temp_path.display())
                    })?;
                }
                Ok(Self {
                    temp_identity_file: Some(temp_path),
                    password: None,
                })
            }
            RemoteAuthMode::KeyFile | RemoteAuthMode::KeyPath => Ok(Self {
                temp_identity_file: server.identity_file.as_ref().map(PathBuf::from),
                password: None,
            }),
            RemoteAuthMode::Password => Ok(Self {
                temp_identity_file: None,
                password: server.password.clone(),
            }),
        }
    }

    fn identity_file_path(&self) -> Option<&Path> {
        self.temp_identity_file.as_deref()
    }

    fn new_command(&self, program: &str) -> Command {
        let program_path = find_command_path(program).unwrap_or_else(|| PathBuf::from(program));
        let mut command = if let Some(password) = self.password.as_deref() {
            let mut command = new_resolved_command("sshpass");
            command.arg("-p").arg(password).arg(&program_path);
            command
        } else {
            new_resolved_command(program)
        };
        command.env("SSH_ASKPASS_REQUIRE", "never");
        command.env_remove("SSH_ASKPASS");
        command.env_remove("DISPLAY");
        command
    }
}

impl Drop for PreparedAuth {
    fn drop(&mut self) {
        if let Some(path) = self.temp_identity_file.as_ref() {
            let _ = fs::remove_file(path);
        }
    }
}

fn summarize_ssh_output(output: &[u8]) -> String {
    let normalized = String::from_utf8_lossy(output).replace('\r', "\n");
    let filtered = normalized
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("** WARNING: connection is not using a post-quantum"))
        .filter(|line| !line.starts_with("** This session may be vulnerable"))
        .filter(|line| !line.starts_with("** The server may need to be upgraded"))
        .filter(|line| !line.contains("https://openssh.com/pq.html"))
        .filter(|line| !line.starts_with("ssh_askpass:"))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    summarize_command_output(filtered.join("\n").as_bytes())
}

fn is_ssh_auth_failure(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    normalized.contains("permission denied")
        || normalized.contains("please try again")
        || normalized.contains("publickey,password")
        || normalized.contains("publickey,gssapi-keyex,gssapi-with-mic,password")
}

fn is_ssh_connection_closed(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    normalized.contains("connection closed by")
        || normalized.contains("connection reset by")
        || normalized.contains("kex_exchange_identification")
}
