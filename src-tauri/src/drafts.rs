//! Private, account-scoped drafts; incomplete addresses are valid until Send.
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Wry};

use crate::infrastructure::database::{local_state, Database};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftData {
    from: String,
    to: Vec<String>,
    cc: Vec<String>,
    bcc: Vec<String>,
    pending_to: String,
    pending_cc: String,
    pending_bcc: String,
    subject: String,
    content: String,
    reply_email_id: Option<String>,
    reply_to: String,
    is_reply_to_enabled: bool,
    is_cc_enabled: bool,
    is_bcc_enabled: bool,
    #[serde(default)]
    is_schedule_enabled: bool,
    #[serde(default)]
    scheduled_local: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    attachments: Option<Vec<DraftAttachment>>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DraftAttachment {
    pub(crate) filename: String,
    pub(crate) content: String,
    pub(crate) mime_type: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftSummary {
    id: String,
    subject: String,
    from: String,
    to: Vec<String>,
    preview: String,
    updated_at: i64,
}

fn validate_id(id: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| "Invalid draft ID".into())
}

pub(crate) fn encode(mut data: DraftData) -> Result<(String, Option<String>), String> {
    if let Some(id) = &data.reply_email_id {
        uuid::Uuid::parse_str(id).map_err(|_| "Invalid reply ID")?;
    }
    let attachments = data.attachments.take();
    let mut total = 0;
    for attachment in attachments.iter().flatten() {
        crate::email::attachment_bytes(
            &crate::email::AttachmentPayload {
                filename: attachment.filename.clone(),
                content: attachment.content.clone(),
            },
            &mut total,
        )?;
    }
    let json = serde_json::to_string(&data).map_err(|_| "Could not encode draft")?;
    let attachments = attachments
        .map(|items| serde_json::to_string(&items))
        .transpose()
        .map_err(|_| "Could not encode draft attachments")?;
    if json.len() + attachments.as_ref().map_or(0, String::len) > 36 * 1024 * 1024 {
        return Err("Draft is too large (maximum 36 MB)".into());
    }
    Ok((json, attachments))
}

async fn save(
    database: &Database,
    account: String,
    id: String,
    json: String,
    attachments: Option<String>,
) -> Result<(), String> {
    database.run(move |connection| {
        let transaction = connection.transaction()?;
        let attachment_bytes = match &attachments {
            Some(json) => json.len() as i64,
            None => transaction.query_row("SELECT length(CAST(data_json AS BLOB)) FROM draft_attachments WHERE account_id=?1 AND draft_id=?2", params![account, id], |row| row.get::<_, i64>(0)).optional()?.unwrap_or(0),
        };
        if json.len() as i64 + attachment_bytes > 36 * 1024 * 1024 {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput, "Draft is too large (maximum 36 MB)",
            ))));
        }
        transaction.execute(
            "INSERT INTO drafts(account_id,draft_id,data_json,updated_at) VALUES (?1,?2,?3,CAST(unixepoch('subsec')*1000 AS INTEGER)) ON CONFLICT(account_id,draft_id) DO UPDATE SET data_json=excluded.data_json,updated_at=excluded.updated_at",
            params![account, id, json],
        )?;
        if let Some(attachments) = attachments {
            transaction.execute("INSERT INTO draft_attachments VALUES (?1,?2,?3) ON CONFLICT(account_id,draft_id) DO UPDATE SET data_json=excluded.data_json", params![account, id, attachments])?;
        }
        transaction.commit()
    }).await
}

async fn get(database: &Database, account: String, id: String) -> Result<Option<String>, String> {
    database
        .run(move |connection| {
            connection
                .query_row(
                    "SELECT json_set(d.data_json,'$.attachments',json(COALESCE(a.data_json,'[]'))) FROM drafts d LEFT JOIN draft_attachments a USING(account_id,draft_id) WHERE d.account_id=?1 AND d.draft_id=?2",
                    params![account, id],
                    |row| row.get(0),
                )
                .optional()
        })
        .await
}

async fn delete(database: &Database, account: String, id: String) -> Result<(), String> {
    database
        .run(move |connection| {
            connection.execute(
                "DELETE FROM drafts WHERE account_id=?1 AND draft_id=?2",
                params![account, id],
            )?;
            Ok(())
        })
        .await
}

/// Publishing the recovered draft and hiding the canceled Sent copy is atomic.
pub(crate) async fn restore_canceled(
    database: &Database,
    account: String,
    email_id: String,
    encoded: (String, Option<String>),
) -> Result<(), String> {
    let (json, attachments) = encoded;
    database.run(move |connection| {
        let tx = connection.transaction()?;
        tx.execute("INSERT OR IGNORE INTO account_state VALUES (?1)", [&account])?;
        // The remote ID makes retries idempotent without overwriting later user edits.
        let inserted = tx.execute("INSERT OR IGNORE INTO drafts(account_id,draft_id,data_json,updated_at) VALUES (?1,?2,?3,CAST(unixepoch('subsec')*1000 AS INTEGER))", params![account,email_id,json])?;
        if inserted > 0 {
            tx.execute("INSERT INTO draft_attachments VALUES (?1,?2,?3)", params![account,email_id,attachments.unwrap_or_else(|| "[]".into())])?;
        }
        tx.execute("INSERT OR IGNORE INTO deleted_emails VALUES (?1,'sent',?2)", params![account,email_id])?;
        tx.commit()
    }).await
}

