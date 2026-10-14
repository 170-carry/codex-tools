//! Local Git activity collection. No account tokens, shell commands, or working-tree writes.
use chrono::{DateTime, FixedOffset, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub kind: String,
    pub location: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub sources: Vec<Source>,
    pub aliases: BTreeMap<String, String>,
    pub excluded_authors: Vec<String>,
    pub rest_days: Vec<u8>,
    pub threshold: u64,
    pub sync_remote: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            sources: vec![],
            aliases: BTreeMap::new(),
            excluded_authors: vec![],
            rest_days: vec![0],
            threshold: 500,
            sync_remote: true,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.sources.len() > 200 {
            return Err("一次最多选择 200 个来源".into());
        }
        if self.rest_days.iter().any(|d| *d > 6) {
            return Err("休息日须为 0—6（周日—周六）".into());
        }
        for source in &self.sources {
            if source.location.trim().is_empty() {
                return Err("仓库路径不能为空".into());
            }
            match source.kind.as_str() {
                "local" => {}
                "remote" => validate_remote(source.location.trim())?,
                _ => return Err("不支持的仓库来源类型".into()),
            }
        }
        if self
            .aliases
            .iter()
            .any(|(k, v)| k.trim().is_empty() || v.trim().is_empty())
        {
            return Err("账号合并的邮箱与显示名都不能为空".into());
        }
        Ok(())
    }
}

