use super::{invalid, StateError};
use base64::{
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    Engine,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Derives the renderer's owner key from identity claims only. This is parsing,
/// not JWT signature verification; callers supply their already selected auth.
pub fn owner_from_auth(auth: &Value) -> Result<String, StateError> {
    let tokens = auth
        .get("tokens")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("auth.tokens must be an object"))?;
    let mut users = BTreeSet::new();
    let mut accounts = BTreeSet::new();
    add_identity(tokens.get("account_id"), &mut accounts)?;
    for key in ["id_token", "access_token"] {
        let Some(value) = tokens.get(key).filter(|value| !value.is_null()) else {
            continue;
        };
        let jwt = value
            .as_str()
            .ok_or_else(|| invalid("identity token must be a string"))?;
        let parts: Vec<_> = jwt.split('.').collect();
        if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
            return Err(invalid("malformed identity token"));
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(parts[1])
            .or_else(|_| URL_SAFE.decode(parts[1]))
            .map_err(|_| invalid("malformed identity token payload"))?;
        let claims: Value = serde_json::from_slice(&bytes)
            .map_err(|_| invalid("malformed identity token claims"))?;
        let claims = claims
            .as_object()
            .ok_or_else(|| invalid("identity claims must be an object"))?;
        if let Some(claim) = claims.get("https://api.openai.com/auth") {
            let claim = claim
                .as_object()
                .ok_or_else(|| invalid("auth identity claim must be an object"))?;
            add_identity(claim.get("chatgpt_user_id"), &mut users)?;
            add_identity(claim.get("user_id"), &mut users)?;
            add_identity(claim.get("chatgpt_account_id"), &mut accounts)?;
        }
    }
    if users.len() != 1 || accounts.len() != 1 {
        return Err(invalid("missing or contradictory account identity"));
    }
    // serde_json's compact string-array encoding matches JSON.stringify here:
    // there are no object key ordering or numeric formatting differences.
    let tuple = serde_json::to_vec(&[users.first().unwrap(), accounts.first().unwrap()])
        .map_err(|_| invalid("could not encode account identity"))?;
    Ok(format!("{:x}", Sha256::digest(tuple)))
}

fn add_identity(
    value: Option<&Value>,
    identities: &mut BTreeSet<String>,
) -> Result<(), StateError> {
    if let Some(value) = value.filter(|value| !value.is_null()) {
        let identity = value
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| invalid("identity must be a nonempty string"))?;
        identities.insert(identity.to_owned());
    }
    Ok(())
}

pub(super) fn validate_owner(owner: &str) -> Result<(), StateError> {
    if owner.len() != 64
        || !owner
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid(
            "account owner must be 64 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

pub(super) fn validate_host(host: &str) -> Result<(), StateError> {
    let home = host
        .strip_prefix("local:")
        .filter(|home| !home.trim().is_empty())
        .ok_or_else(|| invalid("host must identify an explicit local Codex home"))?;
    if home.chars().any(char::is_control) {
        return Err(invalid("invalid local host identity"));
    }
    Ok(())
}
