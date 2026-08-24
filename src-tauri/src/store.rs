use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Wry};
use tauri_plugin_store::StoreExt;

pub(crate) const STORE_FILE: &str = "settings.json";
/// Dedicated credentials file for OAuth tokens. DO NOT SHARE.
const AUTH_FILE: &str = "auth.json";

const API_KEY_RECORD: &str = "resend_api_key";
const OAUTH_RECORD: &str = "resend_oauth";
const CLIENT_ID_KEY: &str = "resend_oauth_client_id";
const SELECTED_DOMAIN_KEY: &str = "selected_domain";
const SETUP_COMPLETE_KEY: &str = "setup_complete";
const FROM_EMAILS_KEY: &str = "from_emails";
const INBOX_ENABLED_KEY: &str = "inbox_enabled";
const READ_INBOUND_KEY: &str = "read_inbound_ids";
/// Cap so the read-marker list can't grow unbounded.
const READ_INBOUND_CAP: usize = 200;

/* ---------------------------------------------------------
 * Store primitives
 * --------------------------------------------------------- */
fn write_key(
    app: &AppHandle<Wry>,
    file: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    let store = app
        .store(file)
        .map_err(|e| format!("[ERROR] Failed to load {file}: {e}"))?;

    store.set(key, value);

    store
        .save()
        .map_err(|e| format!("[ERROR] Failed to save {file}: {e}"))?;

    Ok(())
}

fn delete_key(app: &AppHandle<Wry>, file: &str, key: &str) -> Result<(), String> {
    let store = app
        .store(file)
        .map_err(|e| format!("[ERROR] Failed to load {file}: {e}"))?;

    store.delete(key);

    store
        .save()
        .map_err(|e| format!("[ERROR] Failed to save {file}: {e}"))?;

    Ok(())
}

fn read_raw_key(
    app: &AppHandle<Wry>,
    file: &str,
    key: &str,
) -> Result<Option<serde_json::Value>, String> {
    let store = app
        .store(file)
        .map_err(|e| format!("[ERROR] Failed to load {file}: {e}"))?;

    Ok(store.get(key))
}

fn read_string_key(
    app: &AppHandle<Wry>,
    file: &str,
    key: &str,
) -> Result<Option<String>, String> {
    Ok(read_raw_key(app, file, key)?.and_then(|v| v.as_str().map(String::from)))
}

fn read_typed<T: for<'de> Deserialize<'de>>(
    app: &AppHandle<Wry>,
    file: &str,
    key: &str,
    label: &str,
) -> Result<Option<T>, String> {
    match read_raw_key(app, file, key)? {
        Some(raw) => serde_json::from_value(raw.clone())
            .map(Some)
            .map_err(|e| format!("[ERROR] Failed to parse {label}: {e}")),
        None => Ok(None),
    }
}

/* ---------------------------------------------------------
 * Api Key Management (credentials: auth.json)
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
 * OAuth Management (credentials: auth.json)
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

/* ---------------------------------------------------------
 * Connection status
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

/* ---------------------------------------------------------
 * Onboarding state
 * --------------------------------------------------------- */
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingState {
    pub complete: bool,
    /// True when a credential (API key or OAuth) already exists.
    pub authenticated: bool,
    /// Gates the Inbox nav item: true once receiving is set up.
    #[serde(default)]
    pub inbox_enabled: bool,
}

#[tauri::command]
pub fn get_onboarding_state(app: AppHandle<Wry>) -> Result<OnboardingState, String> {
    let complete = read_raw_key(&app, STORE_FILE, SETUP_COMPLETE_KEY)?
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let authenticated = load_api_key(&app)?.is_some() || load_oauth(&app)?.is_some();

    let inbox_enabled = read_raw_key(&app, STORE_FILE, INBOX_ENABLED_KEY)?
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(OnboardingState {
        complete,
        authenticated,
        inbox_enabled,
    })
}

/// Gates the Inbox nav item; updated by the receiving setup flow.
#[tauri::command]
pub fn set_inbox_enabled(app: AppHandle<Wry>, enabled: bool) -> Result<(), String> {
    write_key(&app, STORE_FILE, INBOX_ENABLED_KEY, json!(enabled))
}

#[tauri::command]
pub fn mark_setup_complete(app: AppHandle<Wry>) -> Result<(), String> {
    write_key(&app, STORE_FILE, SETUP_COMPLETE_KEY, json!(true))
}

pub(crate) fn mark_setup_incomplete(app: AppHandle<Wry>) -> Result<(), String> {
    write_key(&app, STORE_FILE, SETUP_COMPLETE_KEY, json!(false))
}

/* ---------------------------------------------------------
 * Inbound setup cache
 *
 * Last-known domain detail (records + statuses) so the setup card
 * renders instantly without hitting Resend on every open. Refreshed
 * explicitly by the UI (open / Verify now).
 * --------------------------------------------------------- */
const INBOUND_SETUP_CACHE_KEY: &str = "inbound_setup";

#[tauri::command]
pub fn get_inbound_setup_cache(
    app: AppHandle<Wry>,
) -> Result<Option<serde_json::Value>, String> {
    read_raw_key(&app, STORE_FILE, INBOUND_SETUP_CACHE_KEY)
}

#[tauri::command]
pub fn save_inbound_setup_cache(
    app: AppHandle<Wry>,
    detail: serde_json::Value,
) -> Result<(), String> {
    write_key(&app, STORE_FILE, INBOUND_SETUP_CACHE_KEY, detail)
}

/* ---------------------------------------------------------
 * Selected domain
 * --------------------------------------------------------- */
#[tauri::command]
pub fn save_selected_domain(app: AppHandle<Wry>, domain: String) -> Result<(), String> {
    write_key(&app, STORE_FILE, SELECTED_DOMAIN_KEY, json!(domain))
}

#[tauri::command]
pub fn get_active_domain(app: AppHandle<Wry>) -> Result<Option<String>, String> {
    read_string_key(&app, STORE_FILE, SELECTED_DOMAIN_KEY)
}

/* ---------------------------------------------------------
 * From emails management
 * --------------------------------------------------------- */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FromEmail {
    pub id: String,
    pub label: String,
    pub address: String,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
}

pub(crate) fn load_from_emails(app: &AppHandle<Wry>) -> Result<Vec<FromEmail>, String> {
    Ok(read_typed(app, STORE_FILE, FROM_EMAILS_KEY, "from_emails")?.unwrap_or_default())
}

pub(crate) fn save_from_emails(app: &AppHandle<Wry>, emails: &[FromEmail]) -> Result<(), String> {
    write_key(app, STORE_FILE, FROM_EMAILS_KEY, json!(emails))
}

/* ---------------------------------------------------------
 * Inbound read markers
 * --------------------------------------------------------- */
#[tauri::command]
pub fn get_read_inbound_ids(app: AppHandle<Wry>) -> Result<Vec<String>, String> {
    Ok(read_typed(&app, STORE_FILE, READ_INBOUND_KEY, "read ids")?.unwrap_or_default())
}

#[tauri::command]
pub fn mark_inbound_read(app: AppHandle<Wry>, email_id: String) -> Result<(), String> {
    let mut ids: Vec<String> =
        read_typed(&app, STORE_FILE, READ_INBOUND_KEY, "read ids")?.unwrap_or_default();

    if ids.contains(&email_id) {
        return Ok(());
    }

    ids.insert(0, email_id);
    ids.truncate(READ_INBOUND_CAP);

    write_key(&app, STORE_FILE, READ_INBOUND_KEY, json!(ids))
}
