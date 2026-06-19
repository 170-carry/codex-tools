use std::env;
use std::fs;
use std::io;
use std::io::IsTerminal;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use clap::Subcommand;
use serde::Serialize;
use tokio::sync::Mutex;

use crate::account_service;
use crate::app_paths;
use crate::auth;
use crate::cli;
use crate::models::AccountSourceKind;
use crate::models::AccountSummary;
use crate::models::AccountsStore;
use crate::models::AuthJsonImportInput;
use crate::models::ImportAccountsResult;
use crate::models::StoredAccount;
use crate::profile_files;
use crate::store;
use crate::token_usage;
use crate::usage;
use crate::utils;

const CLI_COMMANDS: &[&str] = &[
    "list", "switch", "login", "import", "export", "usage", "doctor", "report", "tui",
];

#[derive(Debug, Parser)]
#[command(
    name = "codex-tools",
    version,
    about = "Codex Tools CLI/TUI account manager"
)]
struct CliArgs {
    #[arg(long, global = true, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// List stored accounts.
    List {
        #[arg(long)]
        refresh: bool,
    },
    /// Switch ~/.codex/auth.json to one stored account.
    Switch {
        account: Option<String>,
        #[arg(long)]
        best: bool,
        #[arg(long)]
        launch: bool,
        #[arg(long, value_name = "DIR")]
        workspace: Option<PathBuf>,
    },
    /// Run official `codex login`, then import the resulting auth.json.
    Login {
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        device_auth: bool,
    },
    /// Import auth/account JSON files or the current ~/.codex/auth.json.
    Import {
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
        #[arg(long)]
        current: bool,
        #[arg(long)]
        label: Option<String>,
    },
    /// Export stored accounts as JSON.
    Export {
        #[arg(value_name = "PATH")]
        output: Option<PathBuf>,
        #[arg(long, value_name = "ACCOUNT")]
        account: Option<String>,
    },
    /// Refresh or show usage for stored accounts.
    Usage {
        account: Option<String>,
        #[arg(long)]
        cached: bool,
    },
    /// Check local paths, Codex CLI, auth files, and account store.
    Doctor,
    /// Print a fuller machine-readable diagnostic report.
    Report,
    /// Open a simple terminal account selector.
    Tui {
        #[arg(long)]
        refresh: bool,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliSwitchResult {
    account: AccountSummary,
    active_auth_path: String,
    active_config_path: String,
    launched_codex: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliLoginResult {
    import_result: ImportAccountsResult,
    account_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliExportResult {
    account_count: usize,
    output_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorCheck {
    name: String,
    ok: bool,
    detail: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorReport {
    ok: bool,
    data_dir: String,
    store_path: String,
    codex_dir: Option<String>,
    codex_auth_path: Option<String>,
    codex_config_path: Option<String>,
    codex_cli_path: Option<String>,
    account_count: usize,
    active_account_id: Option<String>,
    current_account_key: Option<String>,
    checks: Vec<DoctorCheck>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliFullReport {
    doctor: DoctorReport,
    accounts: Vec<AccountSummary>,
    token_usage: Option<token_usage::CodexTokenUsageSnapshot>,
    token_usage_error: Option<String>,
}

pub(crate) fn try_run_from_env() -> bool {
    let args = env::args_os().collect::<Vec<_>>();
    if !is_cli_invocation(&args) {
        return false;
    }

    let code = run_from_os_args(args);
    std::process::exit(code);
}

pub(crate) fn run_from_env_or_exit() -> ! {
    let code = run_from_os_args(env::args_os().collect());
    std::process::exit(code);
}

fn is_cli_invocation(args: &[std::ffi::OsString]) -> bool {
    let Some(first) = args.get(1).and_then(|value| value.to_str()) else {
        return false;
    };

    first == "cli"
        || first == "--help"
        || first == "-h"
        || first == "--version"
        || first == "-V"
        || CLI_COMMANDS.contains(&first)
}

fn run_from_os_args(args: Vec<std::ffi::OsString>) -> i32 {
    let normalized_args = normalize_cli_args(args);
    let parsed = match CliArgs::try_parse_from(normalized_args) {
        Ok(value) => value,
        Err(error) => {
            let _ = error.print();
            return i32::from(error.exit_code());
        }
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("error: 创建 CLI 运行时失败: {error}");
            return 1;
        }
    };

    match runtime.block_on(run_cli(parsed)) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("error: {error}");
            1
        }
    }
}

fn normalize_cli_args(mut args: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    if args.get(1).and_then(|value| value.to_str()) == Some("cli") {
        args.remove(1);
    }
    args
}

async fn run_cli(args: CliArgs) -> Result<(), String> {
    let store_path = resolve_store_path(args.data_dir.as_deref())?;
    match args.command {
        CliCommand::List { refresh } => {
            let accounts = if refresh {
                refresh_usage(&store_path, None).await?
            } else {
                account_summaries(&store::load_store_from_path(&store_path)?)
            };
            print_accounts(&accounts, args.json)
        }
        CliCommand::Switch {
            account,
            best,
            launch,
            workspace,
        } => {
            let result = switch_account(
                &store_path,
                account.as_deref(),
                best,
                launch,
                workspace.as_deref(),
            )
            .await?;
            if args.json {
                print_json(&result)
            } else {
                println!(
                    "switched to {} ({})",
                    result.account.label,
                    utils::short_account(&result.account.account_id)
                );
                if result.launched_codex {
                    println!("launched codex app");
                }
                Ok(())
            }
        }
        CliCommand::Login { label, device_auth } => {
            let result = login_account(&store_path, label, device_auth).await?;
            if args.json {
                print_json(&result)
            } else {
                println!(
                    "login imported {} account(s), updated {} account(s)",
                    result.import_result.imported_count, result.import_result.updated_count
                );
                print_failures(&result.import_result);
                Ok(())
            }
        }
        CliCommand::Import {
            paths,
            current,
            label,
        } => {
            let result = import_accounts(&store_path, paths, current, label).await?;
            if args.json {
                print_json(&result)
            } else {
                println!(
                    "imported {} account(s), updated {} account(s)",
                    result.imported_count, result.updated_count
                );
                print_failures(&result);
                Ok(())
            }
        }
        CliCommand::Export { output, account } => {
            let result = export_accounts(&store_path, output.as_deref(), account.as_deref())?;
            if result.output_path.is_none() {
                return Ok(());
            }
            if args.json {
                print_json(&result)
            } else if let Some(output_path) = result.output_path.as_deref() {
                println!(
                    "exported {} account(s) to {output_path}",
                    result.account_count
                );
                Ok(())
            } else {
                Ok(())
            }
        }
        CliCommand::Usage { account, cached } => {
            let accounts = if cached {
                selected_summaries_from_store(
                    &store::load_store_from_path(&store_path)?,
                    account.as_deref(),
                )?
            } else {
                refresh_usage(&store_path, account.as_deref()).await?
            };
            print_accounts(&accounts, args.json)
        }
        CliCommand::Doctor => {
            let report = build_doctor_report(&store_path)?;
            if args.json {
                print_json(&report)
            } else {
                print_doctor_report(&report);
                Ok(())
            }
        }
        CliCommand::Report => {
            let report = build_full_report(&store_path)?;
            if args.json {
                print_json(&report)
            } else {
                print_doctor_report(&report.doctor);
                println!();
                print_accounts(&report.accounts, false)?;
                if let Some(token_usage) = report.token_usage.as_ref() {
                    println!();
                    println!(
                        "token_usage events={} files={} failed_paths={}",
                        token_usage.event_count,
                        token_usage.source_path_count,
                        token_usage.failed_path_count
                    );
                }
                if let Some(error) = report.token_usage_error.as_deref() {
                    println!("token_usage_error={error}");
                }
                Ok(())
            }
        }
        CliCommand::Tui { refresh } => run_tui(&store_path, refresh).await,
    }
}

fn resolve_store_path(data_dir: Option<&Path>) -> Result<PathBuf, String> {
    let data_dir = match data_dir {
        Some(path) => path.to_path_buf(),
        None => app_paths::app_data_dir_without_tauri()?,
    };
    Ok(store::account_store_path_from_data_dir(&data_dir))
}

fn account_summaries(store: &AccountsStore) -> Vec<AccountSummary> {
    let current_account_key = auth::current_auth_account_key();
    let current_variant_key = auth::current_auth_variant_key();
    let mut summaries = store
        .accounts
        .iter()
        .map(|account| {
            account.to_summary(
                current_account_key.as_deref(),
                current_variant_key.as_deref(),
            )
        })
        .collect::<Vec<_>>();

    if !summaries.iter().any(|account| account.is_current) {
        if let Some(active_id) = store.settings.active_account_id.as_deref() {
            if let Some(account) = summaries.iter_mut().find(|account| account.id == active_id) {
                account.is_current = true;
            }
        }
    }

    summaries
}

fn selected_summaries_from_store(
    store: &AccountsStore,
    account_ref: Option<&str>,
) -> Result<Vec<AccountSummary>, String> {
    let summaries = account_summaries(store);
    if account_ref.is_none() {
        return Ok(summaries);
    }
    let selected = resolve_account_indices(store, &summaries, account_ref, false)?;
    Ok(selected
        .into_iter()
        .filter_map(|index| summaries.get(index).cloned())
        .collect())
}

async fn refresh_usage(
    store_path: &Path,
    account_ref: Option<&str>,
) -> Result<Vec<AccountSummary>, String> {
    let mut store = store::load_store_from_path(store_path)?;
    let base_summaries = account_summaries(&store);
    let target_indices = resolve_account_indices(&store, &base_summaries, account_ref, true)?;
    let refresh_lock = Arc::new(Mutex::new(()));
    let now = utils::now_unix_seconds();

    for index in target_indices {
        let Some(account) = store.accounts.get_mut(index) else {
            continue;
        };
        refresh_one_account_usage(account, &refresh_lock, now).await;
        if let Err(error) = profile_files::sync_account_profile_in_store_path(store_path, account) {
            account.profile_integrity_error = Some(error);
        }
    }

    store::save_store_to_path(store_path, &store)?;
    selected_summaries_from_store(&store, account_ref)
}

async fn refresh_one_account_usage(
    account: &mut StoredAccount,
    refresh_lock: &Arc<Mutex<()>>,
    now: i64,
) {
    if matches!(account.source_kind, AccountSourceKind::Relay) {
        return;
    }

    if account.auth_refresh_blocked {
        account.usage_error = account
            .auth_refresh_error
            .clone()
            .or_else(|| Some("工具保存的授权快照已失效，请重新登录授权。".to_string()));
        return;
    }

    if auth::auth_tokens_need_refresh(&account.auth_json) {
        match auth::refresh_chatgpt_auth_tokens_serialized(&account.auth_json, refresh_lock).await {
            Ok(refreshed) => {
                account.auth_json = refreshed;
                account.auth_refresh_blocked = false;
                account.auth_refresh_error = None;
            }
            Err(error) => {
                let message = normalize_cli_usage_error(&error);
                account.usage_error = Some(message.clone());
                if is_cli_auth_expired_error(&error) {
                    account.auth_refresh_blocked = true;
                    account.auth_refresh_error = Some(message);
                }
                account.updated_at = now;
                return;
            }
        }
    }

    match auth::extract_auth(&account.auth_json) {
        Ok(extracted) => {
            account.email = extracted.email.or_else(|| account.email.clone());
            account.plan_type = extracted.plan_type.or_else(|| account.plan_type.clone());
            match usage::fetch_usage_snapshot(&extracted.access_token, &extracted.account_id).await
            {
                Ok(snapshot) => {
                    account.plan_type = snapshot
                        .plan_type
                        .clone()
                        .or_else(|| account.plan_type.clone());
                    account.usage = Some(snapshot);
                    account.usage_error = None;
                }
                Err(error) => {
                    account.usage_error = Some(normalize_cli_usage_error(&error));
                }
            }
        }
        Err(error) => {
            account.usage_error = Some(normalize_cli_usage_error(&error));
        }
    }
    account.updated_at = now;
}

async fn switch_account(
    store_path: &Path,
    account_ref: Option<&str>,
    best: bool,
    launch: bool,
    workspace: Option<&Path>,
) -> Result<CliSwitchResult, String> {
    let mut store = store::load_store_from_path(store_path)?;
    let summaries = account_summaries(&store);
    let selected_index = resolve_single_account_index(&store, &summaries, account_ref, best)?;
    let refresh_lock = Arc::new(Mutex::new(()));

    {
        let account = store
            .accounts
            .get_mut(selected_index)
            .ok_or_else(|| "找不到要切换的账号".to_string())?;

        if matches!(account.source_kind, AccountSourceKind::Chatgpt)
            && auth::auth_tokens_need_refresh(&account.auth_json)
        {
            if account.auth_refresh_blocked {
                return Err(format!(
                    "切换账号前刷新登录令牌失败: {}",
                    account
                        .auth_refresh_error
                        .clone()
                        .unwrap_or_else(|| "工具保存的授权快照已失效，请重新登录授权。".to_string())
                ));
            }

            match auth::refresh_chatgpt_auth_tokens_serialized(&account.auth_json, &refresh_lock)
                .await
            {
                Ok(refreshed) => {
                    account.auth_json = refreshed;
                    account.auth_refresh_blocked = false;
                    account.auth_refresh_error = None;
                    account.updated_at = utils::now_unix_seconds();
                }
                Err(error) => {
                    return Err(format!(
                        "切换账号前刷新登录令牌失败: {}",
                        normalize_cli_usage_error(&error)
                    ));
                }
            }
        }

        profile_files::sync_account_profile_in_store_path(store_path, account)?;
        profile_files::apply_account_profile(account)?;
        store.settings.active_account_id = Some(account.id.clone());
    }

    let configured_codex_launch_path = store.settings.codex_launch_path.clone();
    store::save_store_to_path(store_path, &store)?;
    if launch {
        launch_codex(configured_codex_launch_path.as_deref(), workspace)?;
    }

    let summaries = account_summaries(&store);
    let account = summaries
        .get(selected_index)
        .cloned()
        .ok_or_else(|| "切换后读取账号摘要失败".to_string())?;
    Ok(CliSwitchResult {
        account,
        active_auth_path: app_paths::codex_auth_path()?.to_string_lossy().to_string(),
        active_config_path: app_paths::codex_config_path()?
            .to_string_lossy()
            .to_string(),
        launched_codex: launch,
    })
}

fn launch_codex(configured_path: Option<&str>, workspace: Option<&Path>) -> Result<(), String> {
    let mut command = cli::new_codex_command(configured_path)?;
    command.arg("app");
    if let Some(workspace) = workspace {
        command.arg(workspace);
    }
    command
        .spawn()
        .map_err(|error| format!("启动 codex app 失败: {error}"))?;
    Ok(())
}

async fn login_account(
    store_path: &Path,
    label: Option<String>,
    device_auth: bool,
) -> Result<CliLoginResult, String> {
    let store_before_login = store::load_store_from_path(store_path)?;
    let mut command = cli::new_codex_foreground_command(
        store_before_login.settings.codex_launch_path.as_deref(),
    )?;
    command.arg("login");
    if device_auth {
        command.arg("--device-auth");
    }
    let status = command
        .status()
        .map_err(|error| format!("启动 codex login 失败: {error}"))?;
    if !status.success() {
        return Err(format!("codex login 退出状态异常: {status}"));
    }

    let auth_json = auth::read_current_codex_auth()?;
    let content = serde_json::to_string(&auth_json)
        .map_err(|error| format!("序列化 codex login 结果失败: {error}"))?;
    let import_result = account_service::import_auth_json_accounts_into_store_path(
        store_path,
        vec![AuthJsonImportInput {
            source: "codex login".to_string(),
            content,
            label,
        }],
    )
    .await?;
    let account_count = store::load_store_from_path(store_path)?.accounts.len();
    Ok(CliLoginResult {
        import_result,
        account_count,
    })
}

async fn import_accounts(
    store_path: &Path,
    paths: Vec<PathBuf>,
    current: bool,
    label: Option<String>,
) -> Result<ImportAccountsResult, String> {
    let mut items = Vec::new();
    if current {
        let content = serde_json::to_string(&auth::read_current_codex_auth()?)
            .map_err(|error| format!("序列化当前 auth.json 失败: {error}"))?;
        items.push(AuthJsonImportInput {
            source: app_paths::codex_auth_path()?.to_string_lossy().to_string(),
            content,
            label: label.clone(),
        });
    }

    for path in expand_import_paths(paths)? {
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("读取导入文件失败 {}: {error}", path.display()))?;
        items.push(AuthJsonImportInput {
            source: path.to_string_lossy().to_string(),
            content,
            label: label.clone(),
        });
    }

    account_service::import_auth_json_accounts_into_store_path(store_path, items).await
}

fn expand_import_paths(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
    let mut expanded = Vec::new();
    for path in paths {
        if path.is_file() {
            expanded.push(path);
            continue;
        }
        if path.is_dir() {
            let mut entries = fs::read_dir(&path)
                .map_err(|error| format!("读取导入目录失败 {}: {error}", path.display()))?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|entry| {
                    entry.is_file()
                        && entry
                            .extension()
                            .and_then(|value| value.to_str())
                            .map(|value| value.eq_ignore_ascii_case("json"))
                            .unwrap_or(false)
                })
                .collect::<Vec<_>>();
            entries.sort();
            expanded.extend(entries);
            continue;
        }
        return Err(format!("导入路径不存在: {}", path.display()));
    }
    Ok(expanded)
}

fn export_accounts(
    store_path: &Path,
    output: Option<&Path>,
    account_ref: Option<&str>,
) -> Result<CliExportResult, String> {
    let mut store = store::load_store_from_path(store_path)?;
    if let Some(account_ref) = account_ref {
        let summaries = account_summaries(&store);
        let selected = resolve_account_indices(&store, &summaries, Some(account_ref), false)?;
        store.accounts = selected
            .into_iter()
            .filter_map(|index| store.accounts.get(index).cloned())
            .collect();
    }
    let account_count = store.accounts.len();
    let serialized = serde_json::to_string_pretty(&store)
        .map_err(|error| format!("序列化导出账号失败: {error}"))?;

    if let Some(output) = output {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("创建导出目录失败 {}: {error}", parent.display()))?;
        }
        fs::write(output, serialized.as_bytes())
            .map_err(|error| format!("写入导出文件失败 {}: {error}", output.display()))?;
        let _ = utils::set_private_permissions(output);
        Ok(CliExportResult {
            account_count,
            output_path: Some(output.to_string_lossy().to_string()),
        })
    } else {
        println!("{serialized}");
        Ok(CliExportResult {
            account_count,
            output_path: None,
        })
    }
}