pub fn validate_remote(value: &str) -> Result<(), String> {
    if value.starts_with('-') || value.chars().any(char::is_whitespace) {
        return Err("Git 地址不能包含空白或以 - 开头".into());
    }
    if let Ok(url) = reqwest::Url::parse(value) {
        if ["https", "http", "ssh", "git", "file"].contains(&url.scheme()) {
            if url.password().is_some()
                || (["https", "http"].contains(&url.scheme()) && !url.username().is_empty())
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err("请勿把密码或令牌放入地址；使用本机 Git 凭据管理器或 SSH 密钥".into());
            }
            if url.path().is_empty() || url.path() == "/" {
                return Err("请输入具体仓库的 Git 克隆地址".into());
            }
            return Ok(());
        }
    }
    // SCP-style SSH remotes, e.g. git@example.com:team/repository.git.
    if let Some((host, path)) = value.split_once(':') {
        if let Some((user, domain)) = host.split_once('@') {
            if !user.is_empty()
                && !domain.is_empty()
                && !path.is_empty()
                && host
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "@._-".contains(c))
            {
                return Ok(());
            }
        }
    }
    Err("请输入 HTTPS、SSH 或 file:// Git 克隆地址，不支持外部命令协议".into())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub repo_key: String,
    pub person: String,
    pub author: String,
    pub email: String,
    pub subject: String,
    pub commit_day: String,
    pub source_add: u64,
    pub source_del: u64,
    pub source_files: usize,
    pub bulk: bool,
    pub keep: bool,
    pub company_unique: bool,
    pub all_unique: bool,
    #[serde(skip)]
    timestamp: i64,
    #[serde(skip)]
    signature: String,
    #[serde(skip)]
    path: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Person {
    pub identity: String,
    pub person: String,
    pub names: Vec<String>,
    pub emails: Vec<String>,
    pub company_unique: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Repository {
    pub key: String,
    pub repo: String,
    pub category: String,
    pub synced: bool,
    pub shallow: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorRecord {
    pub name: String,
    pub email: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub start: String,
    pub end: String,
    pub snapshot: String,
    pub people: Vec<Person>,
    #[serde(default)]
    pub authors: Vec<AuthorRecord>,
    pub repos: Vec<Repository>,
    pub rows: Vec<Commit>,
    pub warnings: Vec<String>,
    pub config: Config,
}

fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

fn git(
    path: &Path,
    args: &[&str],
    input: Option<Vec<u8>>,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("已取消统计，原报告保留".into());
    }
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "credential.interactive=false",
            "-c",
            "core.longpaths=true",
            "-c",
            "maintenance.auto=false",
            "-c",
            "gc.auto=0",
            "-c",
            "core.quotePath=false",
            "-c",
            "i18n.logOutputEncoding=utf-8",
            "-c",
            "protocol.ext.allow=never",
        ])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .env(
            "GIT_SSH_COMMAND",
            "ssh -o BatchMode=yes -o ConnectTimeout=15",
        )
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hidden(&mut command);
    let mut child = command
        .spawn()
        .map_err(|_| "无法启动 Git，请先安装 Git 并加入 PATH".to_string())?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || {
        let mut data = Vec::new();
        stdout.read_to_end(&mut data).map(|_| data)
    });
    let err = thread::spawn(move || {
        let mut data = Vec::new();
        stderr.read_to_end(&mut data).map(|_| data)
    });
    let writer = input.map(|data| {
        let mut stdin = child.stdin.take().unwrap();
        thread::spawn(move || stdin.write_all(&data))
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None)
                if !cancel.load(Ordering::Relaxed)
                    && start.elapsed() < Duration::from_secs(600) =>
            {
                thread::sleep(Duration::from_millis(20))
            }
            _ => {
                #[cfg(windows)]
                {
                    let mut kill = Command::new("taskkill");
                    kill.args(["/PID", &child.id().to_string(), "/T", "/F"])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null());
                    hidden(&mut kill);
                    let _ = kill.status();
                }
                let _ = child.kill();
                let _ = child.wait();
                break Err(if cancel.load(Ordering::Relaxed) {
                    "已取消统计，原报告保留"
                } else {
                    "Git 操作超时，请检查网络与仓库规模"
                }
                .to_string());
            }
        }
    };
    let output = out
        .join()
        .map_err(|_| "读取 Git 输出失败")?
        .map_err(|_| "读取 Git 输出失败")?;
    let error = err
        .join()
        .map_err(|_| "读取 Git 错误失败")?
        .map_err(|_| "读取 Git 错误失败")?;
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    if !status?.success() {
        let detail = String::from_utf8_lossy(&error).to_lowercase();
        // Never return raw stderr: Git may include credential-bearing remote URLs.
        let reason = if detail.contains("authentication")
            || detail.contains("permission denied")
            || detail.contains("username")
        {
            "认证失败，请先在本机 Git 中配置该仓库的凭据或 SSH 密钥"
        } else if detail.contains("not found") || detail.contains("not a git repository") {
            "仓库不存在、无权限或不是有效 Git 仓库"
        } else if detail.contains("resolve host")
            || detail.contains("connect")
            || detail.contains("timed out")
        {
            "网络连接失败，请检查远程地址与网络"
        } else {
            "Git 操作失败，请检查仓库访问权限、分支与标签状态"
        };
        return Err(reason.into());
    }
    Ok(output)
}
fn text(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).trim().to_string()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn repository_identity(remote: &str) -> String {
    if let Ok(url) = reqwest::Url::parse(remote) {
        if let Some(host) = url.host_str() {
            return format!(
                "{}{}{}",
                host.to_lowercase(),
                url.port().map(|p| format!(":{p}")).unwrap_or_default(),
                url.path().trim_end_matches('/').trim_end_matches(".git")
            );
        }
    }
    if let Some((host, path)) = remote.split_once(':') {
        if host.contains('@') && !path.is_empty() {
            return format!(
                "{}/{}",
                host.rsplit('@').next().unwrap().to_lowercase(),
                path.trim_start_matches('/')
                    .trim_end_matches('/')
                    .trim_end_matches(".git")
            );
        }
    }
    remote.trim_end_matches('/').trim_end_matches(".git").into()
}

