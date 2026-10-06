//! Typed local-state adapter and idempotent, verified per-account JSON import.
use std::collections::BTreeSet;
use std::io::Write;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Wry};

use super::Database;
use crate::infrastructure::{
    credentials_store, read_typed, settings_store::DomainPreferences, STORE_FILE,
};

#[derive(Serialize, Deserialize)]
pub(crate) struct LegacyState {
    pub account_id: String,
    pub preferences: Option<DomainPreferences>,
    pub read_ids: Vec<String>,
}

/// No active-account fallback, OAuth refresh or credentials in the snapshot.
pub(crate) async fn for_account(
    app: &AppHandle<Wry>,
    account_id: &str,
) -> Result<Database, String> {
    credentials_store::account_credential(app, account_id)?;
    let database = app.state::<Database>().inner().clone();
    let id = account_id.to_string();
    let imported = database
        .run(move |connection| {
            connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM json_imports WHERE account_id = ?1)",
                [id],
                |row| row.get::<_, bool>(0),
            )
        })
        .await?;
    if !imported {
        let snapshot = LegacyState {
            account_id: account_id.to_string(),
            preferences: read_typed(
                app,
                STORE_FILE,
                &format!("domain_preferences:{account_id}"),
                "domain preferences",
            )?,
            read_ids: read_typed(
                app,
                STORE_FILE,
                &format!("read_inbound_ids:{account_id}"),
                "read ids",
            )?
            .unwrap_or_default(),
        };
        import(&database, snapshot).await?;
    }
    // Logout may have occurred while import waited for the database worker.
    credentials_store::account_credential(app, account_id)?;
    Ok(database)
}

fn conversion(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

pub(crate) async fn import(database: &Database, snapshot: LegacyState) -> Result<(), String> {
    let backup_directory = database
        .inner
        .path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("migration-backups");
    database.run(move |connection| {
        let transaction = connection.transaction()?;
        let imported: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM json_imports WHERE account_id = ?1)", [&snapshot.account_id], |row| row.get(0),
        )?;
        if imported { return Ok(()); }
        // Keep JSON untouched. Write and fsync a minimal snapshot BEFORE changes.
        std::fs::create_dir_all(&backup_directory).map_err(conversion)?;
        let backup_name = format!("local-state-{}.json", uuid::Uuid::new_v4());
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut backup = options.open(backup_directory.join(&backup_name)).map_err(conversion)?;
        let bytes = serde_json::to_vec_pretty(&snapshot).map_err(conversion)?;
        backup.write_all(&bytes).map_err(conversion)?;
        backup.sync_all().map_err(conversion)?;
        #[cfg(unix)]
        std::fs::File::open(&backup_directory).and_then(|directory| directory.sync_all()).map_err(conversion)?;
        let verified: LegacyState = serde_json::from_slice(&std::fs::read(backup_directory.join(&backup_name)).map_err(conversion)?).map_err(conversion)?;
        if serde_json::to_value(&verified).map_err(conversion)? != serde_json::to_value(&snapshot).map_err(conversion)? {
            return Err(rusqlite::Error::InvalidQuery);
        }
        transaction.execute("INSERT INTO account_state VALUES (?1) ON CONFLICT(account_id) DO NOTHING", [&snapshot.account_id])?;
        if let Some(preferences) = &snapshot.preferences {
            transaction.execute("INSERT INTO domain_preferences VALUES (?1)", [&snapshot.account_id])?;
            for id in &preferences.included_domain_ids {
                transaction.execute("INSERT INTO included_domains VALUES (?1, ?2) ON CONFLICT(account_id,domain_id) DO NOTHING", params![snapshot.account_id, id])?;
            }
            let actual = read_preferences(&transaction, &snapshot.account_id)?.ok_or(rusqlite::Error::InvalidQuery)?;
            let expected: BTreeSet<_> = preferences.included_domain_ids.iter().cloned().collect();
            if actual.included_domain_ids.into_iter().collect::<BTreeSet<_>>() != expected {
                return Err(rusqlite::Error::InvalidQuery);
            }
        }
        for id in &snapshot.read_ids {
            transaction.execute("INSERT INTO read_markers VALUES (?1, 'inbox', ?2) ON CONFLICT(account_id,mailbox,email_id) DO NOTHING", params![snapshot.account_id, id])?;
        }
        let actual: BTreeSet<_> = read_ids(&transaction, &snapshot.account_id)?.into_iter().collect();
        if actual != snapshot.read_ids.iter().cloned().collect() { return Err(rusqlite::Error::InvalidQuery); }
        transaction.execute("INSERT INTO json_imports VALUES (?1, ?2, unixepoch())", params![snapshot.account_id, backup_name])?;
        transaction.commit()
    }).await
}

pub(crate) fn read_preferences(
    connection: &rusqlite::Connection,
    account_id: &str,
) -> rusqlite::Result<Option<DomainPreferences>> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM domain_preferences WHERE account_id = ?1)",
        [account_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(None);
    }
    let mut statement = connection.prepare(
        "SELECT domain_id FROM included_domains WHERE account_id = ?1 ORDER BY domain_id",
    )?;
    let ids = statement
        .query_map([account_id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(Some(DomainPreferences {
        included_domain_ids: ids,
    }))
}

pub(crate) fn read_ids(
    connection: &rusqlite::Connection,
    account_id: &str,
) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare("SELECT email_id FROM read_markers WHERE account_id = ?1 AND mailbox = 'inbox' ORDER BY email_id")?;
    let ids = statement
        .query_map([account_id], |row| row.get(0))?
        .collect();
    ids
}

pub(crate) async fn preferences(
    database: &Database,
    account_id: String,
) -> Result<Option<DomainPreferences>, String> {
    database
        .run(move |connection| read_preferences(connection, &account_id))
        .await
}

pub(crate) async fn save_preferences(
    database: &Database,
    account_id: String,
    preferences: DomainPreferences,
) -> Result<(), String> {
    database.run(move |connection| {
        let transaction = connection.transaction()?;
        transaction.execute("INSERT INTO account_state VALUES (?1) ON CONFLICT(account_id) DO NOTHING", [&account_id])?;
        transaction.execute("INSERT INTO domain_preferences VALUES (?1) ON CONFLICT(account_id) DO NOTHING", [&account_id])?;
        transaction.execute("DELETE FROM included_domains WHERE account_id = ?1", [&account_id])?;
        for id in preferences.included_domain_ids {
            transaction.execute("INSERT INTO included_domains VALUES (?1, ?2) ON CONFLICT(account_id,domain_id) DO NOTHING", params![account_id, id])?;
        }
        transaction.commit()
    }).await
}
