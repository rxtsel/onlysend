//! Rust-owned inbox polling, persistent deduplication and native notifications.
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::{Mutex, Notify};

use crate::inbound::InboundEmailDto;
use crate::infrastructure::{
    account_locks::AccountLocks,
    credentials_store,
    database::{
        local_state,
        mail::{self, Mailbox},
        sync,
    },
    read_typed, write_key, STORE_FILE,
};

const SETTINGS_KEY: &str = "inbox_poll_minutes";
const PAGE_SIZE: usize = 100;

#[derive(Default)]
pub(crate) struct InboxService {
    refresh_locks: AccountLocks,
    last_success: Mutex<HashMap<String, Instant>>,
    changed: Notify,
}

fn validate_interval(minutes: u32) -> Result<u32, String> {
    match minutes {
        0 | 5 | 10 | 30 | 60 => Ok(minutes),
        _ => Err("Choose Off, 5, 10, 30 or 60 minutes".into()),
    }
}

#[tauri::command]
pub fn get_inbox_poll_minutes(app: AppHandle<Wry>) -> Result<u32, String> {
    validate_interval(
        read_typed(&app, STORE_FILE, SETTINGS_KEY, "inbox polling interval")?.unwrap_or(5),
    )
}

#[tauri::command]
pub fn set_inbox_poll_minutes(app: AppHandle<Wry>, minutes: u32) -> Result<(), String> {
    validate_interval(minutes)?;
    write_key(&app, STORE_FILE, SETTINGS_KEY, serde_json::json!(minutes))?;
    app.state::<InboxService>().changed.notify_one();
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Checkpoint {
    newest_ms: i64,
    ids: HashSet<String>,
}

impl Default for Checkpoint {
    fn default() -> Self {
        Self {
            newest_ms: i64::MIN,
            ids: HashSet::new(),
        }
    }
}

impl Checkpoint {
    fn is_new(&self, id: &str, date: i64) -> bool {
        date > self.newest_ms || (date == self.newest_ms && !self.ids.contains(id))
    }

    fn observe(&mut self, id: &str, date: i64) {
        if date > self.newest_ms {
            self.newest_ms = date;
            self.ids.clear();
        }
        if date == self.newest_ms {
            self.ids.insert(id.to_string());
        }
    }
}

fn observe_page(
    previous: Option<&Checkpoint>,
    checkpoint: &mut Checkpoint,
    seen: &mut HashSet<String>,
    items: &[InboundEmailDto],
) -> Result<(usize, bool), String> {
    let mut count = 0;
    let mut oldest = i64::MAX;
    for email in items {
        let date = mail::timestamp(&email.created_at)?;
        oldest = oldest.min(date);
        if previous.is_some_and(|state| state.is_new(&email.id, date))
            && seen.insert(email.id.clone())
        {
            count += 1;
        }
        checkpoint.observe(&email.id, date);
    }
    // Include every ID tied at the watermark, even across page boundaries.
    let boundary = previous.unwrap_or(checkpoint).newest_ms;
    Ok((count, oldest < boundary))
}

fn load_checkpoint(connection: &Connection, account: &str) -> rusqlite::Result<Option<Checkpoint>> {
    let json: Option<String> = connection
        .query_row(
            "SELECT checkpoint_json FROM inbox_notification_state WHERE account_id=?1",
            [account],
            |row| row.get(0),
        )
        .optional()?;
    json.map(|json| {
        serde_json::from_str(&json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    })
    .transpose()
}

fn save_checkpoint(
    connection: &Connection,
    account: &str,
    checkpoint: &Checkpoint,
) -> rusqlite::Result<()> {
    let json = serde_json::to_string(checkpoint)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    connection.execute(
        "INSERT INTO inbox_notification_state VALUES (?1,?2) ON CONFLICT(account_id) DO UPDATE SET checkpoint_json=excluded.checkpoint_json",
        params![account, json],
    )?;
    Ok(())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InboxUpdated {
    account_id: String,
}

fn should_refresh(last: Option<Instant>, requested: Instant, minutes: u32, force: bool) -> bool {
    if !force && minutes == 0 {
        return false;
    }
    !last.is_some_and(|last| {
        last >= requested
            || (!force && last.elapsed() < Duration::from_secs(u64::from(minutes) * 60))
    })
}

pub(crate) async fn refresh_inbox(
    app: AppHandle<Wry>,
    account_id: String,
    force: bool,
) -> Result<(), String> {
    let requested = Instant::now();
    let service = app.state::<InboxService>();
    let flight = service.refresh_locks.get(&account_id)?;
    let _guard = flight.lock().await;
    credentials_store::account_credential(&app, &account_id)?;
    let last_success = service.last_success.lock().await.get(&account_id).copied();
    if !should_refresh(
        last_success,
        requested,
        get_inbox_poll_minutes(app.clone())?,
        force,
    ) {
        return Ok(());
    }
    // Timeout includes credential refresh, not only the Resend HTTP request.
    let credential = tokio::time::timeout(
        Duration::from_secs(30),
        crate::oauth::get_account_credential(&app, &account_id),
    )
    .await
    .map_err(|_| "Credential refresh timed out")??;
    let resend = resend_rs::Resend::new(&credential);
    let database = local_state::for_account(&app, &account_id).await?;
    let id = account_id.clone();
    let previous = database
        .run(move |connection| load_checkpoint(connection, &id))
        .await?;
    let mut checkpoint = previous.clone().unwrap_or_default();
    let mut count = 0;
    let mut seen = HashSet::new();
    let mut after = None;
    let mut cursors = HashSet::new();
    loop {
        credentials_store::account_credential(&app, &account_id)?;
        sync::network_turn().await;
        let page = tokio::time::timeout(
            Duration::from_secs(30),
            crate::inbound::fetch_inbound_page(&resend, Some(PAGE_SIZE), after.as_deref()),
        )
        .await
        .map_err(|_| "Inbox request timed out")??;
        let (new_count, reached_history) =
            observe_page(previous.as_ref(), &mut checkpoint, &mut seen, &page.items)?;
        count += new_count;
        credentials_store::account_credential(&app, &account_id)?;
        mail::store_scan_page(
            &database,
            account_id.clone(),
            Mailbox::Inbox,
            PAGE_SIZE,
            after.clone(),
            &page,
        )
        .await?;
        // First connection seeds the checkpoint silently; archive sync owns
        // historical pages. Later checks traverse every page of new arrivals.
        if reached_history || !page.has_more {
            break;
        }
        let cursor = page
            .next_cursor
            .ok_or("Inbox download has no continuation")?;
        if !cursors.insert(cursor.clone()) {
            return Err("Inbox download cursor repeated".into());
        }
        after = Some(cursor);
    }
    credentials_store::account_credential(&app, &account_id)?;
    let id = account_id.clone();
    database
        .run(move |connection| save_checkpoint(connection, &id, &checkpoint))
        .await?;
    credentials_store::account_credential(&app, &account_id)?;
    service.last_success.lock().await.insert(account_id.clone(), Instant::now());
    // Native notification failures must not invalidate a successful refresh or
    // repeatedly alert on the same messages. No private mail in lock-screen text.
    if count > 0 && get_inbox_poll_minutes(app.clone()).unwrap_or(0) != 0 {
        let handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if handle
                .notification()
                .builder()
                .title("OnlySend")
                .body(format!(
                    "{count} new email{} in your inbox",
                    if count == 1 { "" } else { "s" }
                ))
                .show()
                .is_err()
            {
                eprintln!("[WARN] Could not display inbox notification");
            }
        });
    }
    if app
        .emit(
            "inbox-updated",
            InboxUpdated {
                account_id: account_id.clone(),
            },
        )
        .is_err()
    {
        eprintln!("[WARN] Could not publish inbox refresh");
    }
    if previous.is_none() || force {
        let download = sync::start(&app, account_id, Mailbox::Inbox, PAGE_SIZE, force).await;
        if force {
            download?;
        } else if download.is_err() {
            eprintln!("[WARN] Could not start inbox archive download");
        }
    }
    Ok(())
}

fn retry_delay(minutes: u32, failures: u32) -> Duration {
    Duration::from_secs((u64::from(minutes) * 60 * (1u64 << failures.min(4))).min(3600))
}

pub(crate) fn start(app: AppHandle<Wry>) {
    tauri::async_runtime::spawn(async move {
        let mut retries: HashMap<String, (u32, Instant)> = HashMap::new();
        loop {
            let minutes = match get_inbox_poll_minutes(app.clone()) {
                Ok(minutes) => minutes,
                Err(_) => {
                    eprintln!("[WARN] Could not read inbox polling settings");
                    5
                }
            };
            if minutes != 0 {
                if let Ok(accounts) = credentials_store::read_accounts(&app) {
                    retries.retain(|id, _| accounts.iter().any(|account| account.id == *id));
                    for account in accounts {
                        if get_inbox_poll_minutes(app.clone()).unwrap_or(0) == 0 {
                            break;
                        }
                        if retries
                            .get(&account.id)
                            .is_some_and(|(_, next)| *next > Instant::now())
                        {
                            continue;
                        }
                        if !crate::infrastructure::settings_store::get_onboarding_state(
                            app.clone(),
                            account.id.clone(),
                        )
                        .is_ok_and(|state| state.complete)
                        {
                            continue;
                        }
                        match refresh_inbox(app.clone(), account.id.clone(), false).await {
                            Ok(()) => {
                                retries.remove(&account.id);
                            }
                            Err(_) => {
                                let failures = retries
                                    .get(&account.id)
                                    .map_or(1, |(count, _)| count.saturating_add(1));
                                retries.insert(
                                    account.id,
                                    (failures, Instant::now() + retry_delay(minutes, failures)),
                                );
                                eprintln!("[WARN] Inbox check failed; backing off");
                            }
                        }
                    }
                }
            }
            let changed = &app.state::<InboxService>().changed;
            if minutes == 0 {
                changed.notified().await;
            } else {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(u64::from(minutes) * 60)) => {},
                    _ = changed.notified() => { retries.clear(); },
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_slow_account_does_not_block_another_accounts_refresh() {
        let service = InboxService::default();
        let a = service.refresh_locks.get("a").unwrap();
        let same_a = service.refresh_locks.get("a").unwrap();
        let b = service.refresh_locks.get("b").unwrap();
        let _a_guard = a.lock().await;
        assert!(same_a.try_lock().is_err());
        let _b_guard = tokio::time::timeout(Duration::from_secs(1), b.lock()).await.unwrap();
        service.last_success.lock().await.insert("b".into(), Instant::now());
        assert!(service.last_success.lock().await.contains_key("b"));
    }

    #[test]
    fn refresh_obeys_ttl_off_manual_override_and_coalesces_waiters() {
        let now = Instant::now();
        assert!(should_refresh(None, now, 5, false));
        assert!(!should_refresh(None, now, 0, false));
        assert!(!should_refresh(
            Some(now - Duration::from_secs(60)),
            now,
            5,
            false
        ));
        assert!(should_refresh(
            Some(now - Duration::from_secs(301)),
            now,
            5,
            false
        ));
        assert!(!should_refresh(
            Some(now - Duration::from_secs(301)),
            now,
            10,
            false
        ));
        assert!(should_refresh(
            Some(now - Duration::from_secs(1)),
            now,
            0,
            true
        ));
        assert!(!should_refresh(Some(now), now, 5, true));
    }

    #[test]
    fn interval_is_bounded_and_backoff_is_capped() {
        for minutes in [0, 5, 10, 30, 60] {
            assert_eq!(validate_interval(minutes).unwrap(), minutes);
        }
        for minutes in [1, 4, 6, 300, u32::MAX] {
            assert!(validate_interval(minutes).is_err());
        }
        assert_eq!(retry_delay(5, 1), Duration::from_secs(600));
        assert_eq!(retry_delay(30, 100), Duration::from_secs(3600));
    }

    #[test]
    fn checkpoint_handles_empty_inbox_ties_deleted_heads_and_repeated_pages() {
        let mut state = Checkpoint::default();
        assert!(state.is_new("first", 1));
        state.observe("old-head", 10);
        state.observe("same-time", 10);
        state.observe("older", 9);
        assert!(!state.is_new("old-head", 10));
        assert!(!state.is_new("same-time", 10));
        assert!(!state.is_new("unseen-history", 9));
        assert!(state.is_new("new-at-same-time", 10));
        assert!(state.is_new("new-head", 11));
        state.observe("new-head", 11);
        assert_eq!(state.ids, HashSet::from(["new-head".into()]));
        assert!(!state.is_new("new-head", 11));
        assert!(!state.is_new("deleted-old-head", 10));
    }

    fn email(id: impl Into<String>, date: &str) -> InboundEmailDto {
        InboundEmailDto {
            id: id.into(),
            created_at: date.into(),
            from: String::new(),
            to: vec![],
            subject: String::new(),
            domain: String::new(),
            domains: vec![],
        }
    }

    #[test]
    fn initial_history_is_silent_and_bursts_span_pages_without_duplicates() {
        let date = "2026-01-01T00:00:00Z";
        let mut checkpoint = Checkpoint::default();
        let mut seen = HashSet::new();
        let baseline = vec![email("old", date)];
        assert_eq!(
            observe_page(None, &mut checkpoint, &mut seen, &baseline).unwrap(),
            (0, false)
        );
        let previous = checkpoint.clone();
        let arrivals: Vec<_> = (0..250)
            .map(|i| email(format!("new-{i}"), "2026-01-02T00:00:00Z"))
            .collect();
        let mut count = 0;
        for page in arrivals.chunks(100) {
            let (new, reached) =
                observe_page(Some(&previous), &mut checkpoint, &mut seen, page).unwrap();
            count += new;
            assert!(!reached);
        }
        count += observe_page(
            Some(&previous),
            &mut checkpoint,
            &mut seen,
            &arrivals[..100],
        )
        .unwrap()
        .0;
        assert_eq!(count, 250);
        let (new, reached) = observe_page(
            Some(&previous),
            &mut checkpoint,
            &mut seen,
            &[email("old", date), email("older", "2025-12-31T00:00:00Z")],
        )
        .unwrap();
        assert_eq!((new, reached), (0, true));
        assert_eq!(checkpoint.ids.len(), 250);
        let saved = checkpoint.clone();
        assert_eq!(
            observe_page(
                Some(&saved),
                &mut checkpoint,
                &mut HashSet::new(),
                &arrivals
            )
            .unwrap()
            .0,
            0
        );
    }

    #[test]
    fn invalid_remote_dates_do_not_change_the_persisted_checkpoint() {
        let mut previous = Checkpoint::default();
        previous.observe("old", 42);
        let mut pending = previous.clone();
        assert!(observe_page(
            Some(&previous),
            &mut pending,
            &mut HashSet::new(),
            &[email("bad", "invalid")]
        )
        .is_err());
        assert_eq!(previous.newest_ms, 42);
        assert!(!previous.is_new("old", 42));
    }

    #[test]
    fn checkpoints_persist_and_are_account_isolated() {
        let path = std::env::temp_dir().join(format!(
            "onlysend-notification-test-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(include_str!(
                "infrastructure/database/migrations/001_local_state.sql"
            ))
            .unwrap();
        connection
            .execute_batch(include_str!(
                "infrastructure/database/migrations/004_inbox_notifications.sql"
            ))
            .unwrap();
        connection
            .execute("INSERT INTO account_state VALUES ('a'),('b')", [])
            .unwrap();
        assert!(load_checkpoint(&connection, "a").unwrap().is_none());
        let mut state = Checkpoint::default();
        state.observe("mail", 42);
        save_checkpoint(&connection, "a", &state).unwrap();
        drop(connection);
        let connection = Connection::open(&path).unwrap();
        let restored = load_checkpoint(&connection, "a").unwrap().unwrap();
        assert!(!restored.is_new("mail", 42));
        assert!(restored.is_new("next", 43));
        assert!(load_checkpoint(&connection, "b").unwrap().is_none());
        connection.execute("UPDATE inbox_notification_state SET checkpoint_json='invalid' WHERE account_id='a'", []).unwrap();
        assert!(load_checkpoint(&connection, "a").is_err());
        drop(connection);
        std::fs::remove_file(path).unwrap();
    }
}
