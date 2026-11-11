//! Cross-process persistence primitives shared with the headless proxy.
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    sync::Mutex,
};

#[derive(Debug, Default)]
pub(crate) struct StoreRevision(Mutex<Option<String>>);
impl Clone for StoreRevision {
    fn clone(&self) -> Self {
        Self(Mutex::new(self.get()))
    }
}
impl StoreRevision {
    pub(crate) fn get(&self) -> Option<String> {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub(crate) fn set(&self, value: Option<String>) {
        *self.0.lock().unwrap_or_else(|p| p.into_inner()) = value;
    }
}
pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if !meta.file_type().is_file() => {
            log::warn!("PROJECT_SHARING_INVALID_STATE stage=read-file path={} reason=not-a-regular-file type={:?}", path.display(), meta.file_type());
            return Err("PROJECT_SHARING_INVALID_STATE".into());
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error("file-metadata", path, error)),
        _ => {}
    }
    fs::read(path)
        .map(Some)
        .map_err(|error| io_error("read-file", path, error))
}

fn io_error(stage: &str, path: &Path, error: impl std::fmt::Display) -> String {
    log::warn!(
        "PROJECT_SHARING_IO stage={stage} path={} reason={error}",
        path.display()
    );
    "PROJECT_SHARING_IO".into()
}
pub(crate) fn fingerprint(path: &Path) -> Result<Option<String>, String> {
    Ok(read_optional(path)?.map(|b| digest(&b)))
}
/// A per-store marker covers explicit/custom Codex homes without consulting
/// the process default home. Corrupt or non-file markers also fail closed.
pub(crate) fn pending_marker_path(store: &Path) -> Result<std::path::PathBuf, String> {
    let mut name = store
        .file_name()
        .ok_or("PROJECT_SHARING_INVALID_STATE")?
        .to_os_string();
    name.push(".project-switch-pending.json");
    Ok(store.with_file_name(name))
}
pub(crate) fn ensure_store_available(store: &Path) -> Result<(), String> {
    match fs::symlink_metadata(pending_marker_path(store)?) {
        Ok(_) => Err("PROJECT_SHARING_RECOVERY_REQUIRED".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("PROJECT_SHARING_IO".into()),
    }
}
pub(crate) struct FileLock {
    _file: File,
}
impl FileLock {
    pub(crate) fn acquire(path: &Path) -> Result<Self, String> {
        if fs::symlink_metadata(path).is_ok_and(|m| !m.file_type().is_file()) {
            log::warn!(
                "PROJECT_SHARING_INVALID_STATE stage=lock-file path={} reason=not-a-regular-file",
                path.display()
            );
            return Err("PROJECT_SHARING_INVALID_STATE".into());
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let file = options.open(path).map_err(|error| {
            log::warn!(
                "PROJECT_SHARING_BUSY stage=lock-file path={} reason={error}",
                path.display()
            );
            "PROJECT_SHARING_BUSY".to_string()
        })?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            extern "C" {
                fn flock(
                    fd: std::os::raw::c_int,
                    operation: std::os::raw::c_int,
                ) -> std::os::raw::c_int;
            }
            // LOCK_EX | LOCK_NB on macOS/Linux/BSD; closing the handle releases it.
            if unsafe { flock(file.as_raw_fd(), 2 | 4) } != 0 {
                return Err("PROJECT_SHARING_BUSY".into());
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            return Err("PROJECT_SHARING_UNSUPPORTED_PLATFORM".into());
        }
        Ok(Self { _file: file })
    }
}
pub(crate) fn private_dir(path: &Path) -> Result<(), String> {
    private_dir_with_permissions(path, crate::utils::try_set_private_permissions)
}
fn private_dir_with_permissions(
    path: &Path,
    protect: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| io_error("create-directory", path, error))?;
    protect(path).map_err(|error| io_error("protect-directory", path, error))
}
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    atomic_write_with_permissions(path, bytes, crate::utils::try_set_private_permissions)
}
fn atomic_write_with_permissions(
    path: &Path,
    bytes: &[u8],
    protect: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let parent = path.parent().ok_or("PROJECT_SHARING_INVALID_STATE")?;
    if !parent.is_dir() {
        private_dir(parent)?;
    }
    let temp = parent.join(format!(".project-switch-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut f = crate::utils::private_create_new_options()
            .open(&temp)
            .map_err(|error| io_error("create-temp-file", path, error))?;
        // On Windows creation mode alone does not restrict inherited ACLs.
        // Establish private permissions before the first sensitive byte.
        protect(&temp).map_err(|error| io_error("protect-temp-file", path, error))?;
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|error| io_error("write-temp-file", path, error))?;
        drop(f);
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            #[link(name = "kernel32")]
            extern "system" {
                fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
            }
            let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 1 | 8) } == 0 {
                return Err(io_error(
                    "replace-file",
                    path,
                    std::io::Error::last_os_error(),
                ));
            }
        }
        #[cfg(not(windows))]
        fs::rename(&temp, path).map_err(|error| io_error("replace-file", path, error))?;
        sync_dir(parent)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
pub(crate) fn sync_dir(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|error| io_error("sync-directory", path, error))?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permission_failure_precedes_sensitive_bytes_and_removes_empty_temp() {
        let dir =
            std::env::temp_dir().join(format!("synthetic-permission-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("auth.json");
        fs::write(&path, b"synthetic original").unwrap();
        let called = std::cell::Cell::new(false);
        let observed = std::cell::RefCell::new(None);
        let result = atomic_write_with_permissions(&path, b"synthetic sensitive bytes", |temp| {
            called.set(true);
            assert_eq!(fs::read(temp).unwrap(), b"");
            *observed.borrow_mut() = Some(temp.to_path_buf());
            Err("synthetic ACL failure".into())
        });
        assert!(result.is_err());
        assert!(called.get());
        assert!(!observed.borrow().as_ref().unwrap().exists());
        assert_eq!(fs::read(&path).unwrap(), b"synthetic original");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn directory_permission_failure_is_propagated() {
        let dir = std::env::temp_dir().join(format!(
            "synthetic-directory-permission-{}",
            uuid::Uuid::new_v4()
        ));
        assert!(
            private_dir_with_permissions(&dir, |_| Err("synthetic ACL failure".into())).is_err()
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