fn resolve_single_account_index(
    store: &AccountsStore,
    summaries: &[AccountSummary],
    account_ref: Option<&str>,
    best: bool,
) -> Result<usize, String> {
    if best {
        if account_ref.is_some() {
            return Err("不能同时使用账号参数和 --best".to_string());
        }
        return pick_best_account_index(&store.accounts);
    }

    let indices = resolve_account_indices(store, summaries, account_ref, false)?;
    match indices.as_slice() {
        [index] => Ok(*index),
        [] => Err("没有可切换的账号".to_string()),
        _ => Err("账号选择结果不唯一".to_string()),
    }
}

fn resolve_account_indices(
    store: &AccountsStore,
    summaries: &[AccountSummary],
    account_ref: Option<&str>,
    allow_all: bool,
) -> Result<Vec<usize>, String> {
    let Some(raw_ref) = account_ref.map(str::trim).filter(|value| !value.is_empty()) else {
        return if allow_all || !store.accounts.is_empty() {
            Ok((0..store.accounts.len()).collect())
        } else {
            Err("账号列表为空".to_string())
        };
    };

    if let Ok(display_index) = raw_ref.parse::<usize>() {
        if display_index > 0 && display_index <= store.accounts.len() {
            return Ok(vec![display_index - 1]);
        }
    }

    let exact = store
        .accounts
        .iter()
        .enumerate()
        .filter(|(_, account)| account_exact_match(account, raw_ref))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if exact.len() == 1 {
        return Ok(exact);
    }
    if exact.len() > 1 {
        return Err(format!(
            "账号匹配不唯一: {}",
            format_candidate_summaries(summaries, &exact)
        ));
    }

    let partial = store
        .accounts
        .iter()
        .enumerate()
        .filter(|(_, account)| account_partial_match(account, raw_ref))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if partial.len() == 1 {
        Ok(partial)
    } else if partial.is_empty() {
        Err(format!("找不到账号: {raw_ref}"))
    } else {
        Err(format!(
            "账号匹配不唯一: {}",
            format_candidate_summaries(summaries, &partial)
        ))
    }
}

