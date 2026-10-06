//! Rust-owned local persistence. No credentials, frontend SQL or remote truth.
//!
//! One lazily opened connection per app is shared by clones. Async callers wait
//! for a single permit BEFORE entering the blocking pool; SQLite and filesystem
//! I/O never execute on the async executor. A cancelled caller cannot release
//! that permit while its already-started blocking operation is still running.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};
use tokio::sync::Semaphore;

const MIGRATIONS: &[&str] = &[
    include_str!("migrations/001_local_state.sql"),
    include_str!("migrations/002_json_imports.sql"),
    include_str!("migrations/003_mail_archive.sql"),
];
pub(crate) const DATABASE_FILE: &str = "onlysend.sqlite3";

#[derive(Clone)]
pub(crate) struct Database {
    inner: Arc<Inner>,
}

struct Inner {
    path: PathBuf,
    connection: Mutex<Option<Connection>>,
    gate: Arc<Semaphore>,
}

impl Database {
    /// Does no I/O. Safe to register synchronously as Tauri managed state.
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            inner: Arc::new(Inner {
                path,
                connection: Mutex::new(None),
                gate: Arc::new(Semaphore::new(1)),
            }),
        }
    }

    /// Initialization failures leave the connection unset and can be retried.
    pub(crate) async fn initialize(&self) -> Result<(), String> {
        self.run(|_| Ok(())).await
    }

    /// Internal adapter boundary, never exposed as an invoke command.
    /// Account-owned adapters must take explicit account IDs, validate them
    /// against auth.json, and use transactions for grouped writes.
    pub(crate) async fn run<T, F>(&self, operation: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> rusqlite::Result<T> + Send + 'static,
    {
        let permit = self
            .inner
            .gate
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "Local database worker is closed".to_string())?;
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut guard = inner
                .connection
                .lock()
                .map_err(|_| "Local database connection is poisoned".to_string())?;
            if guard.is_none() {
                *guard = Some(open(&inner.path)?);
            }
            let connection = guard
                .as_mut()
                .ok_or_else(|| "Local database connection is unavailable".to_string())?;
            operation(connection).map_err(sql_error)
        })
        .await
        .map_err(|_| "Local database worker failed".to_string())?
    }
}

fn sql_error(error: rusqlite::Error) -> String {
    format!("Local database operation failed: {error}")
}

fn open(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create local database directory: {error}"))?;
    }
    // Mail content is private user data. SQLite inherits database permissions
    // for WAL/SHM files; credentials are still never stored here.
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)
            .map_err(|error| format!("Could not open private local database: {error}"))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("Could not protect local database: {error}"))?;
    }
    let mut connection = Connection::open(path).map_err(sql_error)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(sql_error)?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(sql_error)?;
    let journal: String = connection
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .map_err(sql_error)?;
    if !journal.eq_ignore_ascii_case("wal") {
        return Err("Local database requires WAL journal mode".to_string());
    }
    // Preferences/read state are local user data, not disposable remote cache.
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(sql_error)?;
    migrate(&mut connection, MIGRATIONS)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for suffix in ["-wal", "-shm"] {
            let mut sidecar = path.as_os_str().to_os_string();
            sidecar.push(suffix);
            match std::fs::set_permissions(
                Path::new(&sidecar),
                std::fs::Permissions::from_mode(0o600),
            ) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!("Could not protect local database sidecar: {error}"))
                }
            }
        }
    }
    Ok(connection)
}

fn migrate(connection: &mut Connection, migrations: &[&str]) -> Result<(), String> {
    // Lock before reading user_version so concurrent initializers cannot race.
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_error)?;
    let version: i64 = transaction
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(sql_error)?;
    if version < 0 || version > migrations.len() as i64 {
        return Err(format!(
            "Unsupported local database schema version {version}; maximum supported is {}",
            migrations.len(),
        ));
    }
    for (index, sql) in migrations.iter().enumerate().skip(version as usize) {
        transaction.execute_batch(sql).map_err(sql_error)?;
        transaction
            .pragma_update(None, "user_version", (index + 1) as i64)
            .map_err(sql_error)?;
    }
    transaction.commit().map_err(sql_error)
}

pub(crate) mod local_state;
pub(crate) mod mail;
pub(crate) mod sync;

#[cfg(test)]
mod archive_tests;
#[cfg(test)]
mod tests;
