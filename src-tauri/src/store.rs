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
const FROM_EMAILS_KEY: &str = "from_emails";
const PROFILE_KEY: &str = "profile";

// Api Key Management
#[tauri::command]
pub fn has_api_key(app: AppHandle<Wry>) -> Result<bool, String> {
    Ok(load_api_key(&app)?.is_some())
}

#[tauri::command]
pub fn save_api_key(app: AppHandle<Wry>, api_key: String) -> Result<(), String> {
    let auth = app
        .store(AUTH_FILE)
        .map_err(|e| format!("[ERROR] Failed to load auth store: {}", e))?;

    auth.set(API_KEY_RECORD, json!(api_key));

    auth.save()
        .map_err(|e| format!("[ERROR] Failed to save auth store: {}", e))?;

    Ok(())
}

pub(crate) fn load_api_key(app: &AppHandle<Wry>) -> Result<Option<String>, String> {
    let auth = app
        .store(AUTH_FILE)
        .map_err(|e| format!("[ERROR] Failed to load auth store: {}", e))?;

    if let Some(value) = auth.get(API_KEY_RECORD) {
        return Ok(value.as_str().map(|s| s.to_string()));
    }
    drop(auth);

    // One-time migration: keys saved before auth.json existed.
    let settings = app
        .store(STORE_FILE)
        .map_err(|e| format!("[ERROR] Failed to load store: {}", e))?;

    let legacy = match settings.get(API_KEY_RECORD) {
        Some(value) => value.as_str().map(|s| s.to_string()),
        None => return Ok(None),
    };

    if let Some(key) = &legacy {
        save_api_key(app.clone(), key.clone())?;
        settings.delete(API_KEY_RECORD);
        settings
            .save()
            .map_err(|e| format!("[ERROR] Failed to save store: {}", e))?;
        println!("[INFO] Migrated API key from settings.json to auth.json");
    }

    Ok(legacy)
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
    // Remove from both files in case a legacy copy still exists.
    for (file, label) in [(AUTH_FILE, "auth store"), (STORE_FILE, "store")] {
        let store = app
            .store(file)
            .map_err(|e| format!("[ERROR] Failed to load {}: {}", label, e))?;

        store.delete(API_KEY_RECORD);

        store
            .save()
            .map_err(|e| format!("[ERROR] Failed to save {}: {}", label, e))?;
    }

    println!("[INFO] API key deleted successfully");

    Ok(())
}

// OAuth Management
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
    let auth = app
        .store(AUTH_FILE)
        .map_err(|e| format!("[ERROR] Failed to load auth store: {}", e))?;

    match auth.get(OAUTH_RECORD) {
        Some(value) => {
            let record: OAuthRecord = serde_json::from_value(value.clone())
                .map_err(|e| format!("[ERROR] Failed to parse oauth record: {}", e))?;
            Ok(Some(record))
        }
        None => Ok(None),
    }
}

pub(crate) fn save_oauth(app: &AppHandle<Wry>, record: &OAuthRecord) -> Result<(), String> {
    let auth = app
        .store(AUTH_FILE)
        .map_err(|e| format!("[ERROR] Failed to load auth store: {}", e))?;

    auth.set(OAUTH_RECORD, serde_json::to_value(record).map_err(|e| e.to_string())?);

    auth.save()
        .map_err(|e| format!("[ERROR] Failed to save auth store: {}", e))?;

    Ok(())
}

pub(crate) fn clear_oauth(app: &AppHandle<Wry>) -> Result<(), String> {
    let auth = app
        .store(AUTH_FILE)
        .map_err(|e| format!("[ERROR] Failed to load auth store: {}", e))?;

    auth.delete(OAUTH_RECORD);

    auth.save()
        .map_err(|e| format!("[ERROR] Failed to save auth store: {}", e))?;

    Ok(())
}

