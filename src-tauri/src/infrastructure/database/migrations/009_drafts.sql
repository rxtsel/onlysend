CREATE TABLE drafts (
    account_id TEXT NOT NULL REFERENCES account_state(account_id) ON DELETE CASCADE,
    draft_id TEXT NOT NULL,
    data_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (account_id, draft_id)
);
CREATE INDEX drafts_updated ON drafts(account_id, updated_at DESC);