fn account_exact_match(account: &StoredAccount, raw_ref: &str) -> bool {
    let account_key = account.account_key();
    account.id == raw_ref
        || account.account_id == raw_ref
        || account_key == raw_ref
        || account.label.eq_ignore_ascii_case(raw_ref)
        || account
            .email
            .as_deref()
            .map(|email| email.eq_ignore_ascii_case(raw_ref))
            .unwrap_or(false)
}

fn account_partial_match(account: &StoredAccount, raw_ref: &str) -> bool {
    let query = raw_ref.to_ascii_lowercase();
    let account_key = account.account_key().to_ascii_lowercase();
    account.id.to_ascii_lowercase().starts_with(&query)
        || account.account_id.to_ascii_lowercase().starts_with(&query)
        || account_key.starts_with(&query)
        || account.label.to_ascii_lowercase().contains(&query)
        || account
            .email
            .as_deref()
            .map(|email| email.to_ascii_lowercase().contains(&query))
            .unwrap_or(false)
}

fn pick_best_account_index(accounts: &[StoredAccount]) -> Result<usize, String> {
    accounts
        .iter()
        .enumerate()
        .min_by_key(|(_, account)| account_rank(account))
        .map(|(index, _)| index)
        .ok_or_else(|| "账号列表为空".to_string())
}

