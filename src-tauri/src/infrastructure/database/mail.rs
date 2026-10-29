//! Persistent downloaded-mail archive. Remote pages and local keysets are
//! intentionally different contracts; remote deletions never prune this copy.
use rusqlite::{params, params_from_iter, types::Value as SqlValue, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Database;
use crate::infrastructure::pagination::{EmailPage, PageCache};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mailbox {
    Inbox,
    Sent,
}
impl Mailbox {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Sent => "sent",
        }
    }
}

fn json_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

pub(crate) fn timestamp(value: &str) -> Result<i64, String> {
    let value = value.trim();
    // Resend documents PostgreSQL timestamps such as
    // "2026-04-03 22:13:42.674981+00", not just RFC3339. Expand
    // hour-only offsets without assuming UTC or discarding the offset.
    let suffix = value.as_bytes().get(value.len().saturating_sub(3)..);
    let normalized = match suffix {
        Some([sign, hour1, hour2])
            if matches!(sign, b'+' | b'-') && hour1.is_ascii_digit() && hour2.is_ascii_digit() =>
        {
            format!("{value}:00")
        }
        _ => value.to_string(),
    };
    if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&normalized) {
        return Ok(date.timestamp_millis());
    }
    for format in [
        "%Y-%m-%d %H:%M:%S%.f %:z",
        "%Y-%m-%d %H:%M:%S%.f%:z",
        "%Y-%m-%d %H:%M:%S%.f %z",
        "%Y-%m-%d %H:%M:%S%.f%z",
    ] {
        if let Ok(date) = chrono::DateTime::parse_from_str(&normalized, format) {
            return Ok(date.timestamp_millis());
        }
    }
    Err("Invalid email creation timestamp".to_string())
}

fn strings(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) async fn ensure_visible(
    database: &Database,
    account_id: String,
    mailbox: Mailbox,
    email_id: String,
) -> Result<(), String> {
    let deleted: bool = database.run(move |connection| connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM deleted_emails WHERE account_id=?1 AND mailbox=?2 AND email_id=?3)",
        params![account_id, mailbox.as_str(), email_id], |row| row.get(0),
    )).await?;
    if deleted { return Err("This email was deleted from this device".into()); }
    Ok(())
}

pub(crate) async fn set_deleted(
    database: &Database,
    account_id: String,
    mailbox: Mailbox,
    email_id: String,
    deleted: bool,
) -> Result<(), String> {
    database.run(move |connection| {
        let tx = connection.transaction()?;
        tx.execute("INSERT OR IGNORE INTO account_state VALUES (?1)", [&account_id])?;
        if deleted {
            tx.execute("INSERT OR IGNORE INTO deleted_emails VALUES (?1,?2,?3)", params![account_id, mailbox.as_str(), email_id])?;
        } else {
            tx.execute("DELETE FROM deleted_emails WHERE account_id=?1 AND mailbox=?2 AND email_id=?3", params![account_id, mailbox.as_str(), email_id])?;
        }
        tx.commit()
    }).await
}

pub(crate) async fn cached_detail(
    database: &Database,
    account_id: String,
    mailbox: String,
    email_id: String,
) -> Result<Option<String>, String> {
    database.run(move |connection| connection.query_row(
        "SELECT detail_json FROM email_bodies WHERE account_id = ?1 AND mailbox = ?2 AND email_id = ?3",
        params![account_id, mailbox, email_id], |row| row.get(0),
    ).optional()).await
}

pub(crate) async fn store_thread_link(
    database: &Database,
    account_id: String,
    mailbox: Mailbox,
    email_id: String,
    thread_id: String,
) -> Result<(), String> {
    database.run(move |connection| {
        connection.execute("INSERT OR IGNORE INTO account_state VALUES (?1)", [&account_id])?;
        connection.execute(
            "INSERT INTO email_threads VALUES (?1,?2,?3,?4) ON CONFLICT(account_id,mailbox,email_id) DO UPDATE SET thread_id=excluded.thread_id",
            params![account_id, mailbox.as_str(), email_id, thread_id],
        )?;
        Ok(())
    }).await
}

