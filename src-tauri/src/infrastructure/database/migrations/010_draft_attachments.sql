CREATE TABLE draft_attachments (
    account_id TEXT NOT NULL,
    draft_id TEXT NOT NULL,
    data_json TEXT NOT NULL,
    PRIMARY KEY (account_id, draft_id),
    FOREIGN KEY (account_id, draft_id) REFERENCES drafts(account_id, draft_id) ON DELETE CASCADE
);

INSERT INTO draft_attachments(account_id, draft_id, data_json)
SELECT account_id, draft_id, COALESCE(json_extract(data_json, '$.attachments'), '[]') FROM drafts;

UPDATE drafts SET data_json = json_remove(data_json, '$.attachments');
