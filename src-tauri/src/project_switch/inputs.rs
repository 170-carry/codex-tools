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
        self.optional(path)
            .ok_or_else(|| "PROJECT_SHARING_INVALID_STATE".into())
    }

    pub(super) fn json(&self, path: &Path) -> Result<Value, String> {
        serde_json::from_slice(self.required(path)?)
            .map_err(|_| "PROJECT_SHARING_INVALID_STATE".into())
    }

    pub(super) fn store(&self, path: &Path) -> Result<AccountsStore, String> {
        let bytes = self.required(path)?;
        let store: AccountsStore =
            serde_json::from_slice(bytes).map_err(|_| "PROJECT_SHARING_INVALID_STATE")?;
        store.loaded_revision.set(Some(store_lock::digest(bytes)));
        Ok(store)
    }

    pub(super) fn ensure_unchanged(&self) -> Result<(), String> {
        for (path, bytes) in &self.files {
            if &store_lock::read_optional(path)? != bytes {
                return Err("PROJECT_SHARING_EXTERNAL_CHANGE".into());
            }
        }
        Ok(())
    }
}