fn account_rank(account: &StoredAccount) -> (u8, u8, i64, i64, i64) {
    let blocked = u8::from(account.auth_refresh_blocked);
    let has_error = u8::from(account.usage_error.is_some());
    let five_hour = usage_percent_millis(
        account
            .usage
            .as_ref()
            .and_then(|usage| usage.five_hour.as_ref()),
    );
    let one_week = usage_percent_millis(
        account
            .usage
            .as_ref()
            .and_then(|usage| usage.one_week.as_ref()),
    );
    (blocked, has_error, one_week, five_hour, -account.updated_at)
}

fn usage_percent_millis(window: Option<&crate::models::UsageWindow>) -> i64 {
    window
        .map(|window| (window.used_percent * 1000.0).round() as i64)
        .unwrap_or(i64::MAX)
}

fn format_candidate_summaries(summaries: &[AccountSummary], indices: &[usize]) -> String {
    indices
        .iter()
        .filter_map(|index| summaries.get(*index).map(|summary| (*index + 1, summary)))
        .map(|(index, summary)| format!("{index}:{}:{}", summary.id, summary.label))
        .collect::<Vec<_>>()
        .join(", ")
}

fn build_doctor_report(store_path: &Path) -> Result<DoctorReport, String> {
    let data_dir = store_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut checks = Vec::new();

    let store_result = store::load_store_from_path(store_path);
    checks.push(DoctorCheck {
        name: "store".to_string(),
        ok: store_result.is_ok(),
        detail: store_result
            .as_ref()
            .map(|store| format!("{} account(s)", store.accounts.len()))
            .unwrap_or_else(|error| error.clone()),
    });
    let store = store_result.unwrap_or_default();

    let codex_dir = app_paths::codex_dir().ok();
    let auth_path = app_paths::codex_auth_path().ok();
    let config_path = app_paths::codex_config_path().ok();
    checks.push(path_check("dataDir", &data_dir, true));
    if let Some(path) = auth_path.as_ref() {
        checks.push(path_check("codexAuth", path, false));
    }
    if let Some(path) = config_path.as_ref() {
        checks.push(path_check("codexConfig", path, false));
    }

    let codex_cli = cli::new_codex_command(store.settings.codex_launch_path.as_deref())
        .map(|command| command.get_program().to_string_lossy().to_string());
    checks.push(DoctorCheck {
        name: "codexCli".to_string(),
        ok: codex_cli.is_ok(),
        detail: codex_cli
            .as_ref()
            .cloned()
            .unwrap_or_else(|error| error.clone()),
    });

    let current_account_key = auth::current_auth_account_key();
    let ok = checks.iter().all(|check| check.ok);
    Ok(DoctorReport {
        ok,
        data_dir: data_dir.to_string_lossy().to_string(),
        store_path: store_path.to_string_lossy().to_string(),
        codex_dir: codex_dir.map(|path| path.to_string_lossy().to_string()),
        codex_auth_path: auth_path.map(|path| path.to_string_lossy().to_string()),
        codex_config_path: config_path.map(|path| path.to_string_lossy().to_string()),
        codex_cli_path: codex_cli.ok(),
        account_count: store.accounts.len(),
        active_account_id: store.settings.active_account_id,
        current_account_key,
        checks,
    })
}

