//! Credential storage (auth.json): multiple Resend accounts, one active.
//!
//! Per-account invariant: an account holds EITHER an API key OR an OAuth
//! grant — saving one clears the other on the same account.
//!
//! Some functions are intentionally ahead of the frontend (multi-account
//! switcher is planned). Suppressing dead_code until the UI catches up.
#![allow(dead_code)]

use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Wry};
use uuid::Uuid;

use super::{delete_key, read_raw_key, read_string_key, write_key, AUTH_FILE};

// Serialize the entire read/modify/write transaction, not just store.set().
// Never hold this lock across network I/O.
static ACCOUNTS_WRITE_LOCK: Mutex<()> = Mutex::new(());

fn lock_accounts() -> Result<MutexGuard<'static, ()>, String> {
    ACCOUNTS_WRITE_LOCK.lock().map_err(|_| "[ERROR] Account store lock poisoned".into())
}

const ACCOUNTS_KEY: &str = "accounts";
const ACTIVE_ACCOUNT_KEY: &str = "active_account_id";
const CLIENT_ID_KEY: &str = "resend_oauth_client_id";
const API_KEY_RECORD: &str = "resend_api_key";
const OAUTH_RECORD: &str = "resend_oauth";
/// Legacy single-credential keys from pre-multi-account builds.
const LEGACY_KEYS: &[&str] = &["resend_api_key", "resend_oauth"];

