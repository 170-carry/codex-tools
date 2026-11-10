use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::{
    params_from_iter,
    types::{Value, ValueRef},
    Connection, OpenFlags,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

// host_key is the leading column of Chromium's cookies_unique_index. Both
// selection and deletion touch only Claude's authentication cookies.
const FILTER: &str = "host_key IN ('.claude.ai','claude.ai','.claude.com','claude.com') AND name IN ('sessionKey','sessionKeyLC','sessionKeyV3','sessionKeyV3LC','lastActiveOrg','routingHint')";

#[derive(Clone, Serialize, Deserialize)]
pub struct Cookies {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
}
#[derive(Clone, Serialize, Deserialize)]
pub enum Cell {
    Text(String),
    Integer(i64),
    Real(f64),
    Blob(String),
    Null,
}
impl Cell {
    fn value(&self) -> Result<Value, String> {
        Ok(match self {
            Self::Text(v) => Value::Text(v.clone()),
            Self::Integer(v) => Value::Integer(*v),
            Self::Real(v) => Value::Real(*v),
            Self::Blob(v) => Value::Blob(STANDARD.decode(v).map_err(|e| e.to_string())?),
            Self::Null => Value::Null,
        })
    }
}

pub fn read(path: &Path) -> Result<Cookies, String> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    db.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|e| e.to_string())?;
    let mut statement = db
        .prepare(&format!("SELECT * FROM cookies WHERE {FILTER}"))
        .map_err(|e| e.to_string())?;
    let columns = statement
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let rows = statement
        .query_map([], |row| {
            (0..columns.len())
                .map(|i| {
                    Ok(match row.get_ref(i)? {
                        ValueRef::Text(v) => Cell::Text(String::from_utf8_lossy(v).into_owned()),
                        ValueRef::Integer(v) => Cell::Integer(v),
                        ValueRef::Real(v) => Cell::Real(v),
                        ValueRef::Blob(v) => Cell::Blob(STANDARD.encode(v)),
                        ValueRef::Null => Cell::Null,
                    })
                })
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(Cookies { columns, rows })
}

pub fn restore(path: &Path, cookies: &Cookies) -> Result<(), String> {
    let mut db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| e.to_string())?;
    db.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|e| e.to_string())?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    tx.execute(&format!("DELETE FROM cookies WHERE {FILTER}"), [])
        .map_err(|e| e.to_string())?;
    let names = cookies
        .columns
        .iter()
        .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(",");
    let placeholders = vec!["?"; cookies.columns.len()].join(",");
    // A handful of primary/unique-key inserts in one short transaction; no scans.
    let sql = format!("INSERT OR REPLACE INTO cookies ({names}) VALUES ({placeholders})");
    for row in &cookies.rows {
        let values = row.iter().map(Cell::value).collect::<Result<Vec<_>, _>>()?;
        tx.execute(&sql, params_from_iter(values))
            .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn has_session(cookies: &Cookies) -> bool {
    let Some(index) = cookies.columns.iter().position(|s| s == "name") else {
        return false;
    };
    cookies
        .rows
        .iter()
        .any(|r| matches!(&r[index], Cell::Text(s) if s == "sessionKey" || s == "sessionKeyV3"))
}

#[cfg(target_os = "macos")]
pub(super) fn decrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
    let password =
        security_framework::passwords::get_generic_password("Claude Safe Storage", "Claude Key")
            .map_err(|e| format!("读取 Claude 桌面加密密钥失败: {e}"))?;
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(&password, b"saltysalt", 1003, &mut key);
    if !data.starts_with(b"v10") {
        return Err("无法识别 Claude 桌面 Cookie 加密格式".into());
    }
    cbc::Decryptor::<aes::Aes128>::new((&key).into(), (&[b' '; 16]).into())
        .decrypt_padded_vec_mut::<Pkcs7>(&data[3..])
        .map_err(|_| "Claude 桌面 Cookie 解密失败".into())
}

pub fn active_org(cookies: &Cookies) -> Result<Option<String>, String> {
    let col = |name: &str| {
        cookies
            .columns
            .iter()
            .position(|s| s == name)
            .ok_or_else(|| format!("Cookie 缺少 {name}"))
    };
    let (name, value, encrypted) = (col("name")?, col("value")?, col("encrypted_value")?);
    for row in &cookies.rows {
        if !matches!(&row[name], Cell::Text(s) if s == "lastActiveOrg") {
            continue;
        }
        if let Cell::Text(s) = &row[value] {
            if !s.is_empty() {
                return Ok(Some(s.clone()));
            }
        }
        #[cfg(target_os = "macos")]
        if let Cell::Blob(s) = &row[encrypted] {
            let clear = decrypt(&STANDARD.decode(s).map_err(|e| e.to_string())?)?;
            // Chromium cookie DB v24 prefixes SHA-256(host_key) before the value.
            let tail = if clear.len() == 68 {
                &clear[32..]
            } else {
                &clear[..]
            };
            return Ok(Some(
                String::from_utf8(tail.to_vec()).map_err(|e| e.to_string())?,
            ));
        }
        #[cfg(not(target_os = "macos"))]
        let _ = encrypted;
    }
    Ok(None)
}

#[cfg(test)]
#[path = "cookies_tests.rs"]
mod tests;

pub fn session_header(cookies: &Cookies) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let col = |name: &str| {
        cookies
            .columns
            .iter()
            .position(|s| s == name)
            .ok_or_else(|| format!("Cookie 缺少 {name}"))
    };
    let (name, value, encrypted, host) = (
        col("name")?,
        col("value")?,
        col("encrypted_value")?,
        col("host_key")?,
    );
    let mut pairs = std::collections::BTreeMap::new();
    for row in &cookies.rows {
        let Cell::Text(n) = &row[name] else { continue };
        if !matches!(
            n.as_str(),
            "sessionKey" | "sessionKeyV3" | "sessionKeyLC" | "sessionKeyV3LC"
        ) {
            continue;
        }
        let clear = match &row[value] {
            Cell::Text(s) if !s.is_empty() => s.clone(),
            _ => {
                #[cfg(target_os = "macos")]
                {
                    let Cell::Blob(encoded) = &row[encrypted] else {
                        continue;
                    };
                    let bytes = decrypt(&STANDARD.decode(encoded).map_err(|e| e.to_string())?)?;
                    let Cell::Text(domain) = &row[host] else {
                        continue;
                    };
                    let hash = Sha256::digest(domain.as_bytes());
                    let bytes = bytes.strip_prefix(hash.as_slice()).unwrap_or(&bytes);
                    String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string())?
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = (encrypted, host);
                    return Err("此平台请使用 OAuth 授权以查询桌面账号用量".into());
                }
            }
        };
        pairs.insert(n.clone(), clear);
    }
    if pairs.is_empty() {
        return Err("桌面登录已失效，请在 Claude App 登录后重新导入".into());
    }
    Ok(pairs
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; "))
}
