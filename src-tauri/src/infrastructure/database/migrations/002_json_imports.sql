-- Only account-scoped preference/read-state snapshots are imported; never auth.
CREATE TABLE json_imports (
    account_id TEXT PRIMARY KEY NOT NULL REFERENCES account_state(account_id) ON DELETE CASCADE,
    backup_name TEXT NOT NULL,
    imported_at INTEGER NOT NULL
);