/* ---------------------------------------------------------
 * Types
 * --------------------------------------------------------- */
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthRecord {
    #[serde(rename = "clientId")]
    pub client_id: String,
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
    #[serde(rename = "accessToken")]
    pub access_token: String,
    /// Unix timestamp (seconds) when the access token expires.
    #[serde(rename = "expiresAt")]
    pub expires_at: i64,
    /// Granted scopes, e.g. "full_access" or "emails:send".
    #[serde(rename = "scope", default)]
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountCredential {
    pub id: String,
    pub label: String,
    /// "oauth" | "api_key"
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuthRecord>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountMeta {
    pub id: String,
    pub label: String,
    pub method: String,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub enum ActiveCredential {
    ApiKey(String),
    OAuth(OAuthRecord),
}

/* ---------------------------------------------------------
 * Internals
 * --------------------------------------------------------- */
pub(crate) fn read_accounts(app: &AppHandle<Wry>) -> Result<Vec<AccountCredential>, String> {
    for key in LEGACY_KEYS {
        let _ = delete_key(app, AUTH_FILE, key);
    }

    match read_raw_key(app, AUTH_FILE, ACCOUNTS_KEY)? {
        Some(raw) => serde_json::from_value(raw.clone())
            .map_err(|e| format!("[ERROR] Failed to parse accounts: {e}")),
        None => Ok(Vec::new()),
    }
}

fn write_accounts(app: &AppHandle<Wry>, accounts: &[AccountCredential]) -> Result<(), String> {
    write_key(app, AUTH_FILE, ACCOUNTS_KEY, json!(accounts))
}

fn persist_with_active(
    app: &AppHandle<Wry>,
    accounts: &[AccountCredential],
    active_id: &str,
) -> Result<(), String> {
    write_accounts(app, accounts)?;
    write_key(app, AUTH_FILE, ACTIVE_ACCOUNT_KEY, json!(active_id))
}

pub(crate) fn read_active_id(
    app: &AppHandle<Wry>,
    accounts: &[AccountCredential],
) -> Result<Option<String>, String> {
    let stored = read_string_key(app, AUTH_FILE, ACTIVE_ACCOUNT_KEY)?;

    // Fallback: first account keeps the app usable if the pointer dangles.
    Ok(stored.filter(|id| accounts.iter().any(|a| a.id == *id)).or_else(|| accounts.first().map(|a| a.id.clone())))
}

/* ---------------------------------------------------------
 * Accessors
 * --------------------------------------------------------- */
/// Capture the legacy active pointer once at an operation boundary.
/// Explicit account-scoped callers must not use this fallback.
pub(crate) fn active_account_id(app: &AppHandle<Wry>) -> Result<String, String> {
    let accounts = read_accounts(app)?;
    read_active_id(app, &accounts)?.ok_or_else(|| "[ERROR] Not authenticated".into())
}

fn credential_for(accounts: &[AccountCredential], account_id: &str) -> Result<ActiveCredential, String> {
    let account = accounts.iter().find(|a| a.id == account_id)
        .ok_or_else(|| "[ERROR] Unknown account".to_string())?;
    match account.method.as_str() {
        "api_key" => account.api_key.clone().map(ActiveCredential::ApiKey),
        "oauth" => account.oauth.clone().map(ActiveCredential::OAuth),
        _ => None,
    }.ok_or_else(|| "[ERROR] Not authenticated".into())
}

pub(crate) fn account_credential(app: &AppHandle<Wry>, account_id: &str) -> Result<ActiveCredential, String> {
    credential_for(&read_accounts(app)?, account_id)
}

pub(crate) fn active_credential(
    app: &AppHandle<Wry>,
) -> Result<Option<ActiveCredential>, String> {
    let accounts = read_accounts(app)?;
    let active_id = read_active_id(app, &accounts)?;

    Ok(accounts
        .iter()
        .find(|a| Some(&a.id) == active_id.as_ref())
        .and_then(|a| match a.method.as_str() {
            "api_key" => a.api_key.clone().map(ActiveCredential::ApiKey),
            "oauth" => a.oauth.clone().map(ActiveCredential::OAuth),
            _ => None,
        }))
}

pub(crate) fn has_any_credential(app: &AppHandle<Wry>) -> Result<bool, String> {
    let accounts = read_accounts(app)?;
    Ok(accounts.iter().any(|account| credential_for(&accounts, &account.id).is_ok()))
}

pub(crate) fn active_label(app: &AppHandle<Wry>) -> Result<Option<String>, String> {
    let accounts = read_accounts(app)?;
    let active_id = read_active_id(app, &accounts)?;

    Ok(accounts
        .iter()
        .find(|a| Some(&a.id) == active_id.as_ref())
        .map(|a| a.label.clone()))
}

pub(crate) fn load_api_key(app: &AppHandle<Wry>) -> Result<Option<String>, String> {
    let accounts = read_accounts(app)?;
    let active_id = read_active_id(app, &accounts)?;

    Ok(accounts
        .iter()
        .find(|a| Some(&a.id) == active_id.as_ref() && a.method == "api_key")
        .and_then(|a| a.api_key.clone()))
}

pub(crate) fn load_oauth(app: &AppHandle<Wry>) -> Result<Option<OAuthRecord>, String> {
    let accounts = read_accounts(app)?;
    let active_id = read_active_id(app, &accounts)?;

    Ok(accounts
        .iter()
        .find(|a| Some(&a.id) == active_id.as_ref() && a.method == "oauth")
        .and_then(|a| a.oauth.clone()))
}

pub(crate) fn load_client_id(app: &AppHandle<Wry>) -> Result<Option<String>, String> {
    read_string_key(app, AUTH_FILE, CLIENT_ID_KEY)
}

pub(crate) fn save_client_id(app: &AppHandle<Wry>, client_id: &str) -> Result<(), String> {
    write_key(app, AUTH_FILE, CLIENT_ID_KEY, json!(client_id))
}

/* ---------------------------------------------------------
 * Mutations
 * --------------------------------------------------------- */
/// Upserts the OAuth grant on a specific account (rotation) or creates a
/// fresh account entry (new connect), marking it active.
pub fn upsert_account_oauth(
    app: &AppHandle<Wry>,
    account_id: Option<&str>,
    record: &OAuthRecord,
) -> Result<String, String> {
    let _guard = lock_accounts()?;
    let mut accounts = read_accounts(app)?;

    let target_id = account_id.and_then(|id| {
        accounts
            .iter()
            .position(|a| a.id == id)
            .map(|pos| accounts[pos].id.clone())
    });

    let id = match target_id {
        Some(existing) => {
            if let Some(a) = accounts.iter_mut().find(|a| a.id == existing) {
                a.method = "oauth".into();
                a.oauth = Some(record.clone());
                a.api_key = None;
            }
            existing
        }
        None => {
            let new_id = Uuid::new_v4().to_string();
            accounts.push(AccountCredential {
                id: new_id.clone(),
                label: format!("Account {}", accounts.len() + 1),
                method: "oauth".into(),
                api_key: None,
                oauth: Some(record.clone()),
            });
            new_id
        }
    };

    persist_with_active(app, &accounts, &id)?;
    Ok(id)
}

/// Replaces the active account's credential with an API key (creating the
/// entry when none exists).
pub fn upsert_api_key_active(app: &AppHandle<Wry>, api_key: &str) -> Result<(), String> {
    let _guard = lock_accounts()?;
    let mut accounts = read_accounts(app)?;
    let label = format!("Account {}", accounts.len() + 1);

    accounts.push(AccountCredential {
        id: uuid::Uuid::new_v4().to_string(),
        label,
        method: "api_key".into(),
        api_key: Some(api_key.to_string()),
        oauth: None,
    });

    let id = accounts.last().unwrap().id.clone();
    write_accounts(app, &accounts)?;
    write_key(app, AUTH_FILE, ACTIVE_ACCOUNT_KEY, json!(id))
}

fn disconnect_credential(accounts: &mut [AccountCredential], account_id: &str) -> Result<Option<OAuthRecord>, String> {
    let account = accounts.iter_mut().find(|a| a.id == account_id)
        .ok_or_else(|| "[ERROR] Unknown account".to_string())?;
    account.api_key = None;
    Ok(account.oauth.take())
}

pub(crate) fn disconnect_account(app: &AppHandle<Wry>, account_id: &str) -> Result<Option<OAuthRecord>, String> {
    let _guard = lock_accounts()?;
    let mut accounts = read_accounts(app)?;
    let previous = disconnect_credential(&mut accounts, account_id)?;
    write_accounts(app, &accounts)?;
    Ok(previous)
}

pub(crate) fn clear_oauth(app: &AppHandle<Wry>) -> Result<(), String> {
    delete_key(app, AUTH_FILE, OAUTH_RECORD)
}

/* ---------------------------------------------------------
 * Commands
 * --------------------------------------------------------- */
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    /// "oauth" | "api_key" | null
    pub method: Option<String>,
}

#[tauri::command]
pub fn get_connection_status(app: AppHandle<Wry>) -> Result<ConnectionStatus, String> {
    let method = active_credential(&app)?.map(|c| match c {
        ActiveCredential::ApiKey(_) => "api_key".to_string(),
        ActiveCredential::OAuth(_) => "oauth".to_string(),
    });

    Ok(ConnectionStatus { method })
}

#[tauri::command]
pub fn has_api_key(app: AppHandle<Wry>) -> Result<bool, String> {
    Ok(load_api_key(&app)?.is_some())
}

#[tauri::command]
pub fn save_api_key(app: AppHandle<Wry>, api_key: String) -> Result<(), String> {
    upsert_api_key_active(&app, &api_key)
}

#[tauri::command]
pub fn get_api_key(app: AppHandle<Wry>) -> Result<Option<String>, String> {
    load_api_key(&app)
}

#[tauri::command]
pub fn delete_api_key(app: AppHandle<Wry>, account_id: Option<String>) -> Result<(), String> {
    let _guard = lock_accounts()?;
    let account_id = match account_id {
        Some(id) => id,
        None => active_account_id(&app)?,
    };
    let mut accounts = read_accounts(&app)?;
    let account = accounts.iter_mut().find(|account| account.id == account_id)
        .ok_or_else(|| "[ERROR] Unknown account".to_string())?;
    account.api_key = None;
    write_accounts(&app, &accounts)
}

#[tauri::command]
pub fn list_accounts(app: AppHandle<Wry>) -> Result<Vec<AccountMeta>, String> {
    let accounts = read_accounts(&app)?;
    let active_id = read_active_id(&app, &accounts)?;

    Ok(accounts
        .iter()
        .map(|a| AccountMeta {
            id: a.id.clone(),
            label: a.label.clone(),
            method: a.method.clone(),
            is_active: Some(&a.id) == active_id.as_ref(),
        })
        .collect())
}

#[tauri::command]
pub fn set_active_account(app: AppHandle<Wry>, account_id: String) -> Result<(), String> {
    let _guard = lock_accounts()?;
    let accounts = read_accounts(&app)?;
    if !accounts.iter().any(|a| a.id == account_id) {
        return Err("[ERROR] Unknown account".to_string());
    }
    write_key(&app, AUTH_FILE, ACTIVE_ACCOUNT_KEY, json!(account_id))
}

fn remove_credential(accounts: &mut Vec<AccountCredential>, account_id: &str) -> Result<AccountCredential, String> {
    let pos = accounts.iter().position(|account| account.id == account_id)
        .ok_or_else(|| "[ERROR] Unknown account".to_string())?;
    Ok(accounts.remove(pos))
}

/// Internal only: secrets returned here must never cross the IPC boundary.
pub(crate) fn remove_account(
    app: AppHandle<Wry>,
    account_id: String,
) -> Result<Option<OAuthRecord>, String> {
    let _guard = lock_accounts()?;
    let mut accounts = read_accounts(&app)?;
    let removed = remove_credential(&mut accounts, &account_id)?;
    write_accounts(&app, &accounts)?;
    if read_string_key(&app, AUTH_FILE, ACTIVE_ACCOUNT_KEY)?.as_deref() == Some(&account_id) {
        if let Some(next) = accounts.first() {
            write_key(&app, AUTH_FILE, ACTIVE_ACCOUNT_KEY, json!(next.id))?;
        } else {
            delete_key(&app, AUTH_FILE, ACTIVE_ACCOUNT_KEY)?;
        }
    }

    println!("[INFO] Account removed: {} ({})", removed.label, removed.id);
    Ok(removed.oauth)
}

/// Saves the OAuth grant as a NEW account entry (fresh connect).
pub fn save_oauth(app: &AppHandle<Wry>, record: &OAuthRecord) -> Result<(), String> {
    let _guard = lock_accounts()?;
    let mut accounts = read_accounts(app)?;
    let label = format!("Account {}", accounts.len() + 1);

    accounts.push(AccountCredential {
        id: uuid::Uuid::new_v4().to_string(),
        label,
        method: "oauth".into(),
        api_key: None,
        oauth: Some(record.clone()),
    });

    let id = accounts.last().unwrap().id.clone();
    write_accounts(app, &accounts)?;
    write_key(app, AUTH_FILE, ACTIVE_ACCOUNT_KEY, json!(id))
}

/// Compare-and-swap prevents a late refresh from overwriting a replacement
/// credential or resurrecting a removed account. Never changes the active ID.
fn rotate_oauth(
    accounts: &mut [AccountCredential],
    account_id: &str,
    expected: &OAuthRecord,
    replacement: &OAuthRecord,
) -> Result<(), String> {
    let account = accounts.iter_mut().find(|a| a.id == account_id)
        .ok_or_else(|| "[ERROR] Unknown account".to_string())?;
    if account.method != "oauth" || account.oauth.as_ref() != Some(expected) {
        return Err("[ERROR] Account credential changed during refresh".into());
    }
    account.oauth = Some(replacement.clone());
    Ok(())
}

pub(crate) fn update_account_oauth(
    app: &AppHandle<Wry>,
    account_id: &str,
    expected: &OAuthRecord,
    replacement: &OAuthRecord,
) -> Result<(), String> {
    let _guard = lock_accounts()?;
    let mut accounts = read_accounts(app)?;
    rotate_oauth(&mut accounts, account_id, expected, replacement)?;
    write_accounts(app, &accounts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(token: &str) -> OAuthRecord {
        OAuthRecord { client_id: "client".into(), refresh_token: token.into(),
            access_token: token.into(), expires_at: 100, scope: "full_access".into() }
    }

    fn account(id: &str) -> AccountCredential {
        AccountCredential { id: id.into(), label: id.into(), method: "oauth".into(),
            api_key: None, oauth: Some(record(id)) }
    }

    #[test]
    fn logout_removes_account_without_affecting_other_credentials() {
        let mut accounts = vec![account("a"), account("b")];
        let removed = remove_credential(&mut accounts, "a").unwrap();
        assert_eq!(removed.oauth, Some(record("a")));
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, "b");
        assert_eq!(accounts[0].oauth, Some(record("b")));
        assert!(rotate_oauth(&mut accounts, "a", &record("a"), &record("late")).is_err());
        assert!(remove_credential(&mut accounts, "missing").is_err());
        assert_eq!(accounts.len(), 1);
        remove_credential(&mut accounts, "b").unwrap();
        assert!(accounts.is_empty());
    }

    #[test]
    fn refresh_only_updates_its_original_account() {
        let mut accounts = vec![account("a"), account("b")];
        rotate_oauth(&mut accounts, "a", &record("a"), &record("rotated")).unwrap();
        assert_eq!(accounts[0].oauth, Some(record("rotated")));
        assert_eq!(accounts[1].oauth, Some(record("b")));
    }

    #[test]
    fn removed_account_is_not_recreated_or_redirected() {
        let mut accounts = vec![account("b")];
        assert!(rotate_oauth(&mut accounts, "a", &record("a"), &record("late")).is_err());
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].oauth, Some(record("b")));
        assert!(credential_for(&accounts, "a").is_err());
    }

    #[test]
    fn replacement_grant_rejects_stale_refresh() {
        let mut accounts = vec![account("a")];
        accounts[0].oauth = Some(record("reconnected"));
        assert!(rotate_oauth(&mut accounts, "a", &record("a"), &record("late")).is_err());
        assert_eq!(accounts[0].oauth, Some(record("reconnected")));
    }

    #[test]
    fn disconnect_preserves_identity_and_other_account() {
        let mut accounts = vec![account("a"), account("b")];
        assert_eq!(disconnect_credential(&mut accounts, "a").unwrap(), Some(record("a")));
        assert_eq!(accounts[0].id, "a");
        assert_eq!(accounts[0].label, "a");
        assert!(credential_for(&accounts, "a").is_err());
        assert!(credential_for(&accounts, "b").is_ok());
        assert!(rotate_oauth(&mut accounts, "a", &record("a"), &record("late")).is_err());
        assert_eq!(disconnect_credential(&mut accounts, "a").unwrap(), None);
    }

    #[test]
    fn disconnect_api_key_and_unknown_account() {
        let mut accounts = vec![account("a")];
        accounts[0].method = "api_key".into();
        accounts[0].oauth = None;
        accounts[0].api_key = Some("test-key".into());
        assert!(disconnect_credential(&mut accounts, "missing").is_err());
        assert!(credential_for(&accounts, "a").is_ok());
        disconnect_credential(&mut accounts, "a").unwrap();
        assert!(credential_for(&accounts, "a").is_err());
    }

    #[test]
    fn duplicate_refresh_cannot_overwrite_rotated_tokens() {
        let mut accounts = vec![account("a")];
        rotate_oauth(&mut accounts, "a", &record("a"), &record("first")).unwrap();
        assert!(rotate_oauth(&mut accounts, "a", &record("a"), &record("late")).is_err());
        assert_eq!(accounts[0].oauth, Some(record("first")));
    }

    #[test]
    fn api_key_replacement_is_preserved() {
        let mut accounts = vec![account("a")];
        accounts[0].method = "api_key".into();
        accounts[0].oauth = None;
        accounts[0].api_key = Some("test-key".into());
        assert!(rotate_oauth(&mut accounts, "a", &record("a"), &record("late")).is_err());
        assert_eq!(accounts[0].api_key.as_deref(), Some("test-key"));
        assert!(accounts[0].oauth.is_none());
    }
}