pub(crate) async fn cached_thread(
    database: &Database,
    account_id: String,
    thread_id: String,
    preferred_mailbox: Mailbox,
) -> Result<Vec<(String, String)>, String> {
    let rows = database.run(move |connection| {
        let mut statement = connection.prepare(
            "SELECT e.mailbox,e.created_at_ms,b.detail_json FROM email_threads t JOIN emails e USING(account_id,mailbox,email_id) JOIN email_bodies b USING(account_id,mailbox,email_id) WHERE t.account_id=?1 AND t.thread_id=?2 AND NOT EXISTS(SELECT 1 FROM deleted_emails d WHERE d.account_id=e.account_id AND d.mailbox=e.mailbox AND d.email_id=e.email_id) ORDER BY e.created_at_ms,e.email_id",
        )?;
        let rows = statement.query_map(params![account_id, thread_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?))
        })?.collect::<rusqlite::Result<Vec<(String, i64, String)>>>();
        rows
    }).await?;
    let mut unique = std::collections::HashMap::<String, (bool, i64, String, String)>::new();
    for (mailbox, date, json) in rows {
        let detail: Value =
            serde_json::from_str(&json).map_err(|_| "Could not read downloaded thread")?;
        let key = detail["messageId"]
            .as_str()
            .or_else(|| detail["id"].as_str())
            .unwrap_or(&json)
            .to_owned();
        let preferred = mailbox == preferred_mailbox.as_str();
        match unique.get_mut(&key) {
            Some((is_preferred, old_date, old_mailbox, old_json))
                if preferred && !*is_preferred =>
            {
                *is_preferred = true;
                *old_date = date;
                *old_mailbox = mailbox;
                *old_json = json;
            }
            None => {
                unique.insert(key, (preferred, date, mailbox, json));
            }
            _ => {}
        }
    }
    let mut messages: Vec<_> = unique
        .into_values()
        .map(|(_, date, mailbox, json)| (date, mailbox, json))
        .collect();
    messages.sort_by_key(|(date, _, _)| *date);
    Ok(messages
        .into_iter()
        .map(|(_, mailbox, json)| (mailbox, json))
        .collect())
}

