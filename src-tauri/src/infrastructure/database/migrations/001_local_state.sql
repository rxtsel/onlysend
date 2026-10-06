-- Local ownership only. Credentials and the account registry remain in auth.json.
CREATE TABLE account_state (
    account_id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(account_id)) > 0)
);

-- A row distinguishes a saved empty selection from an account not configured yet.
CREATE TABLE domain_preferences (
    account_id TEXT PRIMARY KEY NOT NULL
        REFERENCES account_state(account_id) ON DELETE CASCADE
);

-- No remote-domain FK: inaccessible/deleted IDs must survive for user review.
CREATE TABLE included_domains (
    account_id TEXT NOT NULL REFERENCES domain_preferences(account_id) ON DELETE CASCADE,
    domain_id TEXT NOT NULL CHECK (length(trim(domain_id)) > 0),
    PRIMARY KEY (account_id, domain_id)
);

-- Remote message IDs are not globally unique, nor shared between mailboxes.
-- No old 200-item cap: importing JSON must not silently discard read state.
CREATE TABLE read_markers (
    account_id TEXT NOT NULL REFERENCES account_state(account_id) ON DELETE CASCADE,
    mailbox TEXT NOT NULL CHECK (mailbox IN ('inbox', 'sent')),
    email_id TEXT NOT NULL CHECK (length(trim(email_id)) > 0),
    PRIMARY KEY (account_id, mailbox, email_id)
);