fn path_check(name: &str, path: &Path, directory: bool) -> DoctorCheck {
    let ok = if directory {
        path.is_dir()
    } else {
        path.is_file()
    };
    DoctorCheck {
        name: name.to_string(),
        ok,
        detail: path.to_string_lossy().to_string(),
    }
}

fn build_full_report(store_path: &Path) -> Result<CliFullReport, String> {
    let doctor = build_doctor_report(store_path)?;
    let accounts = store::load_store_from_path(store_path)
        .map(|store| account_summaries(&store))
        .unwrap_or_default();
    let (token_usage, token_usage_error) = match token_usage::collect_codex_token_usage_snapshot() {
        Ok(snapshot) => (Some(snapshot), None),
        Err(error) => (None, Some(error)),
    };
    Ok(CliFullReport {
        doctor,
        accounts,
        token_usage,
        token_usage_error,
    })
}

async fn run_tui(store_path: &Path, refresh: bool) -> Result<(), String> {
    if !io::stdin().is_terminal() {
        return Err("TUI 需要在交互式终端中运行".to_string());
    }
    let accounts = if refresh {
        refresh_usage(store_path, None).await?
    } else {
        account_summaries(&store::load_store_from_path(store_path)?)
    };
    print_accounts(&accounts, false)?;
    print!("选择账号序号: ");
    io::stdout()
        .flush()
        .map_err(|error| format!("刷新终端输出失败: {error}"))?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("读取终端输入失败: {error}"))?;
    let selected = input.trim();
    if selected.is_empty() {
        return Err("未选择账号".to_string());
    }
    let result = switch_account(store_path, Some(selected), false, false, None).await?;
    println!(
        "switched to {} ({})",
        result.account.label,
        utils::short_account(&result.account.account_id)
    );
    Ok(())
}

