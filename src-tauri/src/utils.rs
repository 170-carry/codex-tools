use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

pub(crate) fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

pub(crate) fn short_account(account_id: &str) -> String {
    account_id.chars().take(8).collect()
}

pub(crate) fn truncate_for_error(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        value.to_string()
    } else {
        format!("{}...", &value[..max_len])
    }
}

pub(crate) fn set_private_permissions(path: &Path) {
    let _ = try_set_private_permissions(path);
}

/// 构造“仅新建”的私有文件选项；Unix 必须在创建瞬间使用 0600，
/// 避免先按默认权限创建、再 chmod 之间出现敏感内容可读的时间窗口。
pub(crate) fn private_create_new_options() -> fs::OpenOptions {
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    options
}

pub(crate) fn try_set_private_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(path)
            .map_err(|error| format!("读取路径权限失败 {}: {error}", path.display()))?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(if metadata.is_dir() { 0o700 } else { 0o600 });
        fs::set_permissions(path, permissions)
            .map_err(|error| format!("设置路径权限失败 {}: {error}", path.display()))?;
        Ok(())
    }

    #[cfg(windows)]
    {
        tighten_windows_private_file_acl(path)
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(windows)]
#[path = "windows_private_acl.rs"]
mod windows_private_acl;

#[cfg(windows)]
fn tighten_windows_private_file_acl(path: &Path) -> Result<(), String> {
    let _timing = crate::switch_timing::Phase::start("private_acl");
    windows_private_acl::tighten(path)
}

pub(crate) fn prepare_process_path() {
    let mut merged = preferred_executable_dirs();
    if let Some(current_path) = env::var_os("PATH") {
        for dir in env::split_paths(&current_path) {
            push_unique_dir(&mut merged, dir);
        }
    }

    if let Ok(path_env) = env::join_paths(merged) {
        env::set_var("PATH", path_env);
    }
}

pub(crate) fn find_command_path(command: &str) -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path_os) = env::var_os("PATH") {
        for dir in env::split_paths(&path_os) {
            push_command_candidates_from_dir(&mut candidates, &dir, command);
        }
    }

    for dir in preferred_executable_dirs() {
        push_command_candidates_from_dir(&mut candidates, &dir, command);
    }

    candidates.into_iter().find(|path| is_executable_file(path))
}

pub(crate) fn configure_background_command(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
}

pub(crate) fn new_background_command<S: AsRef<OsStr>>(program: S) -> Command {
    let mut command = Command::new(program);
    configure_background_command(&mut command);
    command
}

pub(crate) fn new_resolved_command(command: &str) -> Command {
    let program = find_command_path(command).unwrap_or_else(|| PathBuf::from(command));
    let mut command = new_background_command(&program);
    if let Some(parent) = program.parent().filter(|_| program.is_absolute()) {
        if let Some(path_env) = prepend_path_entry(parent) {
            command.env("PATH", path_env);
        }
    }
    command
}

pub(crate) fn prepend_path_entry(path: &Path) -> Option<OsString> {
    let mut paths = vec![path.to_path_buf()];
    if let Some(existing) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing));
    }
    env::join_paths(paths).ok()
}

pub(crate) fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn preferred_executable_dirs() -> Vec<PathBuf> {
    preferred_executable_dir_candidates()
        .into_iter()
        .filter(|dir| dir.is_dir())
        .collect()
}

fn preferred_executable_dir_candidates() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(target_os = "macos")]
    {
        for dir in [
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/opt/homebrew/sbin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/local/sbin"),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
            PathBuf::from("/usr/sbin"),
            PathBuf::from("/sbin"),
            PathBuf::from("/Library/Apple/usr/bin"),
        ] {
            push_unique_dir(&mut dirs, dir);
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for dir in [
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/local/sbin"),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/usr/sbin"),
            PathBuf::from("/bin"),
            PathBuf::from("/sbin"),
            PathBuf::from("/snap/bin"),
            PathBuf::from("/var/lib/flatpak/exports/bin"),
            PathBuf::from("/home/linuxbrew/.linuxbrew/bin"),
            PathBuf::from("/home/linuxbrew/.linuxbrew/sbin"),
            PathBuf::from("/nix/var/nix/profiles/default/bin"),
            PathBuf::from("/run/current-system/sw/bin"),
        ] {
            push_unique_candidate(&mut dirs, dir);
        }
    }

    if let Some(cargo_home) = env::var_os("CARGO_HOME").map(PathBuf::from) {
        push_unique_candidate(&mut dirs, cargo_home.join("bin"));
    }

    if let Some(homebrew_prefix) = env::var_os("HOMEBREW_PREFIX").map(PathBuf::from) {
        push_unique_candidate(&mut dirs, homebrew_prefix.join("bin"));
        push_unique_candidate(&mut dirs, homebrew_prefix.join("sbin"));
    }

    if let Some(pnpm_home) = env::var_os("PNPM_HOME").map(PathBuf::from) {
        push_unique_candidate(&mut dirs, pnpm_home);
    }

    if let Some(home) = dirs::home_dir() {
        for dir in [
            home.join(".cargo").join("bin"),
            home.join(".local").join("bin"),
            home.join("bin"),
            home.join(".asdf").join("shims"),
            home.join(".volta").join("bin"),
            home.join(".npm-global").join("bin"),
            home.join(".linuxbrew").join("bin"),
            home.join(".linuxbrew").join("sbin"),
            home.join(".nix-profile").join("bin"),
            home.join(".rye").join("shims"),
            home.join(".local").join("share").join("mise").join("shims"),
            home.join("Library").join("pnpm"),
            home.join("scoop").join("shims"),
            home.join("AppData")
                .join("Local")
                .join("Microsoft")
                .join("WinGet")
                .join("Links"),
        ] {
            push_unique_candidate(&mut dirs, dir);
        }

        #[cfg(target_os = "windows")]
        add_winget_package_executable_dirs(&mut dirs, &home);
    }

    dirs
}

