use tauri::{AppHandle, Wry};
use uuid::Uuid;

use crate::infrastructure::settings_store::{load_from_emails, save_from_emails, FromEmail};

#[tauri::command]
pub fn list_from_emails(app: AppHandle<Wry>, account_id: String) -> Result<Vec<FromEmail>, String> {
    load_from_emails(&app, &account_id)
}

#[tauri::command]
pub fn create_from_email(
    app: AppHandle<Wry>, account_id: String,
    label: String,
    address: String,
    is_default: bool,
) -> Result<FromEmail, String> {
    let mut emails = load_from_emails(&app, &account_id)?;

    if is_default {
        for e in &mut emails {
            e.is_default = false;
        }
    }

    let email = FromEmail {
        id: Uuid::new_v4().to_string(),
        label,
        address,
        is_default,
    };

    emails.push(email.clone());
    save_from_emails(&app, &account_id, &emails)?;

    Ok(email)
}

#[tauri::command]
pub fn update_from_email(
    app: AppHandle<Wry>, account_id: String,
    id: String,
    label: Option<String>,
    address: Option<String>,
    is_default: Option<bool>,
) -> Result<FromEmail, String> {
    let mut emails = load_from_emails(&app, &account_id)?;
    let mut updated: Option<FromEmail> = None;

    if let Some(true) = is_default {
        for e in &mut emails {
            e.is_default = false;
        }
    }

    for e in &mut emails {
        if e.id == id {
            if let Some(l) = label.clone() {
                e.label = l;
            }
            if let Some(a) = address.clone() {
                e.address = a;
            }
            if let Some(d) = is_default {
                e.is_default = d;
            }
            updated = Some(e.clone());
            break;
        }
    }

    let updated = updated.ok_or_else(|| "From email not found".to_string())?;
    save_from_emails(&app, &account_id, &emails)?;
    Ok(updated)
}

#[tauri::command]
pub fn delete_from_email(app: AppHandle<Wry>, account_id: String, id: String) -> Result<(), String> {
    let mut emails = load_from_emails(&app, &account_id)?;
    let len_before = emails.len();
    emails.retain(|e| e.id != id);

    if emails.len() == len_before {
        return Err("From email not found".to_string());
    }

    save_from_emails(&app, &account_id, &emails)?;
    Ok(())
}
