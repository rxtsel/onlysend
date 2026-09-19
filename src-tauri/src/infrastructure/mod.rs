//! JSON store primitives shared by the infrastructure stores.
//!
//! Two files back the app state:
//! - `settings.json` — user preferences and local data (via settings_store)
//! - `auth.json` — credentials, DO NOT SHARE (via credentials_store)

pub mod credentials_store;
pub mod http;
pub mod settings_store;

use serde::Deserialize;
use serde_json::Value;
use tauri::{AppHandle, Wry};
use tauri_plugin_store::StoreExt;

pub(crate) const STORE_FILE: &str = "settings.json";
/// Dedicated credentials file. DO NOT SHARE.
pub(crate) const AUTH_FILE: &str = "auth.json";

pub(crate) fn write_key(
    app: &AppHandle<Wry>,
    file: &str,
    key: &str,
    value: Value,
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

pub(crate) fn delete_key(app: &AppHandle<Wry>, file: &str, key: &str) -> Result<(), String> {
    let store = app
        .store(file)
        .map_err(|e| format!("[ERROR] Failed to load {file}: {e}"))?;

    store.delete(key);

    store
        .save()
        .map_err(|e| format!("[ERROR] Failed to save {file}: {e}"))?;

    Ok(())
}

pub(crate) fn read_raw_key(
    app: &AppHandle<Wry>,
    file: &str,
    key: &str,
) -> Result<Option<Value>, String> {
    let store = app
        .store(file)
        .map_err(|e| format!("[ERROR] Failed to load {file}: {e}"))?;

    Ok(store.get(key))
}

pub(crate) fn read_string_key(
    app: &AppHandle<Wry>,
    file: &str,
    key: &str,
) -> Result<Option<String>, String> {
    Ok(read_raw_key(app, file, key)?.and_then(|v| v.as_str().map(String::from)))
}

pub(crate) fn read_typed<T: for<'de> Deserialize<'de>>(
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
