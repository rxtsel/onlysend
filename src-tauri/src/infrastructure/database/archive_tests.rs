use super::{
    local_state::{self, LegacyState},
    mail::{self, LocalQuery, Mailbox, Sort},
    tests::TemporaryDatabase,
    MIGRATIONS,
};
use crate::infrastructure::{
    pagination::{email_page, EmailPage},
    settings_store::DomainPreferences,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

fn message(id: &str, subject: &str, date: &str) -> Value {
    json!({"id":id,"subject":subject,"createdAt":date,"from":"Sender <sender@remote.example>","to":["recipient@header.example"],"domains":["a.example","b.example"],"domain":"a.example"})
}
fn page(items: Vec<Value>) -> EmailPage<Value> {
    email_page(items, false, None, |item| item["id"].as_str().unwrap()).unwrap()
}
fn query(mailbox: Mailbox) -> LocalQuery {
    LocalQuery {
        mailbox,
        limit: Some(2),
        sort: Sort::Newest,
        domain: None,
        recipient: None,
        search: None,
        unread_only: false,
        after: None,
    }
}

#[tokio::test]
async fn upgrades_version_one_without_losing_user_state() {
    let temporary = TemporaryDatabase::new();
    std::fs::create_dir_all(temporary.path().parent().unwrap()).unwrap();
    let connection = Connection::open(temporary.path()).unwrap();
    connection.execute_batch(MIGRATIONS[0]).unwrap();
    connection
        .execute("INSERT INTO account_state VALUES ('a')", [])
        .unwrap();
    connection
        .execute(
            "INSERT INTO read_markers VALUES ('a','inbox','existing')",
            [],
        )
        .unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
    drop(connection);
    let database = temporary.database();
    database.initialize().await.unwrap();
    assert_eq!(
        database
            .run(|connection| local_state::read_ids(connection, "a"))
            .await
            .unwrap(),
        vec!["existing"]
    );
}

#[tokio::test]
async fn imports_all_markers_with_verified_backup_and_never_overwrites_new_sqlite_state() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    let snapshot = || LegacyState {
        account_id: "a".into(),
        preferences: Some(DomainPreferences {
            included_domain_ids: vec!["missing-domain".into(), "missing-domain".into()],
        }),
        read_ids: (0..301).map(|id| format!("email-{id}")).collect(),
    };
    local_state::import(&database, snapshot()).await.unwrap();
    assert_eq!(
        database
            .run(|connection| local_state::read_ids(connection, "a"))
            .await
            .unwrap()
            .len(),
        301
    );
    assert_eq!(
        local_state::preferences(&database, "a".into())
            .await
            .unwrap()
            .unwrap()
            .included_domain_ids,
        vec!["missing-domain"]
    );
    let backup_name: String = database
        .run(|connection| {
            connection.query_row(
                "SELECT backup_name FROM json_imports WHERE account_id='a'",
                [],
                |row| row.get(0),
            )
        })
        .await
        .unwrap();
    let directory = temporary.path().parent().unwrap().join("migration-backups");
    let backup: Value =
        serde_json::from_slice(&std::fs::read(directory.join(&backup_name)).unwrap()).unwrap();
    assert_eq!(backup["account_id"], "a");
    assert_eq!(backup["read_ids"].as_array().unwrap().len(), 301);
    assert!(backup.get("api_key").is_none());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(directory.join(&backup_name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(temporary.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    local_state::save_preferences(
        &database,
        "a".into(),
        DomainPreferences {
            included_domain_ids: vec![],
        },
    )
    .await
    .unwrap();
    local_state::import(&database, snapshot()).await.unwrap();
    assert!(local_state::preferences(&database, "a".into())
        .await
        .unwrap()
        .unwrap()
        .included_domain_ids
        .is_empty());
    assert_eq!(std::fs::read_dir(directory).unwrap().count(), 1);
}

#[tokio::test]
async fn import_distinguishes_absent_and_empty_preferences_and_rolls_back_bad_data() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    for (account, preferences) in [
        ("a", None),
        (
            "b",
            Some(DomainPreferences {
                included_domain_ids: vec![],
            }),
        ),
    ] {
        local_state::import(
            &database,
            LegacyState {
                account_id: account.into(),
                preferences,
                read_ids: vec![],
            },
        )
        .await
        .unwrap();
    }
    assert!(local_state::preferences(&database, "a".into())
        .await
        .unwrap()
        .is_none());
    assert!(local_state::preferences(&database, "b".into())
        .await
        .unwrap()
        .unwrap()
        .included_domain_ids
        .is_empty());
    assert!(local_state::import(
        &database,
        LegacyState {
            account_id: "bad".into(),
            preferences: None,
            read_ids: vec!["".into()]
        }
    )
    .await
    .is_err());
    let count: i64 = database
        .run(|connection| {
            connection.query_row(
                "SELECT count(*) FROM account_state WHERE account_id='bad'",
                [],
                |row| row.get(0),
            )
        })
        .await
        .unwrap();
    assert_eq!(count, 0);
    // The failed import's backup remains available; no destructive cleanup.
    assert_eq!(
        std::fs::read_dir(temporary.path().parent().unwrap().join("migration-backups"))
            .unwrap()
            .count(),
        3
    );
}

#[tokio::test]
async fn preference_replace_is_atomic_and_rejects_empty_domain_ids() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    local_state::save_preferences(
        &database,
        "a".into(),
        DomainPreferences {
            included_domain_ids: vec!["keep".into()],
        },
    )
    .await
    .unwrap();
    assert!(local_state::save_preferences(
        &database,
        "a".into(),
        DomainPreferences {
            included_domain_ids: vec!["new".into(), "".into()]
        }
    )
    .await
    .is_err());
    assert_eq!(
        local_state::preferences(&database, "a".into())
            .await
            .unwrap()
            .unwrap()
            .included_domain_ids,
        vec!["keep"]
    );
}

#[tokio::test]
async fn date_keysets_are_stable_for_ties_and_use_numeric_timezones() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    mail::store_page(
        &database,
        "a".into(),
        Mailbox::Inbox,
        16,
        None,
        &page(vec![
            message("1", "Zulu", "2026-01-01T00:00:00Z"),
            message("2", "Alpha", "2026-01-01T01:00:00+01:00"),
            message("3", "Beta", "2025-12-31T23:59:59.999Z"),
            message("4", "Beta", "2026-01-01 00:00:00.000 +00:00"),
        ]),
    )
    .await
    .unwrap();
    let first = mail::query(&database, "a".into(), query(Mailbox::Inbox))
        .await
        .unwrap();
    assert_eq!(
        first
            .items
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["4", "2"]
    );
    assert!(first.is_partial);
    let mut second_query = query(Mailbox::Inbox);
    second_query.after = first.next_cursor;
    let second = mail::query(&database, "a".into(), second_query)
        .await
        .unwrap();
    assert_eq!(
        second
            .items
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["1", "3"]
    );
    assert!(!second.has_more);
    for sort in [
        Sort::Oldest,
        Sort::SubjectAsc,
        Sort::SubjectDesc,
        Sort::SenderAsc,
        Sort::SenderDesc,
    ] {
        let mut ids = Vec::new();
        let mut cursor = None;
        loop {
            let mut request = query(Mailbox::Inbox);
            request.sort = sort;
            request.after = cursor;
            let result = mail::query(&database, "a".into(), request).await.unwrap();
            ids.extend(
                result
                    .items
                    .iter()
                    .map(|item| item["id"].as_str().unwrap().to_string()),
            );
            if !result.has_more {
                break;
            }
            cursor = result.next_cursor;
        }
        ids.sort();
        assert_eq!(ids, vec!["1", "2", "3", "4"]);
    }
}