fn discover(
    root: &Path,
    found: &mut BTreeSet<PathBuf>,
    visited: &mut HashSet<PathBuf>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("已取消统计".into());
    }
    let root = fs::canonicalize(root).map_err(|_| format!("无法读取目录：{}", root.display()))?;
    if !visited.insert(root.clone()) {
        return Ok(());
    }
    if root.join(".git").exists() || (root.join("HEAD").is_file() && root.join("objects").is_dir())
    {
        found.insert(root);
        return Ok(());
    }
    for entry in fs::read_dir(&root).map_err(|_| format!("无法扫描目录：{}", root.display()))?
    {
        let entry = entry.map_err(|_| "读取目录条目失败")?;
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if [
            ".git",
            "node_modules",
            "vendor",
            "dist",
            "build",
            "target",
            ".venv",
            "venv",
            ".next",
            ".idea",
            "__pycache__",
            "site-packages",
        ]
        .contains(&name.as_str())
        {
            continue;
        }
        if entry.path().is_dir() {
            discover(&entry.path(), found, visited, cancel)?;
        }
    }
    Ok(())
}

fn source_file(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_lowercase();
    if lower.split('/').any(|p| {
        [
            "node_modules",
            "vendor",
            "third_party",
            "third-party",
            "dist",
            "build",
            "target",
            ".venv",
            "venv",
            "__pycache__",
            ".next",
            "coverage",
            "site-packages",
            "mediaserver",
        ]
        .contains(&p)
    }) || lower.ends_with(".min.js")
        || lower.ends_with(".min.css")
    {
        return false;
    }
    let extension = lower.rsplit('.').next().unwrap_or("");
    [
        "py", "java", "ts", "tsx", "js", "jsx", "vue", "c", "cpp", "h", "hpp", "cs", "go", "rs",
        "kt", "sql", "html", "css", "scss", "less", "sh", "ps1", "bat", "wxml", "wxss", "xml",
    ]
    .contains(&extension)
}

fn parse_log(
    raw: &[u8],
    repo_key: &str,
    path: &Path,
    config: &Config,
    authors: &mut BTreeSet<(String, String)>,
) -> Result<Vec<Commit>, String> {
    let mut result = Vec::new();
    let aliases: HashMap<_, _> = config
        .aliases
        .iter()
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim()))
        .collect();
    let excluded: HashSet<_> = config
        .excluded_authors
        .iter()
        .map(|s| s.trim().to_lowercase())
        .collect();
    // -z numstat records preserve tabs/newlines in filenames. Headers have an explicit NUL marker.
    let tokens: Vec<_> = raw.split(|b| *b == 0).collect();
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index] != b"CODEX_ACTIVITY_COMMIT" {
            index += 1;
            continue;
        }
        if index + 7 >= tokens.len() {
            return Err("Git 历史格式不完整".into());
        }
        let fields: Vec<String> = tokens[index + 1..index + 8]
            .iter()
            .map(|v| String::from_utf8_lossy(v).into_owned())
            .collect();
        index += 8;
        let [sha, parents, author, email, date, subject, _end]: [String; 7] =
            fields.try_into().unwrap();
        let time = DateTime::parse_from_rfc3339(&date).map_err(|_| "Git 日期格式异常")?;
        let email_key = email.trim().to_lowercase();
        let merged = aliases.get(&email_key).copied();
        let person = merged
            .map(|name| format!("alias:{name}"))
            .unwrap_or_else(|| {
                if email_key.is_empty() {
                    format!("name:{author}")
                } else {
                    email_key.clone()
                }
            });
        let mut row = Commit {
            sha,
            repo_key: repo_key.into(),
            person,
            author,
            email,
            subject,
            commit_day: time
                .with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
                .format("%Y-%m-%d")
                .to_string(),
            source_add: 0,
            source_del: 0,
            source_files: 0,
            bulk: false,
            keep: false,
            company_unique: false,
            all_unique: false,
            timestamp: time.timestamp(),
            signature: String::new(),
            path: path.into(),
        };
        let mut stats = Vec::new();
        while index < tokens.len() && tokens[index] != b"CODEX_ACTIVITY_COMMIT" {
            let line = String::from_utf8_lossy(tokens[index]);
            index += 1;
            let mut parts = line.trim_start_matches('\n').splitn(3, '\t');
            if let (Some(add), Some(del), Some(file)) = (parts.next(), parts.next(), parts.next()) {
                stats.push((file.to_string(), add.to_string(), del.to_string()));
                if source_file(file) {
                    row.source_files += 1;
                    row.source_add += add.parse::<u64>().unwrap_or(0);
                    row.source_del += del.parse::<u64>().unwrap_or(0);
                }
            }
        }
        stats.sort();
        row.signature = hash(&serde_json::to_vec(&stats).map_err(|e| e.to_string())?);
        row.bulk = row.source_add + row.source_del > 10000 || row.source_files > 100;
        if parents.split_whitespace().count() <= 1 {
            authors.insert((row.author.clone(), row.email.clone()));
        }
        if parents.split_whitespace().count() > 1
            || excluded.contains(&email_key)
            || excluded.contains(&row.author.trim().to_lowercase())
        {
            continue;
        }
        result.push(row);
    }
    Ok(result)
}

