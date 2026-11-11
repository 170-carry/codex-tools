use crate::{models::AccountsStore, store_lock};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(super) struct PlanInputs {
    files: Vec<(PathBuf, Option<Vec<u8>>)>,
}

impl PlanInputs {
    pub(super) fn capture(
        paths: impl IntoIterator<Item = PathBuf>,
        read: &mut impl FnMut(&Path) -> Result<Option<Vec<u8>>, String>,
    ) -> Result<Self, String> {
        let mut inputs = Self::default();
        for path in paths {
            inputs.track(path, read)?;
        }
        Ok(inputs)
    }

    pub(super) fn track(
        &mut self,
        path: PathBuf,
        read: &mut impl FnMut(&Path) -> Result<Option<Vec<u8>>, String>,
    ) -> Result<(), String> {
        let bytes = read(&path)?;
        self.files.push((path, bytes));
        Ok(())
    }

    pub(super) fn before(&self, path: &Path) -> Option<&Option<Vec<u8>>> {
        self.files
            .iter()
            .find(|(file, _)| file == path)
            .map(|(_, bytes)| bytes)
    }

    pub(super) fn optional(&self, path: &Path) -> Option<&[u8]> {
        self.before(path).and_then(|bytes| bytes.as_deref())
    }

    fn required(&self, path: &Path) -> Result<&[u8], String> {
        self.optional(path).ok_or_else(|| {
            log::warn!("PROJECT_SHARING_INVALID_STATE stage=read-input path={} reason=required-file-missing", path.display());
            "PROJECT_SHARING_INVALID_STATE".into()
        })
    }

    pub(super) fn json(&self, path: &Path) -> Result<Value, String> {
        serde_json::from_slice(self.required(path)?)
            .map_err(|error| super::diagnostics::invalid_json(path, error))
    }

    pub(super) fn store(&self, path: &Path) -> Result<AccountsStore, String> {
        let bytes = self.required(path)?;
        let store: AccountsStore = serde_json::from_slice(bytes)
            .map_err(|error| super::diagnostics::invalid_json(path, error))?;
        store.loaded_revision.set(Some(store_lock::digest(bytes)));
        Ok(store)
    }

    pub(super) fn ensure_unchanged(&self) -> Result<(), String> {
        for (path, bytes) in &self.files {
            let current = store_lock::read_optional(path).map_err(|error| {
                if super::session_files::is_session_path(path) {
                    super::diagnostics::sync_failed("verify-input", path, error)
                } else {
                    error
                }
            })?;
            if &current != bytes {
                let error = "PROJECT_SHARING_EXTERNAL_CHANGE".to_string();
                return Err(if super::session_files::is_session_path(path) {
                    super::diagnostics::sync_failed("verify-input", path, error)
                } else {
                    error
                });
            }
        }
        Ok(())
    }
}