fn print_accounts(accounts: &[AccountSummary], json: bool) -> Result<(), String> {
    if json {
        return print_json(accounts);
    }
    println!(
        "{:<4} {:<1} {:<22} {:<28} {:<10} {:>8} {:>8} {}",
        "#", "*", "id", "label", "plan", "5h", "1w", "email/error"
    );
    for (index, account) in accounts.iter().enumerate() {
        let marker = if account.is_current { "*" } else { "" };
        let detail = account
            .usage_error
            .as_deref()
            .or(account.auth_refresh_error.as_deref())
            .or(account.email.as_deref())
            .unwrap_or("");
        println!(
            "{:<4} {:<1} {:<22} {:<28} {:<10} {:>8} {:>8} {}",
            index + 1,
            marker,
            utils::short_account(&account.account_id),
            truncate_cell(&account.label, 28),
            truncate_cell(account.plan_type.as_deref().unwrap_or("-"), 10),
            format_window_percent(
                account
                    .usage
                    .as_ref()
                    .and_then(|usage| usage.five_hour.as_ref())
            ),
            format_window_percent(
                account
                    .usage
                    .as_ref()
                    .and_then(|usage| usage.one_week.as_ref())
            ),
            detail
        );
    }
    Ok(())
}

fn print_doctor_report(report: &DoctorReport) {
    println!("ok={}", report.ok);
    println!("data_dir={}", report.data_dir);
    println!("store_path={}", report.store_path);
    println!("account_count={}", report.account_count);
    if let Some(codex_cli_path) = report.codex_cli_path.as_deref() {
        println!("codex_cli={codex_cli_path}");
    }
    for check in &report.checks {
        println!("check.{}={} {}", check.name, check.ok, check.detail);
    }
}

