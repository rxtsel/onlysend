# Local SQLite persistence

SQLite is a Rust-owned local read model, not Resend's source of truth. This first
increment adds infrastructure only; existing JSON commands still own settings,
domain selections and read markers. No JSON data is imported or removed yet.

## Distribution and lifecycle

- `rusqlite` uses `bundled` SQLite for consistent desktop builds without a system
  SQLite dependency. The engine version is locked in `src-tauri/Cargo.lock`.
- Tauri registers one `Database` in managed state, at
  `app_data_dir()/onlysend.sqlite3`. Clones share the same connection and gate.
- Setup starts initialization in the background. A failure is non-fatal to the
  existing JSON flows; the next database operation can retry initialization.
- An async semaphore admits one operation into `spawn_blocking`. Filesystem I/O,
  connection initialization, migrations and queries run there, not on the async
  executor. The mutex protects the connection, not an async task waiting on I/O.
- A started blocking operation is not cancelled if its caller is dropped. Its
  permit stays with the worker until completion; callers must not interpret an
  abandoned response as a rolled-back write.
- The managed state owns the connection for the app lifetime. No detached worker
  loop, connection pool or additional frontend cache is introduced.

## Connection and migrations

Each connection sets a five-second busy timeout, enables foreign keys, and uses
WAL with `synchronous=FULL` because preferences and read state are user data.
SQLite files, including WAL/SHM sidecars, are local application data and must not
be committed. SQLite does not encrypt them.

Ordered SQL migrations live in `src-tauri/src/infrastructure/database/migrations`.
`PRAGMA user_version` tracks the applied version. A single `IMMEDIATE` transaction
locks before reading the version and applies all pending migrations and version
updates atomically. Failure rolls back schema/version changes. A schema newer than
the app supports is rejected, never downgraded or recreated.

## First schema

The schema reflects existing local queries, not hypothetical email synchronization:

- `account_state`: ownership root keyed by the existing account UUID; no credentials
  or duplicate account registry metadata.
- `domain_preferences`: a row distinguishes a saved empty selection from an
  account with no saved selection.
- `included_domains`: composite account/domain key. No remote-domain foreign key,
  so inaccessible or deleted IDs survive for review.
- `read_markers`: composite account/mailbox/email key, with explicit `inbox` and
  `sent` mailboxes. No legacy 200-marker truncation.

Deleting an ownership root cascades only that account's local rows. This is a
schema property, not yet a change to logout: no production rows are written in
this increment. Future typed adapters must validate explicit account IDs against
`auth.json` and capture ownership before asynchronous work. They must never resolve
ownership from a global active-account pointer.

## Next increment

Before switching preferences/read markers to SQLite, implement an idempotent JSON
import with backup and verification. Preserve UUIDs and every available read
marker, distinguish absent preferences from saved empty ones, and leave global
legacy values alone when ownership cannot be established. Define disconnect data
cleanup explicitly. Only then migrate commands without changing their frontend
contracts.

Email metadata, bodies, synchronization state, drafts, FTS and attachment binaries
are outside this increment. API keys and OAuth tokens remain outside SQLite.

## Validation

`cargo test --manifest-path src-tauri/Cargo.toml --lib` exercises real temporary
SQLite databases: initialization, pragmas, restart persistence, account/mailbox
isolation, empty selections, more than 200 markers, foreign keys/cascades, atomic
migration failure, unsupported newer schemas, initialization retry, concurrent
writes and cancellation while a blocking transaction is in flight.