pub fn collect(
    config: Config,
    cache: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(String),
) -> Result<Report, String> {
    config.validate()?;
    if config.sources.is_empty() {
        return Err("请先添加一个本地目录或远程仓库".into());
    }
    fs::create_dir_all(cache).map_err(|e| e.to_string())?;
    let mut found = BTreeSet::new();
    let mut visited = HashSet::new();
    for (index, source) in config.sources.iter().enumerate() {
        progress(format!("发现仓库 {}/{}", index + 1, config.sources.len()));
        if source.kind == "local" {
            discover(
                Path::new(source.location.trim()),
                &mut found,
                &mut visited,
                cancel,
            )?;
        } else {
            let remote = source.location.trim();
            let key = hash(remote.as_bytes())[..32].to_string();
            let index_file = cache.join(format!("{key}.repo"));
            let saved = fs::read_to_string(&index_file).ok();
            let existing = saved
                .as_deref()
                .filter(|name| {
                    name.ends_with(".git") && !name.contains(['/', '\\']) && !name.starts_with('.')
                })
                .map(|name| cache.join(name))
                .filter(|p| p.is_dir());
            let destination = if let Some(path) = existing {
                path
            } else {
                progress(format!(
                    "下载远程仓库 {}/{}（首次下载可能较慢）",
                    index + 1,
                    config.sources.len()
                ));
                // Publish a small completion index instead of renaming a Git directory.
                // Windows indexers can hold directory handles after Git exits.
                let name = format!("{}.git", uuid::Uuid::new_v4());
                let path = cache.join(&name);
                git(
                    cache,
                    &["clone", "--bare", "--", remote, &path.to_string_lossy()],
                    None,
                    cancel,
                )?;
                if cancel.load(Ordering::Relaxed) {
                    return Err("已取消统计".into());
                }
                fs::write(&index_file, name).map_err(|e| format!("保存仓库缓存索引失败：{e}"))?;
                path
            };
            found.insert(fs::canonicalize(destination).map_err(|e| e.to_string())?);
        }
    }
    if found.is_empty() {
        return Err("所选目录中没有找到 Git 仓库，请选择含 .git 的项目目录或其上级目录".into());
    }
    let mut repos: Vec<Repository> = Vec::new();
    let mut rows = Vec::new();
    let mut authors = BTreeSet::new();
    let mut warnings = Vec::new();
    let mut seen_common = HashSet::new();
    let snapshot = Utc::now().with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap());
    for (index, path) in found.iter().enumerate() {
        progress(format!("同步并读取仓库 {}/{}", index + 1, found.len()));
        let common = text(git(
            path,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            None,
            cancel,
        )?);
        // Worktrees share branches but may have separate detached HEADs; include each selected worktree.
        let bare = text(git(
            path,
            &["rev-parse", "--is-bare-repository"],
            None,
            cancel,
        )?) == "true";
        let mut shallow = text(git(
            path,
            &["rev-parse", "--is-shallow-repository"],
            None,
            cancel,
        )?) == "true";
        let remotes = text(git(path, &["remote"], None, cancel)?);
        let origin = if remotes.lines().any(|r| r == "origin") {
            Some(text(git(
                path,
                &["remote", "get-url", "origin"],
                None,
                cancel,
            )?))
        } else {
            None
        };
        let repo_key = hash(
            origin
                .as_deref()
                .map(repository_identity)
                .unwrap_or_else(|| common.clone())
                .as_bytes(),
        );
        let mut synced = false;
        if config.sync_remote && !remotes.is_empty() && seen_common.insert(common) {
            for remote in remotes.lines() {
                let mut args = vec!["fetch", "--tags"];
                if shallow {
                    args.push("--unshallow");
                }
                args.push(remote);
                let refspec = if bare {
                    "+refs/heads/*:refs/heads/*".to_string()
                } else {
                    format!("+refs/heads/*:refs/remotes/{remote}/*")
                };
                args.push(&refspec);
                git(path, &args, None, cancel).map_err(|e| {
                    format!(
                        "仓库 {}：{e}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    )
                })?;
                shallow = false;
            }
            synced = true;
        } else if config.sync_remote && !remotes.is_empty() {
            synced = true;
        }
        if shallow {
            warnings.push("存在浅克隆，当前只统计本地已有历史；开启同步以补全".into());
        }
        let name = if bare && !remotes.is_empty() {
            // Display a remote basename without exposing credentials or cache hashes.
            let remote_name = if remotes.lines().any(|r| r == "origin") {
                "origin"
            } else {
                remotes.lines().next().unwrap()
            };
            let remote = text(git(
                path,
                &["remote", "get-url", remote_name],
                None,
                cancel,
            )?);
            remote
                .trim_end_matches('/')
                .rsplit(['/', ':'])
                .next()
                .unwrap_or("remote")
                .trim_end_matches(".git")
                .to_string()
        } else {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        let until = format!("--until={}", snapshot.to_rfc3339());
        let log = git(path, &["log", "--all", &until, "--format=%x00CODEX_ACTIVITY_COMMIT%x00%H%x00%P%x00%an%x00%ae%x00%cI%x00%s%x00END%x00", "--numstat", "-z", "--no-renames", "--no-ext-diff", "--no-textconv"], None, cancel)?;
        rows.extend(parse_log(&log, &repo_key, path, &config, &mut authors)?);
        if let Some(existing) = repos.iter_mut().find(|r| r.key == repo_key) {
            existing.synced &= synced;
            existing.shallow |= shallow;
        } else {
            repos.push(Repository {
                key: repo_key,
                repo: name,
                category: "selected".into(),
                synced,
                shallow,
            });
        }
    }
    progress("正在按账号、提交及相同补丁去重".into());
    let mut candidates: HashMap<(String, String), BTreeSet<String>> = HashMap::new();
    for c in &rows {
        candidates
            .entry((c.person.clone(), c.signature.clone()))
            .or_default()
            .insert(c.sha.clone());
    }
    let mut compared = HashSet::new();
    let patch_candidates: Vec<_> = rows
        .iter()
        .filter(|c| {
            candidates[&(c.person.clone(), c.signature.clone())].len() > 1
                && compared.insert(c.sha.clone())
        })
        .collect();
    let mut patches = HashMap::new();
    // Limit concurrent Git processes and memory while comparing independent patches.
    for (index, batch) in patch_candidates.chunks(4).enumerate() {
        let results = thread::scope(|scope| {
            let handles: Vec<_> = batch
                .iter()
                .map(|c| {
                    scope.spawn(move || -> Result<(String, String), String> {
                        let patch = git(
                            &c.path,
                            &[
                                "show",
                                "--format=",
                                "--no-ext-diff",
                                "--no-textconv",
                                "--no-renames",
                                &c.sha,
                            ],
                            None,
                            cancel,
                        )?;
                        let output = text(git(
                            &c.path,
                            &["patch-id", "--stable"],
                            Some(patch),
                            cancel,
                        )?);
                        Ok((
                            c.sha.clone(),
                            output
                                .split_whitespace()
                                .next()
                                .unwrap_or(&c.sha)
                                .to_string(),
                        ))
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().map_err(|_| "补丁比较线程失败".to_string())?)
                .collect::<Result<Vec<_>, String>>()
        })?;
        patches.extend(results);
        if index % 10 == 0 || (index + 1) * 4 >= patch_candidates.len() {
            progress(format!(
                "比较重复补丁 {}/{}",
                ((index + 1) * 4).min(patch_candidates.len()),
                patch_candidates.len()
            ));
        }
    }
    rows.sort_by(|a, b| {
        (&a.timestamp, &a.sha, &a.repo_key).cmp(&(&b.timestamp, &b.sha, &b.repo_key))
    });
    let mut local = HashSet::new();
    let mut global = HashSet::new();
    for c in &mut rows {
        let patch = patches.get(&c.sha).unwrap_or(&c.sha);
        c.keep = local.insert((c.repo_key.clone(), c.person.clone(), patch.clone()));
        c.company_unique = global.insert((c.person.clone(), patch.clone()));
        c.all_unique = c.company_unique;
    }
    let mut people: BTreeMap<String, Person> = BTreeMap::new();
    for c in &rows {
        let p = people.entry(c.person.clone()).or_insert_with(|| Person {
            identity: c.person.clone(),
            person: c
                .person
                .strip_prefix("alias:")
                .unwrap_or(&c.author)
                .to_string(),
            names: vec![],
            emails: vec![],
            company_unique: 0,
        });
        if !p.names.contains(&c.author) {
            p.names.push(c.author.clone());
        }
        if !p.emails.contains(&c.email) {
            p.emails.push(c.email.clone());
        }
        p.company_unique += usize::from(c.company_unique);
    }
    let mut people: Vec<_> = people.into_values().collect();
    let mut names = HashMap::new();
    for p in &people {
        *names.entry(p.person.clone()).or_insert(0) += 1;
    }
    for p in &mut people {
        if names[&p.person] > 1 {
            p.person = format!("{} [{}]", p.person, p.identity);
        }
    }
    people.sort_by(|a, b| {
        b.company_unique
            .cmp(&a.company_unique)
            .then(a.person.cmp(&b.person))
    });
    rows.retain(|c| c.keep);
    warnings.sort();
    warnings.dedup();
    if !config.sync_remote {
        warnings.push("仅本地历史：本次未联网同步，远程新提交可能未包含".into());
    }
    let end = snapshot.format("%Y-%m-%d").to_string();
    let start = rows
        .iter()
        .map(|c| c.commit_day.clone())
        .min()
        .unwrap_or_else(|| end.clone());
    progress(format!(
        "统计完成：{} 个仓库，{} 人，{} 次去重提交",
        repos.len(),
        people.len(),
        global.len()
    ));
    Ok(Report {
        start,
        end,
        snapshot: snapshot.to_rfc3339(),
        people,
        authors: authors.into_iter().map(|(name, email)| AuthorRecord { name, email }).collect(),
        repos,
        rows,
        warnings,
        config,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_validation() {
        for url in [
            "https://example.com/a.git",
            "ssh://git@example.com/a.git",
            "git@example.com:team/a.git",
            "file:///tmp/repo.git",
        ] {
            assert!(validate_remote(url).is_ok(), "{url}");
        }
        for url in [
            "--upload-pack=evil",
            "ext::sh evil",
            "https://user:secret@example.com/a.git",
            "https://example.com/a?token=secret",
            "https://example.com/",
            "git@example.com:a\nb",
        ] {
            assert!(validate_remote(url).is_err(), "{url}");
        }
    }
    #[test]
    fn source_exclusions() {
        assert!(source_file("src/中文\t文件.ts"));
        assert!(!source_file("node_modules/a.ts"));
        assert!(!source_file("dist/index.js"));
        assert!(!source_file("vendor/a.py"));
        assert!(!source_file("x.min.js"));
        assert!(!source_file("asset.png"));
    }
    #[test]
    fn parses_dates_aliases_and_binary() {
        let input = b"\0CODEX_ACTIVITY_COMMIT\0abc\0parent\0User\0USER@EXAMPLE.COM\02026-06-01T18:30:00Z\0fix </script>\0END\0\n12\t3\tsrc/a.ts\0-\t-\tsrc/image.png\0";
        let mut config = Config::default();
        config
            .aliases
            .insert("user@example.com".into(), "Team member".into());
        let rows = parse_log(input, "repo", Path::new("."), &config, &mut BTreeSet::new()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].commit_day, "2026-06-02");
        assert_eq!(rows[0].person, "alias:Team member");
        assert_eq!(rows[0].source_add, 12);
        config.excluded_authors.push("user@example.com".into());
        assert!(parse_log(input, "repo", Path::new("."), &config, &mut BTreeSet::new())
            .unwrap()
            .is_empty());
    }
    #[test]
    fn real_local_remote_and_patch_deduplication() {
        let root =
            std::env::temp_dir().join(format!("codex-git-activity-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let repo = root.join("repository");
        fs::create_dir(&repo).unwrap();
        let command = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .env("GIT_AUTHOR_NAME", "Alice")
                .env("GIT_AUTHOR_EMAIL", "alice@example.com")
                .env("GIT_COMMITTER_NAME", "Alice")
                .env("GIT_COMMITTER_EMAIL", "alice@example.com")
                .env("GIT_AUTHOR_DATE", "2026-06-01T12:00:00+08:00")
                .env("GIT_COMMITTER_DATE", "2026-06-01T12:00:00+08:00")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            text(output.stdout)
        };
        command(&["init", "-b", "main"]);
        fs::write(repo.join("a.ts"), "const a = 1;\n").unwrap();
        command(&["add", "."]);
        command(&["commit", "-m", "initial"]);
        let parent = command(&["rev-parse", "HEAD"]);
        fs::write(repo.join("a.ts"), "const a = 2;\n").unwrap();
        command(&["add", "."]);
        command(&["commit", "-m", "feature"]);
        let tree = command(&["rev-parse", "HEAD^{tree}"]);
        let duplicate = command(&[
            "commit-tree",
            &tree,
            "-p",
            &parent,
            "-m",
            "backport feature",
        ]);
        command(&["update-ref", "refs/heads/backport", &duplicate]);
        let config = Config {
            sources: vec![Source {
                kind: "local".into(),
                location: repo.to_string_lossy().into(),
            }],
            sync_remote: false,
            ..Config::default()
        };
        let cancel = AtomicBool::new(false);
        let report = collect(config.clone(), &root.join("cache"), &cancel, |_| {}).unwrap();
        assert_eq!(report.rows.len(), 2, "equivalent backport must deduplicate");
        assert_eq!(report.people.len(), 1);
        assert_eq!(report.people[0].company_unique, 2);
        assert_eq!(report.rows.iter().map(|c| c.source_add).sum::<u64>(), 2);
        assert_eq!(report.rows.iter().map(|c| c.source_del).sum::<u64>(), 1);
        let mut both = config.clone();
        both.sources.push(Source {
            kind: "remote".into(),
            location: reqwest::Url::from_file_path(&repo).unwrap().to_string(),
        });
        both.sync_remote = true;
        let remote = collect(both.clone(), &root.join("cache"), &cancel, |_| {}).unwrap();
        assert_eq!(remote.repos.len(), 2);
        assert_eq!(remote.rows.len(), 4);
        assert_eq!(remote.rows.iter().filter(|c| c.company_unique).count(), 2);
        fs::write(repo.join("new.py"), "print('new')\n").unwrap();
        command(&["add", "."]);
        command(&["commit", "-m", "new commit"]);
        let refreshed = collect(both, &root.join("cache"), &cancel, |_| {}).unwrap();
        assert_eq!(
            refreshed.rows.iter().filter(|c| c.company_unique).count(),
            3
        );
        // The same patch committed under two emails is distinct until the user merges them.
        fs::write(repo.join("alias.ts"), "const alias = true;\n").unwrap();
        command(&["add", "."]);
        command(&["commit", "--author", "Other Alice <alice.other@example.com>", "-m", "alias feature"]);
        command(&["branch", "other-email"]);
        command(&["commit", "--amend", "--author", "Alice <alice@example.com>", "--no-edit"]);
        let unmerged = collect(config.clone(), &root.join("cache"), &cancel, |_| {}).unwrap();
        assert_eq!(unmerged.people.len(), 2);
        assert_eq!(unmerged.rows.len(), 5);
        let mut merged_config = config;
        merged_config.aliases.insert("alice@example.com".into(), "Alice unified".into());
        merged_config.aliases.insert("alice.other@example.com".into(), "Alice unified".into());
        let merged = collect(merged_config.clone(), &root.join("cache"), &cancel, |_| {}).unwrap();
        assert_eq!(merged.people.len(), 1);
        assert_eq!(merged.people[0].person, "Alice unified");
        assert_eq!(merged.people[0].emails.len(), 2);
        assert_eq!(merged.people[0].company_unique, 4);
        assert_eq!(merged.rows.len(), 4, "merging must rerun patch deduplication");
        assert_eq!(merged.rows.iter().map(|row| row.source_add).sum::<u64>(), 4);
        assert_eq!(merged.rows.iter().map(|row| row.source_del).sum::<u64>(), 1);
        let mut excluded_config = merged_config.clone();
        excluded_config.excluded_authors.push("ALICE.OTHER@EXAMPLE.COM".into());
        let excluded = collect(excluded_config.clone(), &root.join("cache"), &cancel, |_| {}).unwrap();
        assert!(excluded.rows.iter().all(|row| row.email != "alice.other@example.com"));
        assert_eq!(excluded.authors.len(), 2, "excluded authors remain selectable in settings");
        let reloaded: Report = serde_json::from_slice(&serde_json::to_vec(&excluded).unwrap()).unwrap();
        assert_eq!(reloaded.authors.len(), 2);
        excluded_config.excluded_authors.push("Alice".into());
        let all_excluded = collect(excluded_config, &root.join("cache"), &cancel, |_| {}).unwrap();
        assert!(all_excluded.rows.is_empty());
        assert!(all_excluded.people.is_empty());
        assert_eq!(all_excluded.authors.len(), 2);
        merged_config.aliases.clear();
        let split = collect(merged_config, &root.join("cache"), &cancel, |_| {}).unwrap();
        assert_eq!(split.people.len(), 2);
        assert_eq!(split.rows.len(), 5, "original authors remain available for splitting");
        cancel.store(true, Ordering::Relaxed);
        assert!(collect(
            Config {
                sources: vec![Source {
                    kind: "local".into(),
                    location: repo.to_string_lossy().into()
                }],
                ..Config::default()
            },
            &root.join("cache"),
            &cancel,
            |_| {}
        )
        .is_err());
        // Only this test's UUID directory is eligible for cleanup.
        assert!(root.starts_with(std::env::temp_dir()));
        fs::remove_dir_all(root).unwrap();
    }
}
