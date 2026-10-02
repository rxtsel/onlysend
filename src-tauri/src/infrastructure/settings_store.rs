//! Local data persisted in settings.json, scoped by explicit account ID.
//!
//! Entities like from_emails and read markers are keyed with the active
//! account ID. Global legacy values are preserved, never copied across accounts.
//!
//! This is the JSON adapter SQLite will replace (plan 011).

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Wry};

use super::{read_raw_key, read_string_key, read_typed, write_key, STORE_FILE};

const SETUP_COMPLETE_KEY: &str = "setup_complete";
const INBOX_ENABLED_KEY: &str = "inbox_enabled";
const FROM_EMAILS_KEY: &str = "from_emails";
const INBOUND_SETUP_CACHE_KEY: &str = "inbound_setup";
const READ_INBOUND_KEY: &str = "read_inbound_ids";
/// Cap so the read-marker list can't grow unbounded.
pub(crate) const READ_INBOUND_CAP: usize = 200;

/* ---------------------------------------------------------
 * Scoped key helpers
 * --------------------------------------------------------- */

/// Account-owned settings never consult the active pointer or a default bucket.
fn scoped_key(app: &AppHandle<Wry>, account_id: &str, base_key: &str) -> Result<String, String> {
    crate::infrastructure::credentials_store::account_credential(app, account_id)?;
    Ok(format!("{base_key}:{account_id}"))
}

fn write_scoped(
    app: &AppHandle<Wry>,
    account_id: &str,
    base_key: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    let key = scoped_key(app, account_id, base_key)?;
    write_key(app, STORE_FILE, &key, value)
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
pub fn get_onboarding_state(app: AppHandle<Wry>, account_id: String) -> Result<OnboardingState, String> {
    let complete = read_raw_key(&app, STORE_FILE, &scoped_key(&app, &account_id, SETUP_COMPLETE_KEY)?)?
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let authenticated =
        crate::infrastructure::credentials_store::account_credential(&app, &account_id).is_ok();

    let inbox_enabled = read_raw_key(&app, STORE_FILE, &scoped_key(&app, &account_id, INBOX_ENABLED_KEY)?)?
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(OnboardingState {
        // Existing account-scoped identities indicate a completed legacy setup.
        complete: complete || !load_from_emails(&app, &account_id)?.is_empty(),
        authenticated,
        inbox_enabled,
    })
}

/// Gates the Inbox nav item; updated by the receiving setup flow.
#[tauri::command]
pub fn set_inbox_enabled(app: AppHandle<Wry>, account_id: String, enabled: bool) -> Result<(), String> {
    write_scoped(&app, &account_id, INBOX_ENABLED_KEY, json!(enabled))
}

#[tauri::command]
pub fn mark_setup_complete(app: AppHandle<Wry>, account_id: String) -> Result<(), String> {
    write_scoped(&app, &account_id, SETUP_COMPLETE_KEY, json!(true))
}

/* ---------------------------------------------------------
 * Inbound setup cache
 * --------------------------------------------------------- */
#[tauri::command]
pub fn get_inbound_setup_cache(
    app: AppHandle<Wry>, account_id: String,
) -> Result<Option<serde_json::Value>, String> {
    let key = scoped_key(&app, &account_id, INBOUND_SETUP_CACHE_KEY)?;
    read_raw_key(&app, STORE_FILE, &key)
}

#[tauri::command]
pub fn save_inbound_setup_cache(
    app: AppHandle<Wry>, account_id: String,
    detail: serde_json::Value,
) -> Result<(), String> {
    let key = scoped_key(&app, &account_id, INBOUND_SETUP_CACHE_KEY)?;
    write_key(&app, STORE_FILE, &key, detail)
}

/* ---------------------------------------------------------
 * Selected domain
 * --------------------------------------------------------- */
#[tauri::command]
pub fn save_selected_domain(app: AppHandle<Wry>, account_id: String, domain: String) -> Result<(), String> {
    write_scoped(&app, &account_id, "selected_domain", json!(domain))
}

pub(crate) fn load_selected_domain(
    app: &AppHandle<Wry>,
    account_id: &str,
) -> Result<Option<String>, String> {
    read_string_key(app, STORE_FILE, &scoped_key(app, account_id, "selected_domain")?)
}

#[tauri::command]
pub fn get_active_domain(app: AppHandle<Wry>, account_id: String) -> Result<Option<String>, String> {
    load_selected_domain(&app, &account_id)
}

#[tauri::command]
pub fn get_selected_domain(app: AppHandle<Wry>, account_id: String) -> Result<Option<String>, String> {
    load_selected_domain(&app, &account_id)
}

/* ---------------------------------------------------------
 * From emails management (per-account)
 * --------------------------------------------------------- */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FromEmail {
    pub id: String,
    pub label: String,
    pub address: String,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
}

pub(crate) fn load_from_emails(app: &AppHandle<Wry>, account_id: &str) -> Result<Vec<FromEmail>, String> {
    let key = scoped_key(app, account_id, FROM_EMAILS_KEY)?;
    Ok(read_typed(app, STORE_FILE, &key, "from_emails")?.unwrap_or_default())
}

pub(crate) fn save_from_emails(
    app: &AppHandle<Wry>,
    account_id: &str,
    emails: &[FromEmail],
) -> Result<(), String> {
    let key = scoped_key(app, account_id, FROM_EMAILS_KEY)?;
    write_key(app, STORE_FILE, &key, json!(emails))
}

/* ---------------------------------------------------------
 * Inbound read markers (per-account)
 * --------------------------------------------------------- */
#[tauri::command]
pub fn get_read_inbound_ids(app: AppHandle<Wry>, account_id: String) -> Result<Vec<String>, String> {
    let key = scoped_key(&app, &account_id, READ_INBOUND_KEY)?;
    Ok(read_typed(&app, STORE_FILE, &key, "read ids")?.unwrap_or_default())
}

#[tauri::command]
pub fn mark_inbound_read(app: AppHandle<Wry>, account_id: String, email_id: String) -> Result<(), String> {
    let key = scoped_key(&app, &account_id, READ_INBOUND_KEY)?;
    let mut ids: Vec<String> =
        read_typed(&app, STORE_FILE, &key, "read ids")?.unwrap_or_default();

    if ids.contains(&email_id) {
        return Ok(());
    }

    ids.insert(0, email_id);
    ids.truncate(crate::infrastructure::settings_store::READ_INBOUND_CAP);

    write_key(&app, STORE_FILE, &key, json!(ids))
}
