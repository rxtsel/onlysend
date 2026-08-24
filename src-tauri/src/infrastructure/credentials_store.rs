//! Credential storage (auth.json): API key, OAuth grant, OAuth client id.
//!
//! Single-credential invariant: saving one credential kind clears the other.
//! This module is the port multi-account will reimplement.

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Wry};

use super::{
    delete_key, read_string_key, read_typed, write_key, AUTH_FILE,
};

const API_KEY_RECORD: &str = "resend_api_key";
const OAUTH_RECORD: &str = "resend_oauth";
const CLIENT_ID_KEY: &str = "resend_oauth_client_id";

/* ---------------------------------------------------------
 * Api Key
 * --------------------------------------------------------- */
#[tauri::command]
pub fn has_api_key(app: AppHandle<Wry>) -> Result<bool, String> {
    Ok(load_api_key(&app)?.is_some())
}

#[tauri::command]
pub fn save_api_key(app: AppHandle<Wry>, api_key: String) -> Result<(), String> {
    // Single-credential invariant: an API key replaces any OAuth grant.
    clear_oauth(&app)?;
    write_key(&app, AUTH_FILE, API_KEY_RECORD, json!(api_key))
}

pub(crate) fn load_api_key(app: &AppHandle<Wry>) -> Result<Option<String>, String> {
    read_string_key(app, AUTH_FILE, API_KEY_RECORD)
}

#[tauri::command]
pub fn get_api_key(app: AppHandle<Wry>) -> Result<Option<String>, String> {
    match load_api_key(&app)? {
        Some(api_key) => {
            println!("[INFO] API key retrieved successfully");
            Ok(Some(api_key))
        }
        None => {
            println!("[INFO]  API key not found");
            Ok(None)
        }
    }
}

#[tauri::command]
pub fn delete_api_key(app: AppHandle<Wry>) -> Result<(), String> {
    delete_key(&app, AUTH_FILE, API_KEY_RECORD)?;
    println!("[INFO] API key deleted successfully");
    Ok(())
}

/* ---------------------------------------------------------
 * OAuth
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

pub(crate) fn load_oauth(app: &AppHandle<Wry>) -> Result<Option<OAuthRecord>, String> {
    read_typed(app, AUTH_FILE, OAUTH_RECORD, "oauth record")
}

pub(crate) fn save_oauth(app: &AppHandle<Wry>, record: &OAuthRecord) -> Result<(), String> {
    let value = serde_json::to_value(record).map_err(|e| e.to_string())?;
    write_key(app, AUTH_FILE, OAUTH_RECORD, value)
}

pub(crate) fn clear_oauth(app: &AppHandle<Wry>) -> Result<(), String> {
    delete_key(app, AUTH_FILE, OAUTH_RECORD)
}

pub(crate) fn load_client_id(app: &AppHandle<Wry>) -> Result<Option<String>, String> {
    read_string_key(app, AUTH_FILE, CLIENT_ID_KEY)
}

pub(crate) fn save_client_id(app: &AppHandle<Wry>, client_id: &str) -> Result<(), String> {
    write_key(app, AUTH_FILE, CLIENT_ID_KEY, json!(client_id))
}

/// True when any credential kind exists (used by onboarding gating).
pub(crate) fn has_any_credential(app: &AppHandle<Wry>) -> Result<bool, String> {
    Ok(load_api_key(app)?.is_some() || load_oauth(app)?.is_some())
}

/* ---------------------------------------------------------
 * Connection status (spans both credential kinds)
 * --------------------------------------------------------- */
#[derive(Debug, Serialize)]
pub struct ConnectionStatus {
    /// "oauth" | "api_key" | null
    pub method: Option<String>,
}

#[tauri::command]
pub fn get_connection_status(app: AppHandle<Wry>) -> Result<ConnectionStatus, String> {
    let has_key = load_api_key(&app)?.is_some();
    let has_oauth = load_oauth(&app)?.is_some();

    Ok(ConnectionStatus {
        method: if has_oauth {
            Some("oauth".to_string())
        } else if has_key {
            Some("api_key".to_string())
        } else {
            None
        },
    })
}