#[tauri::command]
pub async fn save_draft(
    app: AppHandle<Wry>,
    account_id: String,
    draft_id: String,
    data: DraftData,
) -> Result<(), String> {
    validate_id(&draft_id)?;
    let (json, attachments) = encode(data)?;
    let database = local_state::for_account(&app, &account_id).await?;
    save(&database, account_id, draft_id, json, attachments).await
}

#[tauri::command]
pub async fn get_draft(
    app: AppHandle<Wry>,
    account_id: String,
    draft_id: String,
) -> Result<Option<DraftData>, String> {
    validate_id(&draft_id)?;
    let database = local_state::for_account(&app, &account_id).await?;
    get(&database, account_id, draft_id)
        .await?
        .map(|json| serde_json::from_str(&json).map_err(|_| "Could not read draft".into()))
        .transpose()
}

#[tauri::command]
pub async fn delete_draft(
    app: AppHandle<Wry>,
    account_id: String,
    draft_id: String,
) -> Result<(), String> {
    validate_id(&draft_id)?;
    let database = local_state::for_account(&app, &account_id).await?;
    delete(&database, account_id, draft_id).await
}

#[tauri::command]
pub async fn list_drafts(
    app: AppHandle<Wry>,
    account_id: String,
) -> Result<Vec<DraftSummary>, String> {
    let database = local_state::for_account(&app, &account_id).await?;
    database.run(move |connection| {
        let mut query = connection.prepare("SELECT draft_id,json_extract(data_json,'$.subject'),json_extract(data_json,'$.to'),updated_at,json_extract(data_json,'$.from'),json_extract(data_json,'$.content') FROM drafts WHERE account_id=?1 ORDER BY updated_at DESC,draft_id")?;
        let rows = query.query_map([account_id], |row| {
            let to: String = row.get(2)?;
            let content: String = row.get(5)?;
            let preview = crate::infrastructure::database::mail::plain_html(&content)
                .split_whitespace().collect::<Vec<_>>().join(" ").chars().take(240).collect();
            Ok(DraftSummary { id: row.get(0)?, subject: row.get(1)?, from: row.get(4)?, preview, to: serde_json::from_str(&to).map_err(|error| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(error)))?, updated_at: row.get(3)? })
        })?;
        rows.collect()
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_restoration_is_atomic_scoped_and_does_not_overwrite_edited_drafts() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::new(directory.path().join("restore.sqlite3"));
        let encoded = || (r#"{"content":"original"}"#.to_string(), Some(r#"[{"filename":"note.txt","content":"aGk=","mimeType":"text/plain"}]"#.to_string()));
        database.run(|connection| connection.execute_batch("INSERT INTO account_state VALUES ('a'),('b'); CREATE TRIGGER reject_restore BEFORE INSERT ON deleted_emails BEGIN SELECT RAISE(ABORT,'disk full'); END;")).await.unwrap();
        assert!(restore_canceled(&database, "a".into(), "email".into(), encoded()).await.is_err());
        assert!(get(&database, "a".into(), "email".into()).await.unwrap().is_none());
        database.run(|connection| connection.execute_batch("DROP TRIGGER reject_restore")).await.unwrap();
        restore_canceled(&database, "a".into(), "email".into(), encoded()).await.unwrap();
        let value: serde_json::Value = serde_json::from_str(&get(&database, "a".into(), "email".into()).await.unwrap().unwrap()).unwrap();
        assert_eq!(value["content"], "original");
        assert_eq!(value["attachments"][0]["filename"], "note.txt");
        assert!(get(&database, "b".into(), "email".into()).await.unwrap().is_none());
        assert!(crate::infrastructure::database::mail::ensure_visible(&database, "a".into(), crate::infrastructure::database::mail::Mailbox::Sent, "email".into()).await.is_err());
        assert!(crate::infrastructure::database::mail::ensure_visible(&database, "b".into(), crate::infrastructure::database::mail::Mailbox::Sent, "email".into()).await.is_ok());
        save(&database, "a".into(), "email".into(), r#"{"content":"user edit"}"#.into(), None).await.unwrap();
        restore_canceled(&database, "a".into(), "email".into(), encoded()).await.unwrap();
        let value: serde_json::Value = serde_json::from_str(&get(&database, "a".into(), "email".into()).await.unwrap().unwrap()).unwrap();
        assert_eq!(value["content"], "user edit");
        assert_eq!(value["attachments"][0]["content"], "aGk=");
    }

    async fn content(database: &Database, account: &str) -> Option<String> {
        get(database, account.into(), "same".into())
            .await
            .unwrap()
            .map(|json| {
                serde_json::from_str::<serde_json::Value>(&json).unwrap()["content"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
    }

    #[tokio::test]
    async fn drafts_survive_reopen_are_account_scoped_and_cascade_on_removal() {
        let path = std::env::temp_dir().join(format!("drafts-{}.sqlite3", uuid::Uuid::new_v4()));
        let database = Database::new(path.clone());
        database
            .run(|connection| {
                connection.execute_batch("INSERT INTO account_state VALUES ('a'),('b')")
            })
            .await
            .unwrap();
        save(
            &database,
            "a".into(),
            "same".into(),
            r#"{"content":"first"}"#.into(),
            Some("[]".into()),
        )
        .await
        .unwrap();
        save(
            &database,
            "b".into(),
            "same".into(),
            r#"{"content":"other"}"#.into(),
            Some("[]".into()),
        )
        .await
        .unwrap();
        save(
            &database,
            "a".into(),
            "same".into(),
            r#"{"content":"latest"}"#.into(),
            None,
        )
        .await
        .unwrap();
        drop(database);
        let database = Database::new(path.clone());
        assert_eq!(content(&database, "a").await.as_deref(), Some("latest"));
        delete(&database, "a".into(), "same".into()).await.unwrap();
        assert!(get(&database, "a".into(), "same".into())
            .await
            .unwrap()
            .is_none());
        assert_eq!(content(&database, "b").await.as_deref(), Some("other"));
        database
            .run(|connection| {
                connection.execute("DELETE FROM account_state WHERE account_id='b'", [])?;
                Ok(())
            })
            .await
            .unwrap();
        assert!(get(&database, "b".into(), "same".into())
            .await
            .unwrap()
            .is_none());
        drop(database);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn text_updates_leave_attachments_untouched_and_failed_file_changes_roll_back() {
        let path =
            std::env::temp_dir().join(format!("draft-files-{}.sqlite3", uuid::Uuid::new_v4()));
        let database = Database::new(path.clone());
        database
            .run(|connection| connection.execute_batch("INSERT INTO account_state VALUES ('a')"))
            .await
            .unwrap();
        let files = r#"[{"filename":"note.txt","content":"aGVsbG8=","mimeType":"text/plain"}]"#;
        save(
            &database,
            "a".into(),
            "same".into(),
            r#"{"content":"first"}"#.into(),
            Some(files.into()),
        )
        .await
        .unwrap();
        database.run(|connection| connection.execute_batch("CREATE TRIGGER reject_file_update BEFORE UPDATE ON draft_attachments BEGIN SELECT RAISE(ABORT,'file write failed'); END;")).await.unwrap();
        save(
            &database,
            "a".into(),
            "same".into(),
            r#"{"content":"text only"}"#.into(),
            None,
        )
        .await
        .unwrap();
        let data: serde_json::Value = serde_json::from_str(
            &get(&database, "a".into(), "same".into())
                .await
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(data["attachments"][0]["content"], "aGVsbG8=");
        assert!(save(
            &database,
            "a".into(),
            "same".into(),
            r#"{"content":"must roll back"}"#.into(),
            Some("[]".into())
        )
        .await
        .is_err());
        assert_eq!(content(&database, "a").await.as_deref(), Some("text only"));
        delete(&database, "a".into(), "same".into()).await.unwrap();
        assert_eq!(
            database
                .run(|connection| connection.query_row(
                    "SELECT count(*) FROM draft_attachments",
                    [],
                    |row| row.get::<_, i64>(0)
                ))
                .await
                .unwrap(),
            0
        );
        drop(database);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn validates_draft_ids_and_attachment_data_but_allows_incomplete_messages() {
        let mut data: DraftData = serde_json::from_value(serde_json::json!({
            "from":"", "to":[], "cc":[], "bcc":[], "pendingTo":"unfinished@", "pendingCc":"", "pendingBcc":"",
            "subject":"", "content":"", "replyEmailId":null, "replyTo":"", "isReplyToEnabled":false,
            "isCcEnabled":false, "isBccEnabled":false, "attachments":[]
        })).unwrap();
        let (json, attachments) = encode(data).unwrap();
        assert_eq!(attachments.as_deref(), Some("[]"));
        assert!(!json.contains("attachments"));
        assert!(validate_id("../bad").is_err());
        data = serde_json::from_str(&json).unwrap();
        data.attachments = Some(vec![DraftAttachment {
            filename: "file.txt".into(),
            content: "not base64".into(),
            mime_type: "text/plain".into(),
        }]);
        assert!(encode(data).is_err());
    }
}
