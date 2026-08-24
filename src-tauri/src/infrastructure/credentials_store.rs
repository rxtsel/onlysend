//! Credential storage (auth.json): multiple Resend accounts, one active.
//!
//! Per-account invariant: an account holds EITHER an API key OR an OAuth
//! grant — saving one clears the other on the same account.
//!
//! Some functions are intentionally ahead of the frontend (multi-account
//! switcher is planned). Suppressing dead_code until the UI catches up.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Wry};
use uuid::Uuid;

use super::{delete_key, read_raw_key, read_string_key, write_key, AUTH_FILE};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
fn read_accounts(app: &AppHandle<Wry>) -> Result<Vec<AccountCredential>, String> {
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

fn read_active_id(
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
    Ok(!read_accounts(app)?.is_empty())
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
pub fn delete_api_key(app: AppHandle<Wry>) -> Result<(), String> {
    delete_key(&app, AUTH_FILE, API_KEY_RECORD)
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
    let accounts = read_accounts(&app)?;
    if !accounts.iter().any(|a| a.id == account_id) {
        return Err("[ERROR] Unknown account".to_string());
    }
    write_key(&app, AUTH_FILE, ACTIVE_ACCOUNT_KEY, json!(account_id))
}

/// Removes the given account entirely. Returns its OAuth record so the
/// caller can revoke server-side beforehand.
#[tauri::command]
pub fn remove_account(
    app: AppHandle<Wry>,
    account_id: String,
) -> Result<Option<OAuthRecord>, String> {
    let mut accounts = read_accounts(&app)?;
    let pos = accounts.iter().position(|a| a.id == account_id);

    let Some(pos) = pos else {
        return Err("[ERROR] Unknown account".to_string());
    };

    let removed = accounts.remove(pos);
    let _next_active = accounts.first().map(|a| a.id.clone());
    write_accounts(&app, &accounts)?;

    println!("[INFO] Account removed: {} ({})", removed.label, removed.id);
    Ok(removed.oauth)
}

/// Saves the OAuth grant as a NEW account entry (fresh connect).
pub fn save_oauth(app: &AppHandle<Wry>, record: &OAuthRecord) -> Result<(), String> {
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

/// Updates the OAuth grant of the ACTIVE account in place (rotation path).
pub(crate) fn update_active_oauth(
    app: &AppHandle<Wry>,
    record: &OAuthRecord,
) -> Result<(), String> {
    let mut accounts = read_accounts(app)?;
    let active_id = read_active_id(app, &accounts)?;

    if let Some(a) = accounts
        .iter_mut()
        .find(|a| Some(&a.id) == active_id.as_ref())
    {
        a.method = "oauth".into();
        a.oauth = Some(record.clone());
        a.api_key = None;
    }

    let id = active_id.unwrap_or_default();
    persist_with_active(app, &accounts, &id)
}