fn print_failures(result: &ImportAccountsResult) {
    for failure in &result.failures {
        eprintln!("failed {}: {}", failure.source, failure.error);
    }
}

fn print_json<T: Serialize + ?Sized>(value: &T) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(value)
        .map_err(|error| format!("序列化 JSON 失败: {error}"))?;
    println!("{serialized}");
    Ok(())
}

fn format_window_percent(window: Option<&crate::models::UsageWindow>) -> String {
    window
        .map(|window| format!("{:.0}%", window.used_percent))
        .unwrap_or_else(|| "-".to_string())
}

fn truncate_cell(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let mut text = value.chars().take(keep).collect::<String>();
    text.push('…');
    text
}

fn normalize_cli_usage_error(raw_error: &str) -> String {
    let normalized = raw_error.to_ascii_lowercase();
    if normalized.contains("deactivated_workspace") {
        return "该账号已被踢出 team 组织，请重新授权后再刷新。".to_string();
    }
    if normalized.contains("your openai account has been deactivated")
        || normalized.contains("account has been deactivated")
        || normalized.contains("account deactivated")
        || normalized.contains("deactivated_user")
    {
        return "账号被封禁，请检查邮箱".to_string();
    }
    if is_cli_auth_expired_error(raw_error) {
        return "工具保存的授权快照已失效，请重新登录授权。".to_string();
    }
    raw_error.to_string()
}

