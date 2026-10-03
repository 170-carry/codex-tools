//! Edit saved Relay accounts without exposing stored keys or changing the active desktop.
use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateApiAccountInput {
    pub(crate) label: String,
    pub(crate) base_url: String,
    pub(crate) model_name: String,
    pub(crate) api_key: Option<String>,
}

pub(crate) async fn update_api_account_internal(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    input: UpdateApiAccountInput,
) -> Result<AccountSummary, String> {
    let _guard = state.store_lock.lock().await;
    let mut store = load_store(app)?;
    let stored = store
        .accounts
        .iter_mut()
        .find(|account| account.id == id)
        .ok_or("找不到要编辑的 API 账号")?;
    let previous = stored.clone();
    update_relay_fields(stored, input)?;
    let path = account_store_path_from_data_dir(&app_paths::app_data_dir(app)?);
    if let Err(error) = profile_files::sync_account_profile_in_store_path(&path, stored) {
        let mut rollback = previous;
        let _ = profile_files::sync_account_profile_in_store_path(&path, &mut rollback);
        return Err(error);
    }
    let summary = stored.to_summary(
        current_auth_account_key().as_deref(),
        current_auth_variant_key().as_deref(),
    );
    if let Err(error) = save_store(app, &store) {
        let mut rollback = previous;
        let _ = profile_files::sync_account_profile_in_store_path(&path, &mut rollback);
        return Err(error);
    }
    Ok(summary)
}

fn update_relay_fields(
    account: &mut StoredAccount,
    input: UpdateApiAccountInput,
) -> Result<(), String> {
    if !matches!(account.source_kind, AccountSourceKind::Relay) {
        return Err("仅 API 中转站账号支持此编辑操作".into());
    }
    let label = profile_files::normalize_relay_label(&input.label)?;
    let base_url = profile_files::normalize_relay_base_url(&input.base_url)?;
    let model_name = profile_files::normalize_relay_model_name(&input.model_name)?;
    let api_key = match input.api_key.filter(|key| !key.trim().is_empty()) {
        Some(key) => profile_files::normalize_relay_api_key(&key)?,
        None => account
            .api_key
            .clone()
            .ok_or("API 账号缺少已保存的 Key，请重新输入")?,
    };
    account.label = label;
    account.api_base_url = Some(base_url);
    account.model_name = Some(model_name);
    account.auth_json = profile_files::build_api_auth_json(&api_key);
    account.api_key = Some(api_key);
    account.updated_at = now_unix_seconds();
    account.balance_text = None;
    account.profile_last_validated_at = None;
    account.profile_last_validation_error = None;
    account.usage_error = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editing_route_preserves_identity_and_key_unless_replaced() {
        let mut account: StoredAccount = serde_json::from_value(serde_json::json!({
            "id":"relay-id", "label":"Old", "sourceKind":"relay", "accountId":"relay:relay-id",
            "authJson":{"OPENAI_API_KEY":"old-secret", "auth_mode":"apikey"}, "apiKey":"old-secret",
            "apiBaseUrl":"https://old.invalid/v1", "modelName":"old-model", "addedAt":100, "updatedAt":100
        })).unwrap();
        let edit = |key| UpdateApiAccountInput {
            label: "New".into(),
            base_url: "http://192.0.2.2:8787/v1".into(),
            model_name: "gpt-6-sol".into(),
            api_key: key,
        };
        update_relay_fields(&mut account, edit(None)).unwrap();
        assert_eq!(account.id, "relay-id");
        assert_eq!(account.added_at, 100);
        assert_eq!(account.api_key.as_deref(), Some("old-secret"));
        assert_eq!(
            account.api_base_url.as_deref(),
            Some("http://192.0.2.2:8787/v1")
        );
        update_relay_fields(&mut account, edit(Some("new-secret".into()))).unwrap();
        assert_eq!(account.auth_json["OPENAI_API_KEY"], "new-secret");
        account.source_kind = AccountSourceKind::Chatgpt;
        assert!(update_relay_fields(&mut account, edit(None)).is_err());
    }
}
