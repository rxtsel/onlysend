//! Bounded, sequential background archive download; never deletes remote mail.
use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::params;
use tauri::{AppHandle, Wry};

use super::{
    local_state,
    mail::{self, Mailbox},
    Database,
};

static RUNNING: LazyLock<Mutex<HashSet<(String, Mailbox)>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
// Share background pacing across all accounts/mailboxes, not one timer per
// worker. Interactive requests remain usable while the archive is downloading.
static REQUEST_CLOCK: LazyLock<tokio::sync::Mutex<Instant>> =
    LazyLock::new(|| tokio::sync::Mutex::new(Instant::now()));
async fn network_turn() {
    let mut last = REQUEST_CLOCK.lock().await;
    let next = *last + Duration::from_millis(650);
    tokio::time::sleep_until(tokio::time::Instant::from_std(next)).await;
    *last = Instant::now();
}

struct Flight(String, Mailbox);
impl Drop for Flight {
    fn drop(&mut self) {
        if let Ok(mut running) = RUNNING.lock() {
            running.remove(&(self.0.clone(), self.1));
        }
    }
}

pub(crate) async fn start(
    app: &AppHandle<Wry>,
    account: String,
    mailbox: Mailbox,
    limit: usize,
    force: bool,
) -> Result<bool, String> {
    let database = local_state::for_account(app, &account).await?;
    let key = (account.clone(), mailbox);
    {
        let mut running = RUNNING
            .lock()
            .map_err(|_| "Mailbox synchronization lock failed")?;
        if !running.insert(key) {
            return Ok(false);
        }
    }
    let flight = Flight(account.clone(), mailbox);
    let id = account.clone();
    let status = database
        .run(move |connection| mail::sync_status(connection, &id, mailbox))
        .await?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "System clock is unavailable")?
        .as_secs() as i64;
    // Restarting a scan on every frontend poll would hammer the API. Failed or
    // interrupted scans have the same backoff; an explicit retry can bypass it.
    if !force && status.is_some_and(|status| now.saturating_sub(status.started_at) < 300) {
        return Ok(false);
    }
    let id = account.clone();
    database.run(move |connection| {
        connection.execute("INSERT INTO mailbox_sync VALUES (?1,?2,unixepoch(),NULL,NULL,NULL) ON CONFLICT(account_id,mailbox) DO UPDATE SET started_at=excluded.started_at,metadata_completed_at=NULL,completed_at=NULL,last_error=NULL", params![id,mailbox.as_str()])?;
        Ok(())
    }).await?;
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _flight = flight;
        let result = scan(&handle, &database, &account, mailbox, limit).await;
        let error = result.err();
        let completed = error.is_none();
        if database.run(move |connection| {
            connection.execute("UPDATE mailbox_sync SET completed_at=CASE WHEN ?3 THEN unixepoch() ELSE NULL END,last_error=?4 WHERE account_id=?1 AND mailbox=?2", params![account,mailbox.as_str(),completed,error])?;
            Ok(())
        }).await.is_err() {
            eprintln!("[WARN] Could not persist mailbox synchronization status");
        }
    });
    Ok(true)
}