#[cfg(target_os = "windows")]
fn add_winget_package_executable_dirs(dirs: &mut Vec<PathBuf>, home: &Path) {
    let packages_dir = home
        .join("AppData")
        .join("Local")
        .join("Microsoft")
        .join("WinGet")
        .join("Packages");
    let Ok(packages) = fs::read_dir(packages_dir) else {
        return;
    };

    for package in packages.flatten() {
        let package_path = package.path();
        let Some(package_name) = package_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !package_name.starts_with("zig.zig_") {
            continue;
        }
        push_unique_candidate(dirs, package_path.clone());
        if let Ok(children) = fs::read_dir(package_path) {
            for child in children.flatten() {
                push_unique_candidate(dirs, child.path());
            }
        }
    }
}

fn push_unique_dir(dirs: &mut Vec<PathBuf>, candidate: PathBuf) {
    if candidate.is_dir() && !dirs.iter().any(|existing| existing == &candidate) {
        dirs.push(candidate);
    }
}

fn push_unique_candidate(dirs: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !dirs.iter().any(|existing| existing == &candidate) {
        dirs.push(candidate);
    }
}

fn push_command_candidates_from_dir(candidates: &mut Vec<PathBuf>, dir: &Path, command: &str) {
    #[cfg(windows)]
    {
        for name in [
            format!("{command}.exe"),
            format!("{command}.cmd"),
            format!("{command}.bat"),
        ] {
            candidates.push(dir.join(name));
        }
    }

    #[cfg(not(windows))]
    {
        candidates.push(dir.join(command));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn unique_test_dir(name: &str) -> PathBuf {
        let unique = format!(
            "codex-tools-utils-test-{name}-{}-{}",
            std::process::id(),
            now_unix_seconds()
        );
        env::temp_dir().join(unique)
    }

    #[cfg(windows)]
    fn write_test_command(dir: &Path, command: &str) -> PathBuf {
        let path = dir.join(format!("{command}.cmd"));
        fs::write(&path, "@echo off\r\necho ok\r\n").expect("write test command");
        path
    }

    #[cfg(unix)]
    fn write_test_command(dir: &Path, command: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join(command);
        fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write test command");
        let mut permissions = fs::metadata(&path).expect("read metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("set execute bit");
        path
    }

    #[cfg(unix)]
    #[test]
    fn private_permissions_keep_files_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let sandbox = unique_test_dir("private-file");
        fs::create_dir_all(&sandbox).expect("create sandbox");
        let path = sandbox.join("secret.txt");
        fs::write(&path, "secret").expect("write secret");

        try_set_private_permissions(&path).expect("set private file permissions");

        let mode = fs::metadata(&path)
            .expect("read file metadata")
            .permissions()
            .mode()
            & 0o777;
        let _ = fs::remove_dir_all(&sandbox);
        assert_eq!(mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn private_permissions_keep_directories_searchable() {
        use std::os::unix::fs::PermissionsExt;

        let sandbox = unique_test_dir("private-dir");
        fs::create_dir_all(&sandbox).expect("create sandbox");

        try_set_private_permissions(&sandbox).expect("set private directory permissions");

        let mode = fs::metadata(&sandbox)
            .expect("read directory metadata")
            .permissions()
            .mode()
            & 0o777;
        fs::write(sandbox.join("probe.txt"), "ok").expect("directory should remain searchable");
        let _ = fs::remove_dir_all(&sandbox);
        assert_eq!(mode, 0o700);
    }

    #[test]
    fn find_command_path_uses_cargo_home_when_path_is_missing() {
        const PROBE_ENV: &str = "CODEX_TOOLS_TEST_CARGO_HOME_PROBE";
        const COMMAND: &str = "codex-tools-test-probe";
        if env::var_os(PROBE_ENV).is_some() {
            let bin_dir = PathBuf::from(env::var_os("CARGO_HOME").unwrap()).join("bin");
            let expected = bin_dir.join(if cfg!(windows) {
                "codex-tools-test-probe.cmd"
            } else {
                COMMAND
            });
            assert_eq!(find_command_path(COMMAND), Some(expected));
            return;
        }

        let sandbox = unique_test_dir("cargo-home");
        let cargo_home = sandbox.join("cargo-home");
        let bin_dir = cargo_home.join("bin");
        fs::create_dir_all(&bin_dir).expect("create cargo bin dir");
        write_test_command(&bin_dir, COMMAND);
        // Environment is process-wide: changing PATH in a parallel test can
        // break Windows process/ACL tests that launch powershell.exe.
        let output = std::process::Command::new(env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "utils::tests::find_command_path_uses_cargo_home_when_path_is_missing",
                "--nocapture",
            ])
            .env(PROBE_ENV, "1")
            .env("PATH", "")
            .env("CARGO_HOME", &cargo_home)
            .output()
            .expect("run isolated command discovery test");
        let _ = fs::remove_dir_all(&sandbox);
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn preferred_candidates_include_common_unix_system_dirs() {
        let dirs = preferred_executable_dir_candidates();
        assert!(dirs.contains(&PathBuf::from("/usr/local/bin")));
        assert!(dirs.contains(&PathBuf::from("/usr/bin")));
        assert!(dirs.contains(&PathBuf::from("/snap/bin")));
    }
}
