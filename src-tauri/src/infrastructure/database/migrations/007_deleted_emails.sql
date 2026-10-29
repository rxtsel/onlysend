-- Local user state, deliberately independent of downloaded email rows.
CREATE TABLE deleted_emails (
    account_id TEXT NOT NULL REFERENCES account_state(account_id) ON DELETE CASCADE,
    mailbox TEXT NOT NULL CHECK (mailbox IN ('inbox', 'sent')),
    email_id TEXT NOT NULL,
    PRIMARY KEY (account_id, mailbox, email_id)
);
