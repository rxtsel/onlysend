//! Account-scoped settings. Domain preferences and read markers use SQLite;
//! onboarding, identities and remaining legacy settings still use JSON.
//! Global legacy values are preserved, never assigned to an arbitrary account.

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Wry};

use super::{read_raw_key, read_string_key, read_typed, write_key, STORE_FILE};

const SETUP_COMPLETE_KEY: &str = "setup_complete";
const INBOX_ENABLED_KEY: &str = "inbox_enabled";
const FROM_EMAILS_KEY: &str = "from_emails";
const INBOUND_SETUP_CACHE_KEY: &str = "inbound_setup";
use super::database::local_state;

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
    /// Legacy delivery flag; does not gate access to account history.
    #[serde(default)]
    pub inbox_enabled: bool,
}

#[tauri::command]
pub fn get_onboarding_state(
    app: AppHandle<Wry>,
    account_id: String,
) -> Result<OnboardingState, String> {
    let complete = read_raw_key(
        &app,
        STORE_FILE,
        &scoped_key(&app, &account_id, SETUP_COMPLETE_KEY)?,
    )?
    .and_then(|v| v.as_bool())
    .unwrap_or(false);

    let authenticated =
        crate::infrastructure::credentials_store::account_credential(&app, &account_id).is_ok();

    let inbox_enabled = read_raw_key(
        &app,
        STORE_FILE,
        &scoped_key(&app, &account_id, INBOX_ENABLED_KEY)?,
    )?
    .and_then(|v| v.as_bool())
    .unwrap_or(false);

    Ok(OnboardingState {
        // Existing account-scoped identities indicate a completed legacy setup.
        complete: complete || !load_from_emails(&app, &account_id)?.is_empty(),
        authenticated,
        inbox_enabled,
    })
}

/// Legacy delivery observation, independent of access to inbox history.
#[tauri::command]
pub fn set_inbox_enabled(
    app: AppHandle<Wry>,
    account_id: String,
    enabled: bool,
) -> Result<(), String> {
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
    app: AppHandle<Wry>,
    account_id: String,
) -> Result<Option<serde_json::Value>, String> {
    let key = scoped_key(&app, &account_id, INBOUND_SETUP_CACHE_KEY)?;
    read_raw_key(&app, STORE_FILE, &key)
}

#[tauri::command]
pub fn save_inbound_setup_cache(
    app: AppHandle<Wry>,
    account_id: String,
    detail: serde_json::Value,
) -> Result<(), String> {
    let key = scoped_key(&app, &account_id, INBOUND_SETUP_CACHE_KEY)?;
    write_key(&app, STORE_FILE, &key, detail)
}

/* ---------------------------------------------------------
 * Selected domain
 * --------------------------------------------------------- */
#[tauri::command]
pub fn save_selected_domain(
    app: AppHandle<Wry>,
    account_id: String,
    domain: String,
) -> Result<(), String> {
    write_scoped(&app, &account_id, "selected_domain", json!(domain))
}

pub(crate) fn load_selected_domain(
    app: &AppHandle<Wry>,
    account_id: &str,
) -> Result<Option<String>, String> {
    read_string_key(
        app,
        STORE_FILE,
        &scoped_key(app, account_id, "selected_domain")?,
    )
}

#[tauri::command]
pub fn get_active_domain(
    app: AppHandle<Wry>,
    account_id: String,
) -> Result<Option<String>, String> {
    load_selected_domain(&app, &account_id)
}

#[tauri::command]
pub fn get_selected_domain(
    app: AppHandle<Wry>,
    account_id: String,
) -> Result<Option<String>, String> {
    load_selected_domain(&app, &account_id)
}

/* ---------------------------------------------------------
 * Domain inclusion (local setup preference, never a remote mail filter)
 * --------------------------------------------------------- */
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainPreferences {
    pub included_domain_ids: Vec<String>,
}

#[tauri::command]
pub async fn get_domain_preferences(
    app: AppHandle<Wry>,
    account_id: String,
) -> Result<Option<DomainPreferences>, String> {
    let database = local_state::for_account(&app, &account_id).await?;
    local_state::preferences(&database, account_id).await
}

#[tauri::command]
pub async fn save_domain_preferences(
    app: AppHandle<Wry>,
    account_id: String,
    preferences: DomainPreferences,
) -> Result<(), String> {
    let database = local_state::for_account(&app, &account_id).await?;
    local_state::save_preferences(&database, account_id, preferences).await
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

pub(crate) fn load_from_emails(
    app: &AppHandle<Wry>,
    account_id: &str,
) -> Result<Vec<FromEmail>, String> {
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
pub async fn get_read_inbound_ids(
    app: AppHandle<Wry>,
    account_id: String,
) -> Result<Vec<String>, String> {
    let database = local_state::for_account(&app, &account_id).await?;
    database
        .run(move |connection| local_state::read_ids(connection, &account_id))
        .await
}

#[tauri::command]
pub async fn mark_inbound_read(
    app: AppHandle<Wry>,
    account_id: String,
    email_id: String,
) -> Result<(), String> {
    let database = local_state::for_account(&app, &account_id).await?;
    database.run(move |connection| {
        connection.execute("INSERT INTO read_markers VALUES (?1, 'inbox', ?2) ON CONFLICT(account_id,mailbox,email_id) DO NOTHING", rusqlite::params![account_id, email_id])?;
        Ok(())
    }).await
}