async fn scan(
    app: &AppHandle<Wry>,
    database: &Database,
    account: &str,
    mailbox: Mailbox,
    limit: usize,
) -> Result<(), String> {
    let mut after = None;
    let mut cursors = HashSet::new();
    let mut bodies = Vec::new();
    loop {
        // Removed accounts stop before any further network requests.
        crate::infrastructure::credentials_store::account_credential(app, account)?;
        let credential = tokio::time::timeout(
            Duration::from_secs(30),
            crate::oauth::get_account_credential(app, account),
        )
        .await
        .map_err(|_| "Credential refresh timed out")??;
        crate::infrastructure::credentials_store::account_credential(app, account)?;
        network_turn().await;
        crate::infrastructure::credentials_store::account_credential(app, account)?;
        let resend = resend_rs::Resend::new(&credential);
        let (ids, more, next) = match mailbox {
            Mailbox::Inbox => {
                let page = tokio::time::timeout(
                    Duration::from_secs(30),
                    crate::inbound::fetch_inbound_page(&resend, Some(limit), after.as_deref()),
                )
                .await
                .map_err(|_| "Inbox download timed out")??;
                crate::infrastructure::credentials_store::account_credential(app, account)?;
                mail::store_scan_page(
                    database,
                    account.to_string(),
                    mailbox,
                    limit,
                    after.clone(),
                    &page,
                )
                .await?;
                (
                    page.items
                        .iter()
                        .map(|item| item.id.clone())
                        .collect::<Vec<_>>(),
                    page.has_more,
                    page.next_cursor,
                )
            }
            Mailbox::Sent => {
                let page = tokio::time::timeout(
                    Duration::from_secs(30),
                    crate::sent::fetch_sent_page(&resend, Some(limit), after.as_deref()),
                )
                .await
                .map_err(|_| "Sent download timed out")??;
                crate::infrastructure::credentials_store::account_credential(app, account)?;
                mail::store_scan_page(
                    database,
                    account.to_string(),
                    mailbox,
                    limit,
                    after.clone(),
                    &page,
                )
                .await?;
                (
                    page.items
                        .iter()
                        .map(|item| item.id.clone())
                        .collect::<Vec<_>>(),
                    page.has_more,
                    page.next_cursor,
                )
            }
        };
        bodies.extend(ids);
        if !more {
            break;
        }
        let cursor = next.ok_or("Remote download has no continuation")?;
        if !cursors.insert(cursor.clone()) {
            return Err("Remote download cursor repeated".into());
        }
        after = Some(cursor);
    }
    // Finish the metadata traversal first. A unavailable body must not prevent
    // discovering older messages or falsely invalidate complete metadata.
    let id = account.to_string();
    database.run(move |connection| {
        connection.execute("UPDATE mailbox_sync SET metadata_completed_at=unixepoch() WHERE account_id=?1 AND mailbox=?2", params![id,mailbox.as_str()])?;
        Ok(())
    }).await?;
    for id in bodies {
        crate::infrastructure::credentials_store::account_credential(app, account)?;
        if mail::cached_detail(
            database,
            account.to_string(),
            mailbox.as_str().to_string(),
            id.clone(),
        )
        .await?
        .is_some()
        {
            continue;
        }
        network_turn().await;
        let download = async {
            match mailbox {
                Mailbox::Inbox => {
                    crate::inbound::get_inbound_email(app.clone(), account.to_string(), id).await?;
                }
                Mailbox::Sent => {
                    crate::sent::get_sent_email(app.clone(), account.to_string(), id).await?;
                }
            }
            Ok::<_, String>(())
        };
        tokio::time::timeout(Duration::from_secs(30), download)
            .await
            .map_err(|_| "Email body download timed out")??;
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveStatus {
    downloaded_messages: i64,
    downloaded_bodies: i64,
    running: bool,
    sync: Option<mail::SyncStatus>,
}

#[tauri::command]
pub async fn get_mail_archive_status(
    app: AppHandle<Wry>,
    account_id: String,
    mailbox: Mailbox,
) -> Result<ArchiveStatus, String> {
    let database = local_state::for_account(&app, &account_id).await?;
    let id = account_id.clone();
    let mut status = database
        .run(move |connection| {
            let downloaded_messages = connection.query_row(
                "SELECT count(*) FROM emails WHERE account_id=?1 AND mailbox=?2",
                params![id, mailbox.as_str()],
                |row| row.get(0),
            )?;
            let downloaded_bodies = connection.query_row(
                "SELECT count(*) FROM email_bodies WHERE account_id=?1 AND mailbox=?2",
                params![id, mailbox.as_str()],
                |row| row.get(0),
            )?;
            Ok(ArchiveStatus {
                downloaded_messages,
                downloaded_bodies,
                running: false,
                sync: mail::sync_status(connection, &id, mailbox)?,
            })
        })
        .await?;
    status.running = RUNNING
        .lock()
        .map_err(|_| "Mailbox synchronization lock failed")?
        .contains(&(account_id, mailbox));
    Ok(status)
}

#[tauri::command]
pub async fn sync_mailbox(
    app: AppHandle<Wry>,
    account_id: String,
    mailbox: Mailbox,
    limit: Option<usize>,
    force: Option<bool>,
) -> Result<bool, String> {
    let _ = crate::infrastructure::pagination::list_options(limit, None)?;
    start(
        &app,
        account_id,
        mailbox,
        limit.unwrap_or(16),
        force.unwrap_or(false),
    )
    .await
}
