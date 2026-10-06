//! Persistent downloaded-mail archive. Remote pages and local keysets are
//! intentionally different contracts; remote deletions never prune this copy.
use rusqlite::{params, params_from_iter, types::Value as SqlValue, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Wry};

use super::{local_state, Database};
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

fn timestamp(value: &str) -> Result<i64, String> {
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
            tx.execute("INSERT INTO emails(account_id,mailbox,email_id,created_at_ms,subject,sender,subject_sort,sender_sort,recipient_text,search_body,metadata_json,downloaded_at)
                VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,COALESCE((SELECT search_text FROM email_bodies WHERE account_id=?1 AND mailbox=?2 AND email_id=?3),''),?10,unixepoch())
                ON CONFLICT(account_id,mailbox,email_id) DO UPDATE SET created_at_ms=excluded.created_at_ms,subject=excluded.subject,sender=excluded.sender,subject_sort=excluded.subject_sort,sender_sort=excluded.sender_sort,recipient_text=excluded.recipient_text,metadata_json=excluded.metadata_json,downloaded_at=excluded.downloaded_at",
                params![account_id, mailbox.as_str(), id, date, subject, sender, subject.to_lowercase(), sender.to_lowercase(), recipients.join(" "), metadata])?;
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

#[tauri::command]
pub async fn get_cached_mail_page(
    app: AppHandle<Wry>,
    account_id: String,
    mailbox: Mailbox,
    limit: Option<usize>,
    after: Option<String>,
) -> Result<Option<EmailPage<Value>>, String> {
    let _ = crate::infrastructure::pagination::list_options(limit, after.as_deref())?;
    let database = local_state::for_account(&app, &account_id).await?;
    cached_page(
        &database,
        account_id,
        mailbox,
        limit.unwrap_or(12),
        after,
        None,
    )
    .await
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCursor {
    value: String,
    email_id: String,
    query_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
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
    let mut sql = format!("SELECT e.metadata_json,CAST(e.{column} AS TEXT),e.email_id FROM emails e WHERE e.account_id=? AND e.mailbox=?");
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
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let has_more = rows.len() > limit;
            rows.truncate(limit);
            let cursor = if has_more {
                rows.last().map(|(_, value, id)| LocalCursor {
                    value: value.clone(),
                    email_id: id.clone(),
                    query_key: key,
                })
            } else {
                None
            };
            let items = rows
                .into_iter()
                .map(|(json, _, _)| serde_json::from_str(&json).map_err(json_error))
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

#[tauri::command]
pub async fn query_local_mail(
    app: AppHandle<Wry>,
    account_id: String,
    query: LocalQuery,
) -> Result<LocalPage, String> {
    let database = local_state::for_account(&app, &account_id).await?;
    self::query(&database, account_id, query).await
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
