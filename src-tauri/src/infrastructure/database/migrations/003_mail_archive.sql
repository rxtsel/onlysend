CREATE TABLE emails (
    account_id TEXT NOT NULL REFERENCES account_state(account_id) ON DELETE CASCADE,
    mailbox TEXT NOT NULL CHECK (mailbox IN ('inbox', 'sent')),
    email_id TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    subject TEXT NOT NULL,
    sender TEXT NOT NULL,
    subject_sort TEXT NOT NULL,
    sender_sort TEXT NOT NULL,
    recipient_text TEXT NOT NULL,
    search_body TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL CHECK (json_valid(metadata_json)),
    downloaded_at INTEGER NOT NULL,
    PRIMARY KEY (account_id, mailbox, email_id)
);
-- Keyset ordering: reverse index scans support the opposite direction too.
CREATE INDEX emails_date ON emails(account_id, mailbox, created_at_ms DESC, email_id DESC);
CREATE INDEX emails_subject ON emails(account_id, mailbox, subject_sort, email_id);
CREATE INDEX emails_sender ON emails(account_id, mailbox, sender_sort, email_id);

CREATE TABLE email_domains (
    account_id TEXT NOT NULL,
    mailbox TEXT NOT NULL,
    email_id TEXT NOT NULL,
    domain TEXT NOT NULL,
    PRIMARY KEY (account_id, mailbox, email_id, domain),
    FOREIGN KEY (account_id, mailbox, email_id) REFERENCES emails(account_id, mailbox, email_id) ON DELETE CASCADE
);
CREATE INDEX email_domains_filter ON email_domains(account_id, mailbox, domain, email_id);

CREATE TABLE email_recipients (
    account_id TEXT NOT NULL,
    mailbox TEXT NOT NULL,
    email_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('to', 'cc', 'bcc')),
    address TEXT NOT NULL,
    PRIMARY KEY (account_id, mailbox, email_id, role, address),
    FOREIGN KEY (account_id, mailbox, email_id) REFERENCES emails(account_id, mailbox, email_id) ON DELETE CASCADE
);
CREATE INDEX email_recipients_filter ON email_recipients(account_id, mailbox, address, email_id);

-- Details may be opened before a list page has been downloaded. Ownership is
-- still enforced, without inventing envelope domains from header recipients.
CREATE TABLE email_bodies (
    account_id TEXT NOT NULL REFERENCES account_state(account_id) ON DELETE CASCADE,
    mailbox TEXT NOT NULL CHECK (mailbox IN ('inbox', 'sent')),
    email_id TEXT NOT NULL,
    detail_json TEXT NOT NULL CHECK (json_valid(detail_json)),
    search_text TEXT NOT NULL,
    downloaded_at INTEGER NOT NULL,
    PRIMARY KEY (account_id, mailbox, email_id)
);

-- Exact remote-page snapshots keep Resend cursors separate from local keysets.
CREATE TABLE remote_pages (
    account_id TEXT NOT NULL REFERENCES account_state(account_id) ON DELETE CASCADE,
    mailbox TEXT NOT NULL CHECK (mailbox IN ('inbox', 'sent')),
    page_size INTEGER NOT NULL CHECK (page_size BETWEEN 1 AND 100),
    after_cursor TEXT NOT NULL,
    page_json TEXT NOT NULL CHECK (json_valid(page_json)),
    downloaded_at INTEGER NOT NULL,
    PRIMARY KEY (account_id, mailbox, page_size, after_cursor)
);

CREATE TABLE mailbox_sync (
    account_id TEXT NOT NULL REFERENCES account_state(account_id) ON DELETE CASCADE,
    mailbox TEXT NOT NULL CHECK (mailbox IN ('inbox', 'sent')),
    started_at INTEGER NOT NULL,
    metadata_completed_at INTEGER,
    completed_at INTEGER,
    last_error TEXT,
    PRIMARY KEY (account_id, mailbox)
);

-- External-content FTS avoids a second copy of message text. All search queries
-- MUST also restrict emails.account_id and mailbox, including rowid matches.
CREATE VIRTUAL TABLE email_search USING fts5(
    subject, sender, recipient_text, search_body,
    content='emails', content_rowid='rowid', tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER emails_search_insert AFTER INSERT ON emails BEGIN
    INSERT INTO email_search(rowid, subject, sender, recipient_text, search_body)
    VALUES (new.rowid, new.subject, new.sender, new.recipient_text, new.search_body);
END;
CREATE TRIGGER emails_search_delete AFTER DELETE ON emails BEGIN
    INSERT INTO email_search(email_search, rowid, subject, sender, recipient_text, search_body)
    VALUES ('delete', old.rowid, old.subject, old.sender, old.recipient_text, old.search_body);
END;
CREATE TRIGGER emails_search_update AFTER UPDATE ON emails BEGIN
    INSERT INTO email_search(email_search, rowid, subject, sender, recipient_text, search_body)
    VALUES ('delete', old.rowid, old.subject, old.sender, old.recipient_text, old.search_body);
    INSERT INTO email_search(rowid, subject, sender, recipient_text, search_body)
    VALUES (new.rowid, new.subject, new.sender, new.recipient_text, new.search_body);
END;