#[tokio::test]
async fn cursor_cannot_be_reused_for_a_different_account_filter_or_sort() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    mail::store_page(
        &database,
        "a".into(),
        Mailbox::Inbox,
        16,
        None,
        &page(
            (0..3)
                .map(|id| message(&id.to_string(), "Title", "2026-01-01T00:00:00Z"))
                .collect(),
        ),
    )
    .await
    .unwrap();
    let first = mail::query(&database, "a".into(), query(Mailbox::Inbox))
        .await
        .unwrap();
    let mut request = query(Mailbox::Inbox);
    request.after = first.next_cursor;
    assert!(mail::query(&database, "b".into(), request)
        .await
        .unwrap_err()
        .contains("different account or query"));
    let first = mail::query(&database, "a".into(), query(Mailbox::Inbox))
        .await
        .unwrap();
    let mut request = query(Mailbox::Inbox);
    request.after = first.next_cursor;
    request.domain = Some("a.example".into());
    assert!(mail::query(&database, "a".into(), request).await.is_err());
}

#[tokio::test]
async fn filters_and_fts_search_are_scoped_to_the_whole_account_and_mailbox_archive() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    for (account, mailbox) in [
        ("a", Mailbox::Inbox),
        ("a", Mailbox::Sent),
        ("b", Mailbox::Inbox),
    ] {
        mail::store_page(
            &database,
            account.into(),
            mailbox,
            16,
            None,
            &page(vec![message(
                "same-id",
                "Factura café",
                "2026-01-01T00:00:00Z",
            )]),
        )
        .await
        .unwrap();
    }
    let mut request = query(Mailbox::Inbox);
    request.search = Some("factura cafe".into());
    request.domain = Some("B.EXAMPLE".into());
    request.recipient = Some("Recipient <recipient@header.example>".into());
    assert_eq!(
        mail::query(&database, "a".into(), request)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    let mut request = query(Mailbox::Inbox);
    request.domain = Some("remote.example".into());
    assert!(mail::query(&database, "a".into(), request)
        .await
        .unwrap()
        .items
        .is_empty());
    database
        .run(|connection| {
            connection.execute(
                "INSERT INTO read_markers VALUES ('a','inbox','same-id')",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let mut request = query(Mailbox::Inbox);
    request.unread_only = true;
    assert!(mail::query(&database, "a".into(), request)
        .await
        .unwrap()
        .items
        .is_empty());
    let mut request = query(Mailbox::Inbox);
    request.unread_only = true;
    assert_eq!(
        mail::query(&database, "b".into(), request)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    for search in [
        "factura OR unknown",
        "\" OR \"",
        "'; DROP TABLE emails;--",
        "subject:factura*",
    ] {
        let mut request = query(Mailbox::Inbox);
        request.search = Some(search.into());
        assert!(mail::query(&database, "a".into(), request)
            .await
            .unwrap()
            .items
            .is_empty());
    }
}

#[tokio::test]
async fn bodies_survive_metadata_refresh_and_are_searchable_even_if_downloaded_first() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    let detail = json!({"id":"same-id","html":"<p>body</p>"});
    mail::store_detail(
        &database,
        "a".into(),
        Mailbox::Inbox,
        "same-id".into(),
        &detail,
        "unique-body-term".into(),
    )
    .await
    .unwrap();
    let metadata = page(vec![message("same-id", "Original", "2026-01-01T00:00:00Z")]);
    mail::store_page(&database, "a".into(), Mailbox::Inbox, 16, None, &metadata)
        .await
        .unwrap();
    mail::store_page(&database, "a".into(), Mailbox::Inbox, 16, None, &metadata)
        .await
        .unwrap();
    let mut request = query(Mailbox::Inbox);
    request.search = Some("unique-body-term".into());
    assert_eq!(
        mail::query(&database, "a".into(), request)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    let cached = mail::cached_detail(&database, "a".into(), "inbox".into(), "same-id".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&cached).unwrap(), detail);
    assert!(
        mail::cached_detail(&database, "b".into(), "inbox".into(), "same-id".into())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn snapshots_preserve_remote_cursors_and_are_isolated_by_page_size_and_account() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    let remote = email_page(
        vec![message("id", "Title", "2026-01-01T00:00:00Z")],
        true,
        None,
        |item| item["id"].as_str().unwrap(),
    )
    .unwrap();
    mail::store_page(&database, "a".into(), Mailbox::Inbox, 16, None, &remote)
        .await
        .unwrap();
    let cached = mail::cached_page::<Value>(
        &database,
        "a".into(),
        Mailbox::Inbox,
        16,
        None,
        Some("offline".into()),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(cached.has_more);
    assert_eq!(cached.next_cursor.as_deref(), Some("id"));
    assert_eq!(cached.cache.unwrap().error.as_deref(), Some("offline"));
    assert!(
        mail::cached_page::<Value>(&database, "a".into(), Mailbox::Inbox, 100, None, None)
            .await
            .unwrap()
            .is_none()
    );
    assert!(mail::cached_page::<Value>(
        &database,
        "a".into(),
        Mailbox::Inbox,
        16,
        Some("id".into()),
        None
    )
    .await
    .unwrap()
    .is_none());
    assert!(
        mail::cached_page::<Value>(&database, "b".into(), Mailbox::Inbox, 16, None, None)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn failed_page_write_is_atomic_and_missing_remote_messages_do_not_prune_the_copy() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    mail::store_page(
        &database,
        "a".into(),
        Mailbox::Inbox,
        16,
        None,
        &page(vec![message("keep", "Original", "2026-01-01T00:00:00Z")]),
    )
    .await
    .unwrap();
    let bad = page(vec![
        message("new", "Valid", "2026-01-01T00:00:00Z"),
        message("", "Invalid", "2026-01-01T00:00:00Z"),
    ]);
    assert!(
        mail::store_page(&database, "a".into(), Mailbox::Inbox, 16, None, &bad)
            .await
            .is_err()
    );
    mail::store_page(
        &database,
        "a".into(),
        Mailbox::Inbox,
        16,
        None,
        &page(vec![]),
    )
    .await
    .unwrap();
    let local = mail::query(&database, "a".into(), query(Mailbox::Inbox))
        .await
        .unwrap();
    assert_eq!(local.items.len(), 1);
    assert_eq!(local.items[0]["id"], "keep");
}

#[tokio::test]
async fn query_plans_use_the_intended_composite_indexes() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    database.run(|connection| {
        for (column,index) in [("created_at_ms","emails_date"),("subject_sort","emails_subject"),("sender_sort","emails_sender")] {
            let sql=format!("EXPLAIN QUERY PLAN SELECT metadata_json FROM emails WHERE account_id=?1 AND mailbox=?2 ORDER BY {column},email_id LIMIT 10");
            let mut statement=connection.prepare(&sql)?;
            let plans=statement.query_map(params!["a","inbox"],|row|row.get::<_,String>(3))?.collect::<rusqlite::Result<Vec<_>>>()?.join(" ");
            assert!(plans.contains(index),"{plans}"); assert!(!plans.contains("TEMP B-TREE"),"{plans}");
        }
        let mut statement=connection.prepare("EXPLAIN QUERY PLAN SELECT email_id FROM email_domains WHERE account_id=?1 AND mailbox=?2 AND domain=?3")?;
        let plans=statement.query_map(params!["a","inbox","a.example"],|row|row.get::<_,String>(3))?.collect::<rusqlite::Result<Vec<_>>>()?.join(" ");
        assert!(plans.contains("email_domains_filter"),"{plans}");
        Ok(())
    }).await.unwrap();
}

#[tokio::test]
async fn account_cleanup_also_removes_fts_rows_without_leaking_other_accounts() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    for account in ["a", "b"] {
        mail::store_page(
            &database,
            account.into(),
            Mailbox::Inbox,
            16,
            None,
            &page(vec![message("id", "Needle", "2026-01-01T00:00:00Z")]),
        )
        .await
        .unwrap();
    }
    database
        .run(|connection| {
            connection.execute("DELETE FROM account_state WHERE account_id='a'", [])?;
            connection.execute(
                "INSERT INTO email_search(email_search) VALUES ('integrity-check')",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let mut request = query(Mailbox::Inbox);
    request.search = Some("needle".into());
    assert!(mail::query(&database, "a".into(), request)
        .await
        .unwrap()
        .items
        .is_empty());
    let mut request = query(Mailbox::Inbox);
    request.search = Some("needle".into());
    assert_eq!(
        mail::query(&database, "b".into(), request)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}

#[tokio::test]
async fn installed_sdk_history_is_archived_and_traversable_after_restart() {
    let temporary = TemporaryDatabase::new();
    let database = temporary.database();
    let mut expected = Vec::new();
    for mailbox in [Mailbox::Inbox, Mailbox::Sent] {
        let path = if mailbox == Mailbox::Inbox {
            "/emails/receiving"
        } else {
            "/emails"
        };
        let (resend, server, ids) =
            crate::infrastructure::pagination_test_server::mock_history(path);
        let mut after = None;
        for _ in 0..4 {
            let next = match mailbox {
                Mailbox::Inbox => {
                    let page =
                        crate::inbound::fetch_inbound_page(&resend, Some(100), after.as_deref())
                            .await
                            .unwrap();
                    mail::store_page(&database, "a".into(), mailbox, 100, after.clone(), &page)
                        .await
                        .unwrap();
                    page.next_cursor
                }
                Mailbox::Sent => {
                    let page = crate::sent::fetch_sent_page(&resend, Some(100), after.as_deref())
                        .await
                        .unwrap();
                    mail::store_page(&database, "a".into(), mailbox, 100, after.clone(), &page)
                        .await
                        .unwrap();
                    page.next_cursor
                }
            };
            after = next;
        }
        server.join().unwrap();
        expected = ids;
    }
    drop(database);
    let database = temporary.database();
    expected.reverse();
    for mailbox in [Mailbox::Inbox, Mailbox::Sent] {
        let mut actual = Vec::new();
        let mut cursor = None;
        loop {
            let mut request = query(mailbox);
            request.limit = Some(100);
            request.after = cursor;
            if mailbox == Mailbox::Inbox {
                request.domain = Some("b.example".into());
            }
            let result = mail::query(&database, "a".into(), request).await.unwrap();
            actual.extend(
                result
                    .items
                    .iter()
                    .map(|item| item["id"].as_str().unwrap().to_string()),
            );
            if !result.has_more {
                break;
            }
            cursor = result.next_cursor;
        }
        assert_eq!(actual, expected);
    }
}