fn is_cli_auth_expired_error(raw_error: &str) -> bool {
    let normalized = raw_error.to_ascii_lowercase();
    (normalized.contains("invalid_grant")
        && (normalized.contains("refresh")
            || normalized.contains("expired")
            || normalized.contains("revoked")
            || normalized.contains("invalid")))
        || normalized.contains("refresh_token_reused")
        || normalized.contains("provided authentication token is expired")
        || normalized
            .contains("your refresh token has already been used to generate a new access token")
        || normalized.contains("refresh token expired")
        || normalized.contains("refresh_token expired")
        || normalized.contains("expired refresh token")
        || normalized.contains("refresh token is expired")
        || normalized.contains("refresh token revoked")
        || normalized.contains("refresh_token_revoked")
        || normalized.contains("invalid refresh token")
        || normalized.contains("please try signing in again")
        || normalized.contains("token is expired")
        || normalized.contains("auth.json 缺少 refresh_token")
}

#[cfg(test)]
mod tests {
    use super::is_cli_invocation;
    use std::ffi::OsString;

    #[test]
    fn detects_direct_and_prefixed_cli_invocations() {
        assert!(is_cli_invocation(&[
            OsString::from("codex-tools"),
            OsString::from("cli"),
            OsString::from("list"),
        ]));
        assert!(is_cli_invocation(&[
            OsString::from("codex-tools"),
            OsString::from("list"),
        ]));
        assert!(!is_cli_invocation(&[OsString::from("codex-tools")]));
    }
}
