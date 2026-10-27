-- Only the newest timestamp and its IDs are needed to deduplicate across restarts.
CREATE TABLE inbox_notification_state (
    account_id TEXT PRIMARY KEY NOT NULL REFERENCES account_state(account_id) ON DELETE CASCADE,
    checkpoint_json TEXT NOT NULL
);
