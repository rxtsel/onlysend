# Local mail archive

OnlySend keeps a persistent local copy using Rust-owned SQLite. This is **not a
POP connection**: Resend's HTTP API supplies the messages, and downloads never
delete messages or change DNS remotely. The local archive retains downloaded
messages even if a later remote list no longer contains them.

## What uses SQLite

- Domain inclusion preferences and Inbox read markers now use SQLite.
- Inbox/Sent list responses are saved transactionally as indexed metadata.
- Message details and attachment metadata are saved separately. Bodies already
  downloaded can be opened without contacting Resend again.
- The sidebar reads and paginates the local archive, showing disk data before
  revalidating the remote head. Its existing domain buttons still explicitly
  filter loaded rows; the SQL query API supports archive-wide filters/search for
  the future toolbar.
- Credentials remain in `auth.json`. Onboarding, sender identities and remaining
  settings still use JSON; neither credentials nor global legacy settings are
  imported into SQLite.

## Distribution, ownership and lifecycle

`rusqlite` bundles SQLite; versions are locked in `src-tauri/Cargo.lock`. One
Tauri-managed `Database` lives at `app_data_dir()/onlysend.sqlite3`; its clones
share the connection and async admission semaphore. Connection initialization,
queries, migrations and filesystem work run inside `spawn_blocking`. A started
write keeps its permit even if its caller cancels the response.

Connections use WAL, foreign keys, a five-second busy timeout and
`synchronous=FULL`. On Unix the database, existing WAL/SHM sidecars and migration
backup files are protected with mode `0600`. SQLite does not encrypt email data;
Windows relies on the application-data directory's OS permissions. Do not share
or commit database files, sidecars or backups.

Every public local command validates its explicit account UUID against the
credential registry. There is no active-account fallback. Late downloads remain
owned by the captured account, and removal of credentials stops subsequent
background requests. Logout does not silently erase downloaded mail or migration
backups; retained data becomes inaccessible through account commands until an
explicit archive-management policy/UI is implemented. A new connection creates a
new UUID and does not automatically adopt an old archive.

## Migrations and JSON import

Ordered SQL migrations use `PRAGMA user_version`. A single `IMMEDIATE` transaction
locks before reading the version and applies all pending migrations and version
updates atomically. Failure rolls back; newer schemas are rejected rather than
downgraded or recreated.

On first use of each registered account, OnlySend imports **only** its existing
`domain_preferences:<accountId>` and `read_inbound_ids:<accountId>` values. Before
changing SQL state it writes, fsyncs and rereads a minimal JSON snapshot under
`migration-backups/`. Imported sets are verified within the transaction before
recording completion in `json_imports`.

Original JSON is retained. Failed imports retain their backups and roll back;
repeated imports do not overwrite newer SQLite edits. A missing preference is
not a saved empty selection; inaccessible domain IDs survive. All available read
markers are imported without the old 200-item cap. Already lost/truncated JSON
markers cannot be reconstructed. Unscoped legacy values are never attributed to
an arbitrary account.

## Schema and indexes

Migration 1 owns local account state, domain selections and read markers.
Migration 2 tracks verified JSON imports. Migration 3 adds the archive:

| Table | Ownership / purpose |
| --- | --- |
| `account_state` | Existing account UUID, no secrets or duplicate registry |
| `domain_preferences`, `included_domains` | Saved-empty distinction; local domain IDs |
| `read_markers` | `(account_id, mailbox, email_id)` primary key |
| `json_imports` | Account-scoped backup reference and completion timestamp |
| `emails` | `(account_id, mailbox, email_id)`; indexed metadata and DTO projection |
| `email_domains` | Envelope-domain associations, including multi-recipient messages |
| `email_recipients` | Normalized addresses with `to`/`cc`/`bcc` roles |
| `email_bodies` | Details, search text, download timestamp; no binary attachments |
| `remote_pages` | Exact interactive page snapshots, not local query cursors |
| `mailbox_sync` | Separate metadata/body completion, failures and scan timestamps |
| `email_search` | FTS5 external-content index maintained by insert/update/delete triggers |

Composite indexes begin with account and mailbox. Date ordering uses numeric UTC
milliseconds plus email ID as a deterministic tie-breaker, not ISO text or
`OFFSET`. Subject/sender indexes use stored lowercase sort keys, not a
locale-specific alphabet. Reverse index scans support opposite sort directions.
Domain and recipient indexes support scoped association filters. Read/unread
lookups use the existing marker primary key.

`query_local_mail` exposes newest/oldest, subject/sender ascending/descending,
domain, recipient, unread and full-text filters with limits 1–100. Its opaque
keyset cursor is bound to account, mailbox, sort and normalized filter/search
criteria. Resend's `after` IDs are a separate contract and never receive a local
cursor. SQL values are bound; sort SQL comes only from enums. FTS input is escaped
literal AND terms, not user-supplied SQL or FTS operators. FTS searches always
restrict account/mailbox, including when matching body text.

Bodies are not overwritten by metadata refreshes. A full scan updates canonical
rows and only the head snapshot, avoiding a new duplicate set of page snapshots
on every shifted head. Removing local account rows cascades through associations
and FTS triggers, but production logout intentionally does not call that purge.

## Download behavior and coverage

Opening a mailbox revalidates its head and starts a background scan if one is not
already running or within the five-minute backoff. Manual Refresh can retry
sooner. The worker follows authoritative `has_more`/`after` cursors, detects
cycles, downloads metadata first, then copies missing bodies sequentially. A
shared background request clock spaces requests across accounts/mailboxes by at
least 650 ms; remote page/body work is bounded by 30-second deadlines. Interactive
API calls and existing DNS polling are separate, so upstream rate-limit errors
still stop the scan and surface for retry rather than being silently ignored.

A window-wide bottom status bar shows download progress or errors at the left.
When the local copy is healthy it collapses to one icon; its hover/focus tooltip
shows metadata/body counts, mailbox and last completion. Status remains scoped to
the visible account/mailbox, without adding another polling loop. A body failure does not erase metadata or falsely mark bodies complete.
`query_local_mail.isPartial` concerns metadata coverage; body coverage is separate.
Restarted/interrupted scans retain partial copies and retry without duplicating
IDs. No timestamp-based Resend delta or atomic remote snapshot is assumed: a
completed traversal describes that scan, not proof that the remote mailbox can
never change afterwards.

Downloaded metadata and bodies remain usable offline. Never-downloaded content
still requires the API. Remote errors remain visible alongside the local copy;
a new empty schema is not presented as known empty history. Attachment binaries,
embedded images and externally referenced resources are **not** downloaded.
Incoming cached HTML is sanitized again when read; sent HTML is rendered in a
fully sandboxed iframe rather than injected into the application DOM.

## Validation and remaining acceptance

Rust tests cover real temporary database files, migration from version 1, JSON
backup/import/retry isolation, more than 200 markers, atomic replacement, numeric
timezone sorting, keysets, query-bound cursors, scoped filters/FTS, body retention,
FK/FTS cleanup and index selection with `EXPLAIN QUERY PLAN`. Installed SDK HTTP
fixtures download 321 Inbox and 321 Sent messages, archive them, reopen the DB and
traverse filtered local pages independently of remote order.

Frontend tests cover disk hydration, stale account responses, failed refresh
indicators, disk-only background refresh and separation of local/remote cursors.
Real Tauri acceptance of background body downloads, restart/offline behavior and
restricted credentials is still required; fixtures do not certify live Resend
permissions or remote retention behavior. Export/purge UI, attachment downloads,
retention controls and the full search/filter toolbar remain separate work.