pub(crate) fn load_client_id(app: &AppHandle<Wry>) -> Result<Option<String>, String> {
    let auth = app
        .store(AUTH_FILE)
        .map_err(|e| format!("[ERROR] Failed to load auth store: {}", e))?;

    match auth.get(CLIENT_ID_KEY) {
        Some(value) => Ok(value.as_str().map(|s| s.to_string())),
        None => Ok(None),
    }
}

pub(crate) fn save_client_id(app: &AppHandle<Wry>, client_id: &str) -> Result<(), String> {
    let auth = app
        .store(AUTH_FILE)
        .map_err(|e| format!("[ERROR] Failed to load auth store: {}", e))?;

    auth.set(CLIENT_ID_KEY, json!(client_id));

    auth.save()
        .map_err(|e| format!("[ERROR] Failed to save auth store: {}", e))?;

    Ok(())
}

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

/// True when the user completed onboarding with either auth method.
#[tauri::command]
pub fn is_authenticated(app: AppHandle<Wry>) -> Result<bool, String> {
    let has_key = load_api_key(&app)?;
    let has_oauth = load_oauth(&app)?;

    Ok(has_key.is_some() || has_oauth.is_some())
}

// Selected domain
#[tauri::command]
pub fn save_selected_domain(app: AppHandle<Wry>, domain: String) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("[ERROR] Failed to load store: {}", e))?;

    store.set(SELECTED_DOMAIN_KEY, json!(domain));

    store
        .save()
        .map_err(|e| format!("[ERROR] Failed to save store: {}", e))?;

    Ok(())
}

pub(crate) fn load_selected_domain(app: &AppHandle<Wry>) -> Result<Option<String>, String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("[ERROR] Failed to load store: {}", e))?;

    match store.get(SELECTED_DOMAIN_KEY) {
        Some(value) => Ok(value.as_str().map(|s| s.to_string())),
        None => Ok(None),
    }
}

#[tauri::command]
pub fn get_selected_domain(app: AppHandle<Wry>) -> Result<Option<String>, String> {
    load_selected_domain(&app)
}

// From emails management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FromEmail {
    pub id: String,
    pub label: String,
    pub address: String,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
}

pub(crate) fn load_from_emails(app: &AppHandle<Wry>) -> Result<Vec<FromEmail>, String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("[ERROR] Failed to load store: {}", e))?;

    let value = store.get(FROM_EMAILS_KEY);

    if let Some(raw) = value {
        let emails: Vec<FromEmail> = serde_json::from_value(raw.clone())
            .map_err(|e| format!("[ERROR] Failed to parse from_emails: {}", e))?;
        Ok(emails)
    } else {
        Ok(Vec::new())
    }
}

pub(crate) fn save_from_emails(app: &AppHandle<Wry>, emails: &[FromEmail]) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("[ERROR] Failed to load store: {}", e))?;

    store.set(FROM_EMAILS_KEY, json!(emails));

    store
        .save()
        .map_err(|e| format!("[ERROR] Failed to save store: {}", e))?;

    Ok(())
}

// Profile Management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    #[serde(rename = "firstName")]
    pub first_name: String,

    #[serde(rename = "lastName")]
    pub last_name: String,
    pub username: String,
    pub domain: String,
}

pub(crate) fn load_profile(app: &AppHandle<Wry>) -> Result<Option<Profile>, String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("[ERROR] Failed to load store: {}", e))?;

    let value = store.get(PROFILE_KEY);

    if let Some(raw) = value {
        let profile: Profile = serde_json::from_value(raw.clone())
            .map_err(|e| format!("[ERROR] Failed to parse profile: {}", e))?;
        Ok(Some(profile))
    } else {
        Ok(None)
    }
}

pub(crate) fn save_profile(app: &AppHandle<Wry>, profile: &Profile) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("[ERROR] Failed to load store: {}", e))?;

    store.set(PROFILE_KEY, json!(profile));

    store
        .save()
        .map_err(|e| format!("[ERROR] Failed to save store: {}", e))?;

    Ok(())
}
