use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Wry};

use crate::infrastructure::database::{local_state, Database};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailTemplate {
    pub id: String,
    pub name: String,
    pub subject: String,
    pub html: String,
}

fn validate(name: &str, subject: &str, html: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
        return Err("Template name must contain 1–120 characters".into());
    }
    if subject.len() > 998 || subject.chars().any(char::is_control) {
        return Err("Invalid template subject".into());
    }
    if html.trim().is_empty() || html.len() > 1024 * 1024 {
        return Err("Template HTML must contain 1 byte to 1 MB".into());
    }
    Ok(())
}

async fn save(
    database: &Database,
    account_id: String,
    id: Option<String>,
    name: String,
    subject: String,
    html: String,
) -> Result<EmailTemplate, String> {
    validate(&name, &subject, &html)?;
    let updating = id.is_some();
    let template = EmailTemplate {
        id: id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        name: name.trim().into(),
        subject,
        html,
    };
    database.run(move |connection| {
        if updating {
            let changed = connection.execute(
                "UPDATE email_templates SET name = ?3, subject = ?4, html = ?5, updated_at = unixepoch() WHERE account_id = ?1 AND id = ?2",
                params![account_id, template.id, template.name, template.subject, template.html],
            )?;
            if changed == 0 { return Err(rusqlite::Error::QueryReturnedNoRows); }
        } else {
            connection.execute(
                "INSERT INTO email_templates (account_id, id, name, subject, html) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![account_id, template.id, template.name, template.subject, template.html],
            )?;
        }
        Ok(template)
    }).await
}

async fn list(database: &Database, account_id: String) -> Result<Vec<EmailTemplate>, String> {
    database.run(move |connection| {
        let mut statement = connection.prepare("SELECT id, name, subject, html FROM email_templates WHERE account_id = ?1 ORDER BY updated_at DESC, name, id")?;
        let rows = statement.query_map([account_id], |row| Ok(EmailTemplate { id: row.get(0)?, name: row.get(1)?, subject: row.get(2)?, html: row.get(3)? }))?.collect();
        rows
    }).await
}

async fn delete(database: &Database, account_id: String, id: String) -> Result<(), String> {
    database
        .run(move |connection| {
            connection.execute(
                "DELETE FROM email_templates WHERE account_id = ?1 AND id = ?2",
                params![account_id, id],
            )?;
            Ok(())
        })
        .await
}

#[tauri::command]
pub async fn list_email_templates(
    app: AppHandle<Wry>,
    account_id: String,
) -> Result<Vec<EmailTemplate>, String> {
    let database = local_state::for_account(&app, &account_id).await?;
    list(&database, account_id).await
}

#[tauri::command]
pub async fn save_email_template(
    app: AppHandle<Wry>,
    account_id: String,
    id: Option<String>,
    name: String,
    subject: String,
    html: String,
) -> Result<EmailTemplate, String> {
    let database = local_state::for_account(&app, &account_id).await?;
    save(&database, account_id, id, name, subject, html).await
}

#[tauri::command]
pub async fn delete_email_template(
    app: AppHandle<Wry>,
    account_id: String,
    id: String,
) -> Result<(), String> {
    let database = local_state::for_account(&app, &account_id).await?;
    delete(&database, account_id, id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn templates_persist_and_updates_and_deletes_are_account_scoped() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("templates.sqlite3");
        let database = Database::new(path.clone());
        let first = save(
            &database,
            "a".into(),
            None,
            "Welcome".into(),
            "Hello".into(),
            "<p>Hi</p>".into(),
        )
        .await
        .unwrap();
        drop(database);
        let database = Database::new(path);
        assert_eq!(
            list(&database, "a".into()).await.unwrap()[0].html,
            "<p>Hi</p>"
        );
        assert!(list(&database, "b".into()).await.unwrap().is_empty());
        assert!(save(
            &database,
            "b".into(),
            Some(first.id.clone()),
            "Bad".into(),
            "".into(),
            "<p>Bad</p>".into()
        )
        .await
        .is_err());
        delete(&database, "b".into(), first.id.clone())
            .await
            .unwrap();
        assert_eq!(list(&database, "a".into()).await.unwrap().len(), 1);
        save(
            &database,
            "a".into(),
            Some(first.id.clone()),
            "Updated".into(),
            "New subject".into(),
            "<p>New</p>".into(),
        )
        .await
        .unwrap();
        assert_eq!(
            list(&database, "a".into()).await.unwrap()[0].name,
            "Updated"
        );
        delete(&database, "a".into(), first.id).await.unwrap();
        assert!(list(&database, "a".into()).await.unwrap().is_empty());
    }

    #[test]
    fn validates_saved_content() {
        for (name, subject, html) in [
            (" ", "", "<p>x</p>"),
            ("Name", "bad\nsubject", "x"),
            ("Name", "", " "),
        ] {
            assert!(validate(name, subject, html).is_err());
        }
        assert!(validate("Name", "", &"x".repeat(1024 * 1024 + 1)).is_err());
        assert!(validate("Name", "", "<html><body>Hello</body></html>").is_ok());
    }
}
