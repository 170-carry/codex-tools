use super::{diagnostics::SYNC_FAILED, PreservationResult};
use crate::models::SwitchAccountResult;
use std::future::Future;

pub(crate) async fn finish_switch<F>(
    attempt: Result<SwitchAccountResult, String>,
    switch_without_sync: impl FnOnce() -> F,
) -> Result<SwitchAccountResult, String>
where
    F: Future<Output = Result<SwitchAccountResult, String>>,
{
    let error = match attempt {
        Ok(result) => return Ok(result),
        Err(error) => error,
    };
    if !matches!(
        error.as_str(),
        SYNC_FAILED | "PROJECT_SHARING_UNSUPPORTED_IDENTITY" | "PROJECT_SHARING_UNSUPPORTED_ROUTE"
    ) {
        return Err(error);
    }
    log::warn!("{SYNC_FAILED} stage=switch action=continue-without-session-sync reason={error}");
    // The ordinary switch retains the existing pending-transaction gate.
    let mut result = switch_without_sync().await?;
    result.local_project_preservation = Some(PreservationResult {
        status: "skipped".into(),
        project_count: 0,
        thread_link_count: 0,
        unassigned_thread_count: 0,
        warning: Some(error),
    });
    Ok(result)
}
