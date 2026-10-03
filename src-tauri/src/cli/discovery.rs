//! Locate configured and installed Codex command line executables.
use super::*;

pub(super) fn find_codex_cli_path() -> Option<PathBuf> {
    let mut candidates = codex_cli_candidates();
    append_nvm_codex_candidates(&mut candidates);
    append_macos_app_bundle_codex_candidates(&mut candidates);

    let mut seen = HashSet::new();
    for candidate in candidates {
        if !seen.insert(candidate.clone()) {
            continue;
        }
        if is_executable_file(&candidate) && is_codex_cli_file(&candidate) {
            return Some(candidate);
        }
    }

    None
}

pub(super) fn find_configured_codex_cli_path(configured_path: Option<&Path>) -> Option<PathBuf> {
    let configured_path = configured_path?;
    let mut candidates = Vec::new();
    append_configured_codex_candidates(&mut candidates, configured_path);

    let mut seen = HashSet::new();
    for candidate in candidates {
        if !seen.insert(candidate.clone()) {
            continue;
        }
        if is_executable_file(&candidate) && is_codex_cli_file(&candidate) {
            return Some(candidate);
        }
    }

    None
}

pub(super) fn find_configured_codex_app_path_from_path(
    configured_path: Option<&Path>,
) -> Option<PathBuf> {
    let configured_path = configured_path?;

    #[cfg(target_os = "macos")]
    {
        if is_macos_app_bundle(configured_path) {
            return Some(configured_path.to_path_buf());
        }
    }

    #[cfg(target_os = "windows")]
    {
        if is_windows_store_codex_path(configured_path) {
            return if has_windows_store_codex_app() {
                Some(configured_path.to_path_buf())
            } else {
                None
            };
        }

        if configured_path.is_file() && is_windows_codex_app_file(configured_path) {
            return Some(configured_path.to_path_buf());
        }

        if configured_path.is_dir() {
            let mut candidates = Vec::new();
            append_windows_codex_app_candidates_from_dir(&mut candidates, configured_path);
            append_windows_codex_app_candidates_from_dir(
                &mut candidates,
                &configured_path.join("current"),
            );
            append_windows_codex_app_candidates_from_dir(
                &mut candidates,
                &configured_path.join("app"),
            );
            append_windows_codex_app_candidates_from_dir(
                &mut candidates,
                &configured_path.join("Application"),
            );
            return first_executable_candidate(candidates);
        }
    }

    None
}

pub(super) fn codex_cli_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path_os) = env::var_os("PATH") {
        for dir in env::split_paths(&path_os) {
            push_codex_candidates_from_dir(&mut candidates, &dir);
        }
    }

    #[cfg(target_os = "macos")]
    {
        for dir in [
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
        ] {
            push_codex_candidates_from_dir(&mut candidates, &dir);
        }
    }

    if let Some(home) = dirs::home_dir() {
        for dir in [
            home.join(".local").join("bin"),
            home.join(".npm-global").join("bin"),
            home.join(".volta").join("bin"),
            home.join(".asdf").join("shims"),
            home.join(".pnpm"),
            home.join("Library").join("pnpm"),
            home.join("bin"),
            home.join("AppData")
                .join("Local")
                .join("Microsoft")
                .join("WindowsApps"),
            home.join("AppData")
                .join("Local")
                .join("Microsoft")
                .join("WinGet")
                .join("Links"),
        ] {
            push_codex_candidates_from_dir(&mut candidates, &dir);
        }
    }

    candidates
}
pub(super) fn append_nvm_codex_candidates(candidates: &mut Vec<PathBuf>) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let nvm_versions_dir = home.join(".nvm").join("versions").join("node");
    let Ok(entries) = fs::read_dir(&nvm_versions_dir) else {
        return;
    };

    let mut version_dirs = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    version_dirs.sort();
    version_dirs.reverse();

    for version_dir in version_dirs {
        push_codex_candidates_from_dir(candidates, &version_dir.join("bin"));
    }
}

pub(super) fn append_configured_codex_candidates(
    candidates: &mut Vec<PathBuf>,
    configured_path: &Path,
) {
    if configured_path.is_file() {
        if is_codex_cli_file(configured_path) {
            candidates.push(configured_path.to_path_buf());
        }
        return;
    }

    let mut search_dirs = vec![configured_path.to_path_buf()];

    if configured_path.is_dir() {
        search_dirs.push(configured_path.join("bin"));
        search_dirs.push(configured_path.join("resources"));
        search_dirs.push(configured_path.join("resources").join("bin"));
    }

    #[cfg(target_os = "macos")]
    if is_macos_app_bundle(configured_path) {
        candidates.extend(macos_codex_cli_candidates_for_bundle(configured_path));
    }

    for dir in search_dirs {
        push_codex_candidates_from_dir(candidates, &dir);
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn append_macos_app_bundle_codex_candidates(_candidates: &mut Vec<PathBuf>) {}

pub(super) fn push_codex_candidates_from_dir(candidates: &mut Vec<PathBuf>, dir: &Path) {
    #[cfg(windows)]
    let names = ["codex.exe", "codex.cmd", "codex.bat"];
    #[cfg(not(windows))]
    let names = ["codex"];

    for name in names {
        candidates.push(dir.join(name));
    }
}
