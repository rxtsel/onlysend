use super::{migrate, Database, MIGRATIONS};
use rusqlite::{params, Connection};
use std::path::PathBuf;

struct TemporaryDatabase {
    directory: PathBuf,
}

impl TemporaryDatabase {
    fn new() -> Self {
        Self {
            directory: std::env::temp_dir()
                .join(format!("onlysend-db-test-{}", uuid::Uuid::new_v4())),
        }
    }

    fn path(&self) -> PathBuf {
        self.directory.join("nested").join("onlysend.sqlite3")
    }

    fn database(&self) -> Database {
        Database::new(self.path())
    }
}

impl Drop for TemporaryDatabase {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[tokio::test]
async fn initializes_real_file_with_versioned_schema_and_connection_pragmas() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    assert!(!temporary.path().exists());
    database.initialize().await.unwrap();
    database.initialize().await.unwrap();
    let (version, journal, foreign_keys, timeout, synchronous) = database
        .run(|connection| {
            Ok((
                connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
                connection
                    .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))?,
                connection.pragma_query_value(None, "foreign_keys", |row| row.get::<_, i64>(0))?,
                connection.pragma_query_value(None, "busy_timeout", |row| row.get::<_, i64>(0))?,
                connection.pragma_query_value(None, "synchronous", |row| row.get::<_, i64>(0))?,
            ))
        })
        .await
        .unwrap();
    assert_eq!(version, 1);
    assert_eq!(journal, "wal");
    assert_eq!(foreign_keys, 1);
    assert_eq!(timeout, 5000);
    assert_eq!(synchronous, 2);
    assert!(temporary.path().is_file());
}

#[tokio::test]
async fn persists_local_state_across_reopen_without_rerunning_migrations() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    database
        .run(|connection| {
            let transaction = connection.transaction()?;
            transaction.execute(
                "INSERT INTO account_state VALUES ('stable-account-uuid')",
                [],
            )?;
            transaction.execute(
                "INSERT INTO read_markers VALUES ('stable-account-uuid', 'inbox', 'email')",
                [],
            )?;
            transaction.commit()
        })
        .await
        .unwrap();
    drop(database);
    let database = temporary.database();
    let account: String = database
        .run(|connection| {
            connection.query_row("SELECT account_id FROM read_markers", [], |row| row.get(0))
        })
        .await
        .unwrap();
    assert_eq!(account, "stable-account-uuid");
}

#[tokio::test]
async fn isolates_accounts_mailboxes_and_empty_selections_without_read_marker_cap() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    database
        .run(|connection| {
            let transaction = connection.transaction()?;
            for account in ["a", "b"] {
                transaction.execute("INSERT INTO account_state VALUES (?1)", [account])?;
            }
            transaction.execute("INSERT INTO domain_preferences VALUES ('a')", [])?;
            for (account, mailbox) in [("a", "inbox"), ("a", "sent"), ("b", "inbox")] {
                transaction.execute(
                    "INSERT INTO read_markers VALUES (?1, ?2, 'same-id')",
                    params![account, mailbox],
                )?;
            }
            for index in 0..301 {
                transaction.execute(
                    "INSERT INTO read_markers VALUES ('a', 'inbox', ?1)",
                    [format!("email-{index}")],
                )?;
            }
            transaction.commit()
        })
        .await
        .unwrap();
    let (markers, saved_empty, unconfigured) = database
        .run(|connection| {
            Ok((
                connection.query_row("SELECT count(*) FROM read_markers", [], |row| {
                    row.get::<_, i64>(0)
                })?,
                connection.query_row(
                    "SELECT count(*) FROM domain_preferences WHERE account_id = 'a'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                connection.query_row(
                    "SELECT count(*) FROM domain_preferences WHERE account_id = 'b'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
            ))
        })
        .await
        .unwrap();
    assert_eq!(markers, 304);
    assert_eq!(saved_empty, 1);
    assert_eq!(unconfigured, 0);
    assert!(database
        .run(|connection| connection.execute(
            "INSERT INTO read_markers VALUES ('a', 'inbox', 'same-id')",
            [],
        ))
        .await
        .is_err());
    assert!(database
        .run(|connection| connection.execute(
            "INSERT INTO read_markers VALUES ('missing-account', 'inbox', 'email')",
            [],
        ))
        .await
        .is_err());
    assert!(database
        .run(|connection| connection.execute(
            "INSERT INTO read_markers VALUES ('a', 'invalid-mailbox', 'email')",
            [],
        ))
        .await
        .is_err());
}

