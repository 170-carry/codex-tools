use std::path::Path;

pub(super) const SYNC_FAILED: &str = "PROJECT_SHARING_SYNC_FAILED";

pub(super) fn invalid_json(path: &Path, error: serde_json::Error) -> String {
    log::warn!(
        "PROJECT_SHARING_INVALID_STATE stage=parse-json path={} category={:?} line={} column={} reason={}",
        path.display(),
        error.classify(),
        error.line(),
        error.column(),
        json_reason(&error),
    );
    "PROJECT_SHARING_INVALID_STATE".into()
}

fn json_reason(error: &serde_json::Error) -> String {
    // Deserialization errors can contain a credential value from the input.
    if error.is_data() {
        "JSON does not match the expected schema".into()
    } else {
        error.to_string()
    }
}

pub(super) fn sync_failed(stage: &str, path: &Path, error: impl std::fmt::Display) -> String {
    log::warn!(
        "{SYNC_FAILED} stage={stage} path={} reason={error}",
        path.display()
    );
    SYNC_FAILED.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_errors_do_not_log_input_values() {
        let error = serde_json::from_str::<bool>(r#""synthetic-private-token""#).unwrap_err();
        assert!(!json_reason(&error).contains("synthetic-private-token"));
        let error = serde_json::from_str::<serde_json::Value>("{\n broken").unwrap_err();
        assert!(json_reason(&error).contains("line 2"));
    }
}
