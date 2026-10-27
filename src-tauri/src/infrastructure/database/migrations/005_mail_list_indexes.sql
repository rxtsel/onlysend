-- Cover the navigation catalog and latest-snapshot lookup without table scans.
CREATE INDEX email_domains_navigation ON email_domains(account_id, domain);
CREATE INDEX remote_pages_downloaded ON remote_pages(account_id, mailbox, downloaded_at DESC);
