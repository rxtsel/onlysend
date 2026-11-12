-- Older locally sent replies stored bracketed Message-IDs as conversation keys.
UPDATE email_threads
SET thread_id = substr(thread_id, 2, length(thread_id) - 2)
WHERE mailbox = 'sent' AND thread_id LIKE '<%>';