pub(crate) async fn cached_thread_candidates(
    database: &Database,
    account_id: String,
    message_id: String,
) -> Result<Vec<(String, String, String)>, String> {
    database.run(move |connection| {
        let mut statement = connection.prepare(
            "SELECT mailbox,email_id,detail_json FROM email_bodies WHERE account_id=?1 AND instr(detail_json,?2)>0",
        )?;
        let rows = statement.query_map(params![account_id, message_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?.collect();
        rows
    }).await
}

pub(crate) async fn thread_id_for(
    database: &Database,
    account_id: String,
    mailbox: Mailbox,
    email_id: String,
) -> Result<Option<String>, String> {
    database
        .run(move |connection| {
            connection.query_row(
        "SELECT thread_id FROM email_threads WHERE account_id=?1 AND mailbox=?2 AND email_id=?3",
        params![account_id, mailbox.as_str(), email_id], |row| row.get(0),
    ).optional()
        })
        .await
}

pub(crate) async fn store_attachments<T: Serialize>(
    database: &Database,
    account_id: String,
    mailbox: Mailbox,
    email_id: String,
    attachments: &[T],
) -> Result<(), String> {
    let json =
        serde_json::to_string(attachments).map_err(|_| "Could not store attachment metadata")?;
    database.run(move |connection| {
        connection.execute("UPDATE email_bodies SET detail_json=json_set(detail_json,'$.attachments',json(?4)) WHERE account_id=?1 AND mailbox=?2 AND email_id=?3", params![account_id,mailbox.as_str(),email_id,json])?;
        Ok(())
    }).await
}

pub(crate) fn plain_html(html: &str) -> String {
    ammonia::Builder::default()
        .tags(std::collections::HashSet::new())
        .clean(html)
        .to_string()
}

fn address(value: &str) -> String {
    value
        .rsplit_once('<')
        .map(|(_, address)| address)
        .unwrap_or(value)
        .trim()
        .trim_end_matches('>')
        .trim()
        .to_lowercase()
}

pub(crate) async fn store_page<T: Serialize>(
    database: &Database,
    account_id: String,
    mailbox: Mailbox,
    limit: usize,
    after: Option<String>,
    page: &EmailPage<T>,
) -> Result<(), String> {
    write_page(database, account_id, mailbox, limit, after, page, true).await
}

pub(crate) async fn store_scan_page<T: Serialize>(
    database: &Database,
    account_id: String,
    mailbox: Mailbox,
    limit: usize,
    after: Option<String>,
    page: &EmailPage<T>,
) -> Result<(), String> {
    // Full scans update canonical rows, not duplicate snapshots of the entire
    // archive at every shifted head. The interactive head stays available.
    let snapshot = after.is_none();
    write_page(database, account_id, mailbox, limit, after, page, snapshot).await
}

async fn write_page<T: Serialize>(
    database: &Database,
    account_id: String,
    mailbox: Mailbox,
    limit: usize,
    after: Option<String>,
    page: &EmailPage<T>,
    snapshot: bool,
) -> Result<(), String> {
    let data =
        serde_json::to_value(page).map_err(|_| "Could not serialize email page".to_string())?;
    let items = data["items"]
        .as_array()
        .ok_or("Invalid email page")?
        .clone();
    let dates = items
        .iter()
        .map(|item| {
            timestamp(
                item["createdAt"]
                    .as_str()
                    .ok_or("Missing email timestamp")?,
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    let page_json =
        serde_json::to_string(&data).map_err(|_| "Could not serialize email page".to_string())?;
    database.run(move |connection| {
        let tx = connection.transaction()?;
        tx.execute("INSERT OR IGNORE INTO account_state VALUES (?1)", [&account_id])?;
        for (item, date) in items.iter().zip(dates) {
            let id = item["id"].as_str().filter(|id| !id.is_empty()).ok_or(rusqlite::Error::InvalidQuery)?;
            let sender = item["from"].as_str().ok_or(rusqlite::Error::InvalidQuery)?;
            let subject = item["subject"].as_str().ok_or(rusqlite::Error::InvalidQuery)?;
            let recipients: Vec<_> = ["to", "cc", "bcc"].into_iter().flat_map(|key| strings(item, key)).collect();
            let metadata = serde_json::to_string(item).map_err(json_error)?;
            if let Some(thread_id) = item["threadId"].as_str() {
                tx.execute("INSERT OR IGNORE INTO email_threads VALUES (?1,?2,?3,?4)", params![account_id, mailbox.as_str(), id, thread_id])?;
            }
            let changed = tx.execute("INSERT INTO emails(account_id,mailbox,email_id,created_at_ms,subject,sender,subject_sort,sender_sort,recipient_text,search_body,metadata_json,downloaded_at)
                VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,COALESCE((SELECT search_text FROM email_bodies WHERE account_id=?1 AND mailbox=?2 AND email_id=?3),''),?10,unixepoch())
                ON CONFLICT(account_id,mailbox,email_id) DO UPDATE SET created_at_ms=excluded.created_at_ms,subject=excluded.subject,sender=excluded.sender,subject_sort=excluded.subject_sort,sender_sort=excluded.sender_sort,recipient_text=excluded.recipient_text,metadata_json=excluded.metadata_json,downloaded_at=excluded.downloaded_at WHERE emails.metadata_json<>excluded.metadata_json",
                params![account_id, mailbox.as_str(), id, date, subject, sender, subject.to_lowercase(), sender.to_lowercase(), recipients.join(" "), metadata])?;
            if changed == 0 { continue; }
            tx.execute("DELETE FROM email_domains WHERE account_id=?1 AND mailbox=?2 AND email_id=?3", params![account_id, mailbox.as_str(), id])?;
            let domains = if mailbox == Mailbox::Inbox { strings(item, "domains") } else {
                address(sender).rsplit_once('@').map(|(_, host)| vec![host.to_string()]).unwrap_or_default()
            };
            for domain in domains {
                tx.execute("INSERT OR IGNORE INTO email_domains VALUES (?1,?2,?3,?4)", params![account_id,mailbox.as_str(),id,domain.to_lowercase()])?;
            }
            tx.execute("DELETE FROM email_recipients WHERE account_id=?1 AND mailbox=?2 AND email_id=?3", params![account_id,mailbox.as_str(),id])?;
            for role in ["to", "cc", "bcc"] {
                for recipient in strings(item, role) {
                    tx.execute("INSERT OR IGNORE INTO email_recipients VALUES (?1,?2,?3,?4,?5)", params![account_id,mailbox.as_str(),id,role,address(&recipient)])?;
                }
            }
        }
        if snapshot {
            tx.execute("INSERT INTO remote_pages VALUES (?1,?2,?3,?4,?5,unixepoch()) ON CONFLICT(account_id,mailbox,page_size,after_cursor) DO UPDATE SET page_json=excluded.page_json,downloaded_at=excluded.downloaded_at",
                params![account_id,mailbox.as_str(),limit as i64,after.unwrap_or_default(),page_json])?;
        }
        tx.commit()
    }).await
}

pub(crate) async fn cached_page<T: for<'de> Deserialize<'de>>(
    database: &Database,
    account_id: String,
    mailbox: Mailbox,
    limit: usize,
    after: Option<String>,
    error: Option<String>,
) -> Result<Option<EmailPage<T>>, String> {
    let row = database.run(move |connection| connection.query_row(
        "SELECT page_json,downloaded_at FROM remote_pages WHERE account_id=?1 AND mailbox=?2 AND page_size=?3 AND after_cursor=?4",
        params![account_id,mailbox.as_str(),limit as i64,after.unwrap_or_default()], |row| Ok((row.get::<_,String>(0)?, row.get::<_,i64>(1)?)),
    ).optional()).await?;
    match row {
        None => Ok(None),
        Some((json, downloaded_at)) => {
            let mut page: EmailPage<T> = serde_json::from_str(&json)
                .map_err(|_| "Could not read downloaded email page".to_string())?;
            page.cache = Some(PageCache {
                downloaded_at,
                error,
            });
            Ok(Some(page))
        }
    }
}

pub(crate) async fn store_detail<T: Serialize>(
    database: &Database,
    account_id: String,
    mailbox: Mailbox,
    email_id: String,
    detail: &T,
    search_text: String,
) -> Result<(), String> {
    let json = serde_json::to_string(detail)
        .map_err(|_| "Could not serialize email detail".to_string())?;
    database.run(move |connection| {
        let tx = connection.transaction()?;
        tx.execute("INSERT OR IGNORE INTO account_state VALUES (?1)", [&account_id])?;
        tx.execute("INSERT INTO email_bodies VALUES (?1,?2,?3,?4,?5,unixepoch()) ON CONFLICT(account_id,mailbox,email_id) DO UPDATE SET detail_json=excluded.detail_json,search_text=excluded.search_text,downloaded_at=excluded.downloaded_at", params![account_id,mailbox.as_str(),email_id,json,search_text])?;
        tx.execute("UPDATE emails SET search_body=?4 WHERE account_id=?1 AND mailbox=?2 AND email_id=?3", params![account_id,mailbox.as_str(),email_id,search_text])?;
        tx.commit()
    }).await
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sort {
    #[default]
    Newest,
    Oldest,
    SubjectAsc,
    SubjectDesc,
    SenderAsc,
    SenderDesc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCursor {
    value: String,
    email_id: String,
    query_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalQuery {
    pub mailbox: Mailbox,
    pub limit: Option<usize>,
    #[serde(default)]
    pub sort: Sort,
    pub domain: Option<String>,
    pub recipient: Option<String>,
    pub search: Option<String>,
    #[serde(default)]
    pub unread_only: bool,
    pub after: Option<LocalCursor>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub started_at: i64,
    pub metadata_completed_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPage {
    pub items: Vec<Value>,
    pub has_more: bool,
    pub next_cursor: Option<LocalCursor>,
    pub sync: Option<SyncStatus>,
    pub is_partial: bool,
    pub downloaded_at: Option<i64>,
}

pub(crate) fn sync_status(
    connection: &rusqlite::Connection,
    account: &str,
    mailbox: Mailbox,
) -> rusqlite::Result<Option<SyncStatus>> {
    connection.query_row("SELECT started_at,metadata_completed_at,completed_at,last_error FROM mailbox_sync WHERE account_id=?1 AND mailbox=?2", params![account,mailbox.as_str()], |row| Ok(SyncStatus { started_at: row.get(0)?, metadata_completed_at: row.get(1)?, completed_at: row.get(2)?, last_error: row.get(3)? })).optional()
}

pub(crate) async fn query(
    database: &Database,
    account: String,
    mut query: LocalQuery,
) -> Result<LocalPage, String> {
    let limit = query.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err("Local page limit must be between 1 and 100".into());
    }
    query.domain = query
        .domain
        .map(|domain| domain.trim().to_lowercase())
        .filter(|domain| !domain.is_empty());
    query.recipient = query
        .recipient
        .map(|recipient| address(&recipient))
        .filter(|recipient| !recipient.is_empty());
    query.search = query
        .search
        .map(|search| search.trim().to_string())
        .filter(|search| !search.is_empty());
    if query
        .search
        .as_ref()
        .is_some_and(|search| search.len() > 512)
    {
        return Err("Search must not exceed 512 bytes".into());
    }
    let key = serde_json::to_string(&(
        &account,
        query.mailbox,
        query.sort,
        &query.domain,
        &query.recipient,
        &query.search,
        query.unread_only,
    ))
    .map_err(|_| "Invalid local query")?;
    if query
        .after
        .as_ref()
        .is_some_and(|cursor| cursor.query_key != key)
    {
        return Err("Local cursor belongs to a different account or query".into());
    }
    let (column, descending, numeric) = match query.sort {
        Sort::Newest => ("created_at_ms", true, true),
        Sort::Oldest => ("created_at_ms", false, true),
        Sort::SubjectAsc => ("subject_sort", false, false),
        Sort::SubjectDesc => ("subject_sort", true, false),
        Sort::SenderAsc => ("sender_sort", false, false),
        Sort::SenderDesc => ("sender_sort", true, false),
    };
    let mut sql = format!("SELECT CASE WHEN t.thread_id IS NULL THEN e.metadata_json ELSE json_set(e.metadata_json,'$.threadId',t.thread_id,'$.threadCount',(SELECT count(DISTINCT COALESCE(json_extract(b.detail_json,'$.messageId'),siblings.email_id)) FROM email_threads siblings LEFT JOIN email_bodies b ON b.account_id=siblings.account_id AND b.mailbox=siblings.mailbox AND b.email_id=siblings.email_id WHERE siblings.account_id=t.account_id AND siblings.thread_id=t.thread_id AND NOT EXISTS(SELECT 1 FROM deleted_emails d WHERE d.account_id=siblings.account_id AND d.mailbox=siblings.mailbox AND d.email_id=siblings.email_id))) END,CAST(e.{column} AS TEXT),e.email_id,substr(e.search_body,1,240),m.email_id IS NOT NULL FROM emails e LEFT JOIN read_markers m ON m.account_id=e.account_id AND m.mailbox=e.mailbox AND m.email_id=e.email_id LEFT JOIN email_threads t ON t.account_id=e.account_id AND t.mailbox=e.mailbox AND t.email_id=e.email_id WHERE e.account_id=? AND e.mailbox=? AND NOT EXISTS(SELECT 1 FROM deleted_emails d WHERE d.account_id=e.account_id AND d.mailbox=e.mailbox AND d.email_id=e.email_id)");
    let mut values: Vec<SqlValue> = vec![
        account.clone().into(),
        query.mailbox.as_str().to_string().into(),
    ];
    if let Some(domain) = &query.domain {
        sql.push_str(" AND EXISTS(SELECT 1 FROM email_domains d WHERE d.account_id=e.account_id AND d.mailbox=e.mailbox AND d.email_id=e.email_id AND d.domain=?)");
        values.push(domain.clone().into());
    }
    if let Some(recipient) = &query.recipient {
        sql.push_str(" AND EXISTS(SELECT 1 FROM email_recipients r WHERE r.account_id=e.account_id AND r.mailbox=e.mailbox AND r.email_id=e.email_id AND r.address=?)");
        values.push(recipient.clone().into());
    }
    if let Some(search) = &query.search {
        // Literal terms, never raw FTS operators or SQL interpolation.
        let fts = search
            .split_whitespace()
            .map(|word| format!("\"{}\"", word.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" AND ");
        sql.push_str(" AND e.rowid IN (SELECT rowid FROM email_search WHERE email_search MATCH ?)");
        values.push(fts.into());
    }
    if query.unread_only {
        sql.push_str(" AND NOT EXISTS(SELECT 1 FROM read_markers m WHERE m.account_id=e.account_id AND m.mailbox=e.mailbox AND m.email_id=e.email_id)");
    }
    if let Some(cursor) = &query.after {
        let comparator = if descending { "<" } else { ">" };
        sql.push_str(&format!(" AND (e.{column},e.email_id) {comparator} (?,?)"));
        values.push(if numeric {
            SqlValue::Integer(cursor.value.parse().map_err(|_| "Invalid date cursor")?)
        } else {
            cursor.value.clone().into()
        });
        values.push(cursor.email_id.clone().into());
    }
    let direction = if descending { "DESC" } else { "ASC" };
    sql.push_str(&format!(
        " ORDER BY e.{column} {direction},e.email_id {direction} LIMIT ?"
    ));
    values.push((limit as i64 + 1).into());
    database
        .run(move |connection| {
            let mut statement = connection.prepare(&sql)?;
            let mut rows = statement
                .query_map(params_from_iter(values), |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, bool>(4)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let has_more = rows.len() > limit;
            rows.truncate(limit);
            let cursor = if has_more {
                rows.last().map(|(_, value, id, _, _)| LocalCursor {
                    value: value.clone(),
                    email_id: id.clone(),
                    query_key: key,
                })
            } else {
                None
            };
            let items = rows
                .into_iter()
                .map(|(json, _, _, preview, is_read)| {
                    let mut item: Value = serde_json::from_str(&json).map_err(json_error)?;
                    let fields = item.as_object_mut().ok_or(rusqlite::Error::InvalidQuery)?;
                    fields.insert(
                        "preview".into(),
                        Value::String(preview.split_whitespace().collect::<Vec<_>>().join(" ")),
                    );
                    fields.insert("isRead".into(), Value::Bool(is_read));
                    Ok(item)
                })
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let sync = sync_status(connection, &account, query.mailbox)?;
            let is_partial = sync
                .as_ref()
                .is_none_or(|state| state.metadata_completed_at.is_none());
            let downloaded_at: Option<i64> = connection.query_row(
                "SELECT MAX(downloaded_at) FROM remote_pages WHERE account_id=?1 AND mailbox=?2",
                params![account, query.mailbox.as_str()],
                |row| row.get(0),
            )?;
            Ok(LocalPage {
                items,
                has_more,
                next_cursor: cursor,
                sync,
                is_partial,
                downloaded_at,
            })
        })
        .await
}

#[cfg(test)]
mod timestamp_tests {
    use super::timestamp;

    #[test]
    fn accepts_documented_resend_postgres_and_rfc3339_dates() {
        let expected = timestamp("2026-04-03T22:13:42.674981Z").unwrap();
        for value in [
            "2026-04-03 22:13:42.674981+00",
            "2026-04-03 22:13:42.674981+00:00",
            "2026-04-03 22:13:42.674981 +00:00",
            "2026-04-03 22:13:42.674981+0000",
            "2026-04-03T22:13:42.674981+00",
            "2026-04-04 00:13:42.674981+02",
            "2026-04-04 00:13:42.674981+02:00",
            "2026-04-03 19:13:42.674981-03",
            "2026-04-03 19:13:42.674981-0300",
            " 2026-04-03 22:13:42.674981+00 ",
        ] {
            assert_eq!(timestamp(value).unwrap(), expected, "{value}");
        }
        assert_eq!(
            timestamp("2026-04-03 22:13:42+00").unwrap(),
            timestamp("2026-04-03T22:13:42Z").unwrap()
        );
    }

    #[test]
    fn rejects_invalid_and_timezone_less_dates_instead_of_faking_epoch_or_utc() {
        for value in [
            "",
            "not-a-date",
            "2026-04-03",
            "2026-04-03 22:13:42",
            "2026-02-30 22:13:42+00",
            "2026-04-03 22:13:42+99",
            "🚀",
        ] {
            assert!(timestamp(value).is_err(), "{value}");
        }
    }
}
