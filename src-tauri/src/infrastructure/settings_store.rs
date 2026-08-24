//! Local data persisted in settings.json: onboarding flags, selected
//! domain, sender identities, inbound read markers and the receiving
//! setup cache. This is the JSON adapter SQLite will replace (plan 011).

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Wry};

use super::{
    read_raw_key, read_string_key, read_typed, write_key, STORE_FILE,
};

const SELECTED_DOMAIN_KEY: &str = "selected_domain";
const SETUP_COMPLETE_KEY: &str = "setup_complete";
const INBOX_ENABLED_KEY: &str = "inbox_enabled";
const FROM_EMAILS_KEY: &str = "from_emails";
const INBOUND_SETUP_CACHE_KEY: &str = "inbound_setup";
const READ_INBOUND_KEY: &str = "read_inbound_ids";
/// Cap so the read-marker list can't grow unbounded.
pub(crate) const READ_INBOUND_CAP: usize = 200;

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

fn authenticated(app: &AppHandle<Wry>) -> Result<bool, String> {
    // Defined in credentials_store; re-exported there to avoid a cycle.
    crate::infrastructure::credentials_store::has_any_credential(app)
}

#[tauri::command]
pub fn get_onboarding_state(app: AppHandle<Wry>) -> Result<OnboardingState, String> {
    let complete = read_raw_key(&app, STORE_FILE, SETUP_COMPLETE_KEY)?
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let inbox_enabled = read_raw_key(&app, STORE_FILE, INBOX_ENABLED_KEY)?
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(OnboardingState {
        complete,
        authenticated: authenticated(&app)?,
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
 * --------------------------------------------------------- */
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

pub(crate) fn load_selected_domain(app: &AppHandle<Wry>) -> Result<Option<String>, String> {
    read_string_key(app, STORE_FILE, SELECTED_DOMAIN_KEY)
}

#[tauri::command]
pub fn get_active_domain(app: AppHandle<Wry>) -> Result<Option<String>, String> {
    load_selected_domain(&app)
}

#[tauri::command]
pub fn get_selected_domain(app: AppHandle<Wry>) -> Result<Option<String>, String> {
    load_selected_domain(&app)
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