#[tokio::test]
async fn account_deletion_cascades_only_its_own_local_state() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    database
        .run(|connection| {
            let transaction = connection.transaction()?;
            for account in ["a", "b"] {
                transaction.execute("INSERT INTO account_state VALUES (?1)", [account])?;
                transaction.execute("INSERT INTO domain_preferences VALUES (?1)", [account])?;
                transaction.execute(
                    "INSERT INTO included_domains VALUES (?1, 'inaccessible-domain')",
                    [account],
                )?;
                transaction.execute(
                    "INSERT INTO read_markers VALUES (?1, 'inbox', 'same-id')",
                    [account],
                )?;
            }
            transaction.execute("DELETE FROM account_state WHERE account_id = 'a'", [])?;
            transaction.commit()
        })
        .await
        .unwrap();
    database
        .run(|connection| {
            for table in [
                "account_state",
                "domain_preferences",
                "included_domains",
                "read_markers",
            ] {
                let account: String = connection.query_row(
                    &format!("SELECT account_id FROM {table}"),
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(account, "b");
            }
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn failed_migration_rolls_back_schema_and_version_preserving_existing_data() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    database
        .run(|connection| {
            connection.execute("INSERT INTO account_state VALUES ('a')", [])?;
            let broken =
                "CREATE TABLE partial_migration (id TEXT); INSERT INTO missing_table VALUES (1);";
            assert!(migrate(connection, &[MIGRATIONS[0], broken]).is_err());
            let version: i64 =
                connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
            let partial: i64 = connection.query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'partial_migration'",
                [],
                |row| row.get(0),
            )?;
            let accounts: i64 =
                connection.query_row("SELECT count(*) FROM account_state", [], |row| row.get(0))?;
            assert_eq!(version, 1);
            assert_eq!(partial, 0);
            assert_eq!(accounts, 1);
            migrate(connection, MIGRATIONS).unwrap();
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn refuses_newer_schema_without_downgrading_or_deleting_data() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    database
        .run(|connection| {
            connection.execute("INSERT INTO account_state VALUES ('a')", [])?;
            connection.pragma_update(None, "user_version", 999)
        })
        .await
        .unwrap();
    drop(database);
    let database = temporary.database();
    assert!(database
        .initialize()
        .await
        .unwrap_err()
        .contains("Unsupported local database schema version 999"));
    let connection = Connection::open(temporary.path()).unwrap();
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    let accounts: i64 = connection
        .query_row("SELECT count(*) FROM account_state", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 999);
    assert_eq!(accounts, 1);
}

#[tokio::test]
async fn initialization_can_retry_after_filesystem_failure() {
    let temporary = TemporaryDatabase::new();
    std::fs::create_dir_all(&temporary.directory).unwrap();
    let blocker = temporary.directory.join("nested");
    std::fs::write(&blocker, b"not a directory").unwrap();
    let database = temporary.database();
    assert!(database.initialize().await.is_err());
    std::fs::remove_file(blocker).unwrap();
    database.initialize().await.unwrap();
}

#[tokio::test]
async fn concurrent_operations_share_one_connection_and_do_not_lose_writes() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    let mut tasks = Vec::new();
    for index in 0..32 {
        let database = database.clone();
        tasks.push(tokio::spawn(async move {
            database
                .run(move |connection| {
                    connection.execute(
                        "INSERT INTO account_state VALUES (?1)",
                        [format!("account-{index}")],
                    )
                })
                .await
                .unwrap();
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    let count: i64 = database
        .run(|connection| {
            connection.query_row("SELECT count(*) FROM account_state", [], |row| row.get(0))
        })
        .await
        .unwrap();
    assert_eq!(count, 32);
}

#[tokio::test]
async fn cancelling_a_caller_does_not_release_the_running_workers_permit() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (finish_tx, finish_rx) = std::sync::mpsc::channel();
    let first_database = database.clone();
    let first = tokio::spawn(async move {
        first_database
            .run(move |connection| {
                let transaction = connection.transaction()?;
                transaction.execute("INSERT INTO account_state VALUES ('a')", [])?;
                started_tx.send(()).unwrap();
                finish_rx.recv().unwrap();
                transaction.commit()
            })
            .await
    });
    started_rx.await.unwrap();
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    assert_eq!(database.inner.gate.available_permits(), 0);
    finish_tx.send(()).unwrap();
    let count: i64 = database
        .run(|connection| {
            connection.query_row("SELECT count(*) FROM account_state", [], |row| row.get(0))
        })
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(database.inner.gate.available_permits(), 1);
}
