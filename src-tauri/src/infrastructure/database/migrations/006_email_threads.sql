CREATE TABLE email_threads (
    account_id TEXT NOT NULL REFERENCES account_state(account_id) ON DELETE CASCADE,
    mailbox TEXT NOT NULL CHECK (mailbox IN ('inbox', 'sent')),
    email_id TEXT NOT NULL,
    thread_id TEXT NOT NULL,
    PRIMARY KEY (account_id, mailbox, email_id)
);
CREATE INDEX email_threads_lookup ON email_threads(account_id, thread_id, mailbox, email_id);
