use crate::oauth;
use crate::permissions;
use base64::{engine::general_purpose, Engine as _};
use resend_rs::types::{CreateAttachment, CreateEmailBaseOptions};
use resend_rs::Resend;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Wry};

#[derive(Debug, Deserialize)]
pub struct AttachmentPayload {
    pub filename: String,
    pub content: String, // base64
}

#[derive(Debug, Deserialize)]
pub struct EmailComposerData {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,

    #[serde(default)]
    pub cc: Vec<String>,

    #[serde(default)]
    pub bcc: Vec<String>,

    #[serde(rename = "replyTo")]
    pub reply_to: Option<String>,

    pub content: String,

    #[serde(rename = "scheduledAt")]
    pub scheduled_at: Option<String>,

    #[serde(rename = "replyEmailId")]
    pub reply_email_id: Option<String>,

    #[serde(default)]
    pub attachments: Vec<AttachmentPayload>,
}

pub(crate) fn attachment_bytes(attachment: &AttachmentPayload, total: &mut u64) -> Result<Vec<u8>, String> {
    let limit = crate::attachments::MAX_ATTACHMENT_BYTES;
    if attachment.filename.is_empty()
        || attachment
            .filename
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '/' | '\\'))
    {
        return Err("Invalid attachment filename".into());
    }
    if attachment.content.len() as u64 > (limit + 2) / 3 * 4 {
        return Err("Attachments must total no more than 25 MB".into());
    }
    let bytes = general_purpose::STANDARD
        .decode(&attachment.content)
        .map_err(|_| "Invalid attachment data")?;
    *total += bytes.len() as u64;
    if *total > limit {
        return Err("Attachments must total no more than 25 MB".into());
    }
    Ok(bytes)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyContext {
    pub to: Vec<String>,
    pub subject: String,
    pub from_candidates: Vec<String>,
    pub reply_to: Option<String>,
    #[serde(skip)]
    message_id: String,
    #[serde(skip)]
    references: String,
    #[serde(skip)]
    thread_id: String,
}

fn address(value: &str) -> Result<String, String> {
    if value.len() > 16 * 1024 || value.chars().any(char::is_control) {
        return Err("Invalid email address".into());
    }
    mailparse::addrparse(value)
        .ok()
        .and_then(|list| list.extract_single_info())
        .filter(|mailbox| mailbox.addr.contains('@'))
        .map(|mailbox| mailbox.addr)
        .ok_or_else(|| "Invalid email address".into())
}

fn message_ids(value: &str) -> Result<Vec<String>, String> {
    if value.len() > 16 * 1024 {
        return Err("Message-ID header is too large".into());
    }
    let ids = match serde_json::from_str::<Vec<String>>(value) {
        Ok(values) => values
            .into_iter()
            .map(|value| {
                let parsed =
                    mailparse::msgidparse(&value).map_err(|_| "Invalid Message-ID header")?;
                if parsed.len() != 1 {
                    return Err("Invalid Message-ID header".into());
                }
                Ok(parsed[0].clone())
            })
            .collect::<Result<Vec<_>, String>>()?,
        Err(_) => mailparse::msgidparse(value)
            .map_err(|_| "Invalid Message-ID header")?
            .iter()
            .cloned()
            .collect(),
    };
    if ids.is_empty()
        || ids.iter().any(|id| {
            id.is_empty()
                || id
                    .chars()
                    .any(|ch| ch.is_control() || ch.is_whitespace() || matches!(ch, '<' | '>'))
        })
    {
        return Err("Invalid Message-ID header".into());
    }
    Ok(ids.iter().map(|id| format!("<{id}>")).collect())
}

fn single_message_id(value: &str) -> Result<String, String> {
    let ids = message_ids(value)?;
    if ids.len() != 1 {
        return Err("A single Message-ID is required".into());
    }
    Ok(ids[0].clone())
}

fn reply_context(original: &crate::inbound::InboundEmailDetailDto) -> Result<ReplyContext, String> {
    let message_id = single_message_id(
        original
            .message_id
            .as_deref()
            .ok_or("Reply details are not downloaded. Connect to Resend and try again.")?,
    )?;
    let recipients: &[String] = if original.reply_to.is_empty() {
        std::slice::from_ref(&original.from)
    } else {
        &original.reply_to
    };
    if recipients.len() > 50 {
        return Err("Reply-To contains too many recipients".into());
    }
    let mut to = Vec::new();
    for recipient in recipients {
        let recipient = address(recipient)?;
        if !to
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&recipient))
        {
            to.push(recipient);
        }
    }
    // Malformed ancestry must not prevent replying to a valid parent.
    let mut references =
        message_ids(original.references.as_deref().unwrap_or_default()).unwrap_or_default();
    if references.is_empty() {
        references = message_ids(original.in_reply_to.as_deref().unwrap_or_default()).unwrap_or_default();
    }
    references.retain(|id| id != &message_id);
    references.push(message_id.clone());
    let thread_id = crate::inbound::thread_id(original)
        .unwrap_or_else(|| message_id.trim_matches(['<', '>']).to_owned());
    let normalized = original
        .subject
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut subject = normalized.as_str();
    while subject
        .get(..3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("re:"))
    {
        subject = subject[3..].trim_start();
    }
    let subject = format!(
        "Re: {}",
        if subject.is_empty() {
            "(No subject)"
        } else {
            subject
        }
    );
    let from_candidates = original
        .to
        .iter()
        .take(50)
        .filter_map(|recipient| address(recipient).ok())
        .collect();
    Ok(ReplyContext {
        to,
        subject,
        from_candidates,
        reply_to: None,
        message_id,
        references: references.join(" "),
        thread_id,
    })
}

fn inherit_reply_settings(context: &mut ReplyContext, messages: Vec<(String, String)>) -> Result<bool, String> {
    for (mailbox, json) in messages.into_iter().rev() {
        if mailbox != "sent" { continue; }
        let sent: crate::sent::SentEmail = serde_json::from_str(&json)
            .map_err(|_| "Could not read the conversation's sent message")?;
        if matches!(sent.last_event.as_deref(), Some("scheduled" | "canceled" | "failed" | "suppressed" | "bounced")) { continue; }
        context.reply_to = sent.reply_to.as_deref().and_then(|value| address(value).ok());
        // An absent Reply-To on the latest sent message is an explicit opt-out.
        return Ok(true);
    }
    Ok(false)
}

async fn load_reply_settings<F, Fut>(
    database: &crate::infrastructure::database::Database,
    account_id: String,
    context: &mut ReplyContext,
    mut download: F,
) -> Result<(), String>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<crate::sent::SentEmail, String>>,
{
    use crate::infrastructure::database::mail::{cached_thread, pending_sent_header_ids, Mailbox};
    let messages = cached_thread(database, account_id.clone(), context.thread_id.clone(), Mailbox::Sent).await?;
    if inherit_reply_settings(context, messages)? { return Ok(()); }
    let pending = pending_sent_header_ids(database, account_id.clone()).await?;
    if pending.is_empty() { return Ok(()); }
    for id in pending {
        let sent = download(id).await?;
        if sent.thread_id.as_deref() == Some(context.thread_id.as_str()) { break; }
    }
    let messages = cached_thread(database, account_id, context.thread_id.clone(), Mailbox::Sent).await?;
    inherit_reply_settings(context, messages)?;
    Ok(())
}

#[tauri::command]
pub async fn prepare_reply(
    app: AppHandle<Wry>,
    account_id: String,
    email_id: String,
) -> Result<ReplyContext, String> {
    let original =
        crate::inbound::get_inbound_email(app.clone(), account_id.clone(), email_id, true).await?;
    let database = crate::infrastructure::database::local_state::for_account(&app, &account_id).await?;
    let mut context = reply_context(&original)?;
    load_reply_settings(&database, account_id.clone(), &mut context, |id| {
        let app = app.clone();
        let account_id = account_id.clone();
        async move {
            crate::infrastructure::database::sync::network_turn().await;
            crate::sent::download_sent_email(app, account_id, id, true).await
        }
    }).await?;
    crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
    Ok(context)
}

fn compose_email(
    data: &EmailComposerData,
    reply: Option<&ReplyContext>,
) -> Result<CreateEmailBaseOptions, String> {
    if data.from.trim().is_empty() || data.from.chars().any(char::is_control) {
        return Err("Invalid sender".into());
    }
    if data.to.is_empty() {
        return Err("At least one recipient is required".into());
    }
    for recipient in data.to.iter().chain(&data.cc).chain(&data.bcc) {
        address(recipient)?;
    }
    if data.subject.chars().any(char::is_control) {
        return Err("Invalid subject".into());
    }
    let subject = reply.map_or(data.subject.as_str(), |reply| &reply.subject);
    let mut email = CreateEmailBaseOptions::new(data.from.as_str(), data.to.clone(), subject)
        .with_html(data.content.as_str());
    if let Some(scheduled_at) = &data.scheduled_at {
        let scheduled = chrono::DateTime::parse_from_rfc3339(scheduled_at)
            .map_err(|_| "Invalid scheduled time")?;
        let now: chrono::DateTime<chrono::Utc> = std::time::SystemTime::now().into();
        let delay = scheduled.signed_duration_since(now);
        if delay <= chrono::Duration::zero() || delay > chrono::Duration::days(30) {
            return Err("Choose a future time within the next 30 days".into());
        }
        email = email.with_scheduled_at(scheduled_at);
    }
    for cc in &data.cc {
        email = email.with_cc(cc.as_str());
    }
    for bcc in &data.bcc {
        email = email.with_bcc(bcc.as_str());
    }
    if let Some(reply_to) = &data.reply_to {
        email = email.with_reply(&address(reply_to)?);
    }
    if let Some(reply) = reply {
        email = email
            .with_header("In-Reply-To", &reply.message_id)
            .with_header("References", &reply.references);
    }
    let mut total = 0;
    for attachment in &data.attachments {
        let bytes = attachment_bytes(attachment, &mut total)?;
        email = email.with_attachment(
            CreateAttachment::from_content(bytes).with_filename(&attachment.filename),
        );
    }
    Ok(email)
}

#[tauri::command]
pub async fn send_email(
    app: AppHandle<Wry>,
    account_id: String,
    data: EmailComposerData,
) -> Result<(), String> {
    let reply = if let Some(email_id) = &data.reply_email_id {
        let original = crate::inbound::get_inbound_email(
            app.clone(),
            account_id.clone(),
            email_id.clone(),
            true,
        )
        .await?;
        Some(reply_context(&original)?)
    } else {
        None
    };
    let email = compose_email(&data, reply.as_ref())?;
    let credential = oauth::get_account_credential(&app, &account_id).await?;
    crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
    let resend = Resend::new(&credential);
    let database =
        crate::infrastructure::database::local_state::for_account(&app, &account_id).await?;
    let sent =
        resend.emails.send(email).await.map_err(|e| {
            permissions::friendly_error(format!("[ERROR] Error sending email: {e}"))
        })?;
    let now: chrono::DateTime<chrono::Utc> = std::time::SystemTime::now().into();
    let outgoing = crate::sent::SentEmail {
        id: sent.id.to_string(),
        from: data.from,
        to: data.to,
        subject: reply.as_ref().map_or(data.subject, |reply| reply.subject.clone()),
        html: Some(data.content),
        text: None,
        body_format_version: 0,
        attachments: Vec::new(),
        created_at: now.to_rfc3339(),
        cc: data.cc,
        bcc: data.bcc,
        reply_to: data.reply_to,
        last_event: Some(if data.scheduled_at.is_some() { "scheduled" } else { "sent" }.into()),
        thread_id: reply.map(|reply| reply.thread_id),
        message_id: None,
    };
    if crate::infrastructure::database::mail::store_outgoing_sent(&database, account_id.clone(), &outgoing).await.is_err() {
        // The remote send succeeded; returning an error here could cause a duplicate send.
        eprintln!("[WARN] Email sent but could not save its local copy and reply settings");
    }
    // The first outgoing message needs Resend's Message-ID to identify future replies.
    tauri::async_runtime::spawn(async move {
        crate::infrastructure::database::sync::network_turn().await;
        if crate::sent::download_sent_email(app, account_id, outgoing.id, false).await.is_err() {
            eprintln!("[WARN] Could not refresh sent message headers; the archive sync will retry");
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn original(subject: &str) -> crate::inbound::InboundEmailDetailDto {
        serde_json::from_value(serde_json::json!({
            "id":"b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1", "from":"\"Sender, Inc.\" <sender@example.com>",
            "to":["support@example.com"], "subject":subject, "createdAt":"2026-01-01T00:00:00Z",
            "html":null,"text":"Original body", "attachments":[], "messageId":"<parent@example.com>",
            "references":"<root@example.com>\r\n\t<parent@example.com>"
        })).unwrap()
    }

    #[test]
    fn scheduled_email_validates_time_and_serializes_for_resend() {
        let mut data = draft();
        let now: chrono::DateTime<chrono::Utc> = std::time::SystemTime::now().into();
        for value in ["invalid".to_string(), (now - chrono::Duration::minutes(1)).to_rfc3339(), (now + chrono::Duration::days(31)).to_rfc3339()] {
            data.scheduled_at = Some(value);
            assert!(compose_email(&data, None).is_err());
        }
        let scheduled = (now + chrono::Duration::hours(1)).to_rfc3339();
        data.scheduled_at = Some(scheduled.clone());
        let payload = serde_json::to_value(compose_email(&data, None).unwrap()).unwrap();
        assert_eq!(payload["scheduled_at"], scheduled);
        data.scheduled_at = None;
        let payload = serde_json::to_value(compose_email(&data, None).unwrap()).unwrap();
        assert!(payload.get("scheduled_at").is_none_or(serde_json::Value::is_null));
    }

    fn draft() -> EmailComposerData {
        serde_json::from_value(serde_json::json!({
            "from":"Support <support@example.com>", "to":["sender@example.com"], "subject":"Untrusted subject",
            "content":"<p>My reply</p>", "cc":["copy@example.com"], "bcc":["hidden@example.com"],
            "messageId":"<ignored@example.com>", "attachments":[{"filename":"reply.txt", "content":"eA=="}]
        })).unwrap()
    }

    fn sent_message(id: &str, reply_to: Option<&str>, date: &str, event: &str) -> crate::sent::SentEmail {
        serde_json::from_value(serde_json::json!({
            "id":id, "from":"Me <c@mail.rxtsel.dev>", "to":["sender@example.com"], "subject":"Re: Invoice",
            "html":"<p>My reply</p>", "createdAt":date, "replyTo":reply_to, "lastEvent":event,
            "threadId":"root@example.com", "bodyFormatVersion":0
        })).unwrap()
    }

    #[test]
    fn only_our_latest_sent_reply_to_is_inherited_and_an_explicit_opt_out_sticks() {
        let mut incoming = original("Invoice");
        incoming.reply_to = vec!["external@example.com".into()];
        let mut context = reply_context(&incoming).unwrap();
        assert_eq!(context.thread_id, crate::inbound::thread_id(&incoming).unwrap());
        let parent = sent_message("root", Some("c@rxtsel.dev"), "2026-01-01T00:00:00Z", "sent");
        let json = serde_json::to_string(&parent).unwrap();
        inherit_reply_settings(&mut context, vec![("inbox".into(), json.clone())]).unwrap();
        assert!(context.reply_to.is_none());
        inherit_reply_settings(&mut context, vec![("sent".into(), json.clone())]).unwrap();
        assert_eq!(context.reply_to.as_deref(), Some("c@rxtsel.dev"));
        assert_eq!(context.to, ["external@example.com"]);
        assert_eq!(context.from_candidates, ["support@example.com"]);
        for event in ["sent", "delivered", "scheduled", "canceled", "failed", "suppressed", "bounced"] {
            let last = sent_message("reply", None, "2026-01-02T00:00:00Z", event);
            inherit_reply_settings(&mut context, vec![("sent".into(), json.clone()), ("sent".into(), serde_json::to_string(&last).unwrap())]).unwrap();
            if matches!(event, "sent" | "delivered") { assert!(context.reply_to.is_none()); }
            else { assert_eq!(context.reply_to.as_deref(), Some("c@rxtsel.dev")); }
        }
        let changed = sent_message("reply", Some("new@rxtsel.dev"), "2026-01-02T00:00:00Z", "delivered");
        inherit_reply_settings(&mut context, vec![("sent".into(), json), ("sent".into(), serde_json::to_string(&changed).unwrap())]).unwrap();
        assert_eq!(context.reply_to.as_deref(), Some("new@rxtsel.dev"));
    }

    #[tokio::test]
    async fn accepted_sent_replies_persist_defaults_before_remote_sync_and_are_account_scoped() {
        let directory = tempfile::tempdir().unwrap();
        let database = crate::infrastructure::database::Database::new(directory.path().join("reply.sqlite3"));
        use crate::infrastructure::database::mail::{cached_thread, store_outgoing_sent, Mailbox};
        let sent = sent_message("root", Some("c@rxtsel.dev"), "2026-01-01T00:00:00Z", "sent");
        store_outgoing_sent(&database, "a".into(), &sent).await.unwrap();
        assert_eq!(crate::infrastructure::database::mail::missing_detail_ids(&database, "a".into(), Mailbox::Sent, vec!["root".into()]).await.unwrap(), ["root"]);
        let mut hydrated = sent.clone();
        hydrated.body_format_version = 2;
        hydrated.message_id = Some("<root@example.com>".into());
        store_outgoing_sent(&database, "a".into(), &hydrated).await.unwrap();
        assert!(crate::infrastructure::database::mail::missing_detail_ids(&database, "a".into(), Mailbox::Sent, vec!["root".into()]).await.unwrap().is_empty());
        let other_account = sent_message("other", Some("other@example.com"), "2026-01-03T00:00:00Z", "sent");
        store_outgoing_sent(&database, "b".into(), &other_account).await.unwrap();
        let mut context = reply_context(&original("Invoice")).unwrap();
        let messages = cached_thread(&database, "a".into(), context.thread_id.clone(), Mailbox::Sent).await.unwrap();
        inherit_reply_settings(&mut context, messages).unwrap();
        assert_eq!(context.reply_to.as_deref(), Some("c@rxtsel.dev"));
        let opt_out = sent_message("opt-out", None, "2026-01-02T00:00:00Z", "sent");
        store_outgoing_sent(&database, "a".into(), &opt_out).await.unwrap();
        let messages = cached_thread(&database, "a".into(), context.thread_id.clone(), Mailbox::Sent).await.unwrap();
        inherit_reply_settings(&mut context, messages).unwrap();
        assert!(context.reply_to.is_none());
        assert!(crate::infrastructure::database::mail::cached_page::<crate::sent::SentEmail>(&database, "a".into(), Mailbox::Sent, 1, None, None).await.unwrap().is_none());
    }

    #[test]
    fn replies_without_references_reuse_in_reply_to_for_the_thread_and_outgoing_ancestry() {
        for references in [None, Some(""), Some("bad ancestry") ] {
            let mut incoming = original("Invoice");
            incoming.references = references.map(str::to_owned);
            incoming.in_reply_to = Some("<root@example.com>".into());
            let context = reply_context(&incoming).unwrap();
            assert_eq!(context.thread_id, crate::inbound::thread_id(&incoming).unwrap());
            assert_eq!(context.thread_id, "root@example.com");
            let email = serde_json::to_value(compose_email(&draft(), Some(&context)).unwrap()).unwrap();
            assert_eq!(email["headers"]["References"], "<root@example.com> <parent@example.com>");
            assert_eq!(email["headers"]["In-Reply-To"], "<parent@example.com>");
        }
    }

    #[tokio::test]
    async fn refreshed_sent_metadata_wins_over_a_stale_scheduled_body_for_reply_defaults() {
        use crate::infrastructure::database::{mail, Database};
        let directory = tempfile::tempdir().unwrap();
        let database = Database::new(directory.path().join("status.sqlite3"));
        let root = sent_message("root", Some("alias@example.com"), "2026-01-01T00:00:00Z", "delivered");
        let mut latest = sent_message("latest", None, "2026-01-02T00:00:00Z", "scheduled");
        latest.body_format_version = 2;
        mail::store_outgoing_sent(&database, "a".into(), &root).await.unwrap();
        mail::store_outgoing_sent(&database, "a".into(), &latest).await.unwrap();
        let mut context = reply_context(&original("Invoice")).unwrap();
        let messages = mail::cached_thread(&database, "a".into(), context.thread_id.clone(), mail::Mailbox::Sent).await.unwrap();
        inherit_reply_settings(&mut context, messages).unwrap();
        assert_eq!(context.reply_to.as_deref(), Some("alias@example.com"));
        latest.last_event = Some("delivered".into());
        let page = crate::infrastructure::pagination::EmailPage { items: vec![latest], has_more: false, next_cursor: None, cache: None };
        mail::store_page(&database, "a".into(), mail::Mailbox::Sent, 12, None, &page).await.unwrap();
        let body: serde_json::Value = serde_json::from_str(&mail::cached_detail(&database, "a".into(), "sent".into(), "latest".into()).await.unwrap().unwrap()).unwrap();
        assert_eq!(body["lastEvent"], "scheduled");
        load_reply_settings(&database, "a".into(), &mut context, |_| async { Err("No download should be needed".into()) }).await.unwrap();
        assert!(context.reply_to.is_none());
    }

    #[tokio::test]
    async fn replying_after_restart_recovers_an_unlinked_first_send_without_opening_sent() {
        use crate::infrastructure::database::{mail, Database};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("restart.sqlite3");
        let database = Database::new(path.clone());
        let mut pending = sent_message("first-send", Some("alias@example.com"), "2026-01-01T00:00:00Z", "sent");
        pending.thread_id = None;
        mail::store_outgoing_sent(&database, "a".into(), &pending).await.unwrap();
        let mut other = pending.clone();
        other.id = "other-account".into();
        mail::store_outgoing_sent(&database, "b".into(), &other).await.unwrap();
        let mut deleted = pending.clone();
        deleted.id = "deleted".into();
        mail::store_outgoing_sent(&database, "a".into(), &deleted).await.unwrap();
        mail::set_deleted(&database, "a".into(), mail::Mailbox::Sent, deleted.id, true).await.unwrap();
        drop(database);
        let database = Database::new(path);
        let mut incoming = original("Invoice");
        incoming.references = None;
        incoming.in_reply_to = Some("<root@example.com>".into());
        let mut context = reply_context(&incoming).unwrap();
        assert!(mail::cached_thread(&database, "a".into(), context.thread_id.clone(), mail::Mailbox::Sent).await.unwrap().is_empty());
        let mut downloaded = Vec::new();
        load_reply_settings(&database, "a".into(), &mut context, |id| {
            downloaded.push(id.clone());
            let database = database.clone();
            let mut hydrated = pending.clone();
            async move {
                assert_eq!(id, hydrated.id);
                hydrated.body_format_version = 2;
                hydrated.message_id = Some("<root@example.com>".into());
                hydrated.thread_id = Some("root@example.com".into());
                mail::store_outgoing_sent(&database, "a".into(), &hydrated).await?;
                Ok(hydrated)
            }
        }).await.unwrap();
        assert_eq!(downloaded, ["first-send"]);
        assert_eq!(context.reply_to.as_deref(), Some("alias@example.com"));
        assert!(mail::pending_sent_header_ids(&database, "a".into()).await.unwrap().is_empty());
        load_reply_settings(&database, "a".into(), &mut context, |_| async { Err("Recovered headers must not be fetched again".into()) }).await.unwrap();
        assert_eq!(context.reply_to.as_deref(), Some("alias@example.com"));
    }

    #[tokio::test]
    async fn a_scheduled_first_send_without_message_id_can_be_linked_after_delivery() {
        use crate::infrastructure::database::{mail, Database};
        let directory = tempfile::tempdir().unwrap();
        let database = Database::new(directory.path().join("scheduled-root.sqlite3"));
        let mut pending = sent_message("scheduled-root", Some("alias@example.com"), "2026-01-01T00:00:00Z", "scheduled");
        pending.body_format_version = 2;
        pending.thread_id = None;
        mail::store_outgoing_sent(&database, "a".into(), &pending).await.unwrap();
        let mut context = reply_context(&original("Invoice")).unwrap();
        load_reply_settings(&database, "a".into(), &mut context, |id| {
            let database = database.clone();
            let mut delivered = pending.clone();
            async move {
                assert_eq!(id, "scheduled-root");
                delivered.last_event = Some("delivered".into());
                delivered.message_id = Some("<root@example.com>".into());
                delivered.thread_id = Some("root@example.com".into());
                mail::store_outgoing_sent(&database, "a".into(), &delivered).await?;
                Ok(delivered)
            }
        }).await.unwrap();
        assert_eq!(context.reply_to.as_deref(), Some("alias@example.com"));
    }

    #[test]
    fn reply_to_takes_precedence_and_addresses_are_unquoted_and_deduplicated() {
        let mut original = original("Invoice");
        original.reply_to = vec![
            "\"Replies, Team\" <replies@example.com>".into(),
            "REPLIES@example.com".into(),
        ];
        original.received_for = vec!["alias@example.com".into()];
        original.references = Some(r#"["<root@example.com>","<parent@example.com>"]"#.into());
        let context = reply_context(&original).unwrap();
        assert_eq!(context.to, vec!["replies@example.com"]);
        assert_eq!(context.from_candidates, vec!["support@example.com"]);
        assert_eq!(
            context.references,
            "<root@example.com> <parent@example.com>"
        );
        let dto = serde_json::to_value(context).unwrap();
        assert!(dto.get("messageId").is_none());
        assert!(dto.get("references").is_none());
        original.reply_to.clear();
        assert_eq!(
            reply_context(&original).unwrap().to,
            vec!["sender@example.com"]
        );
    }

    #[test]
    fn reply_subject_has_one_prefix_including_empty_and_folded_subjects() {
        for (input, expected) in [
            ("Invoice", "Re: Invoice"),
            ("RE: re: Invoice", "Re: Invoice"),
            ("Re:Invoice", "Re: Invoice"),
            ("", "Re: (No subject)"),
            ("Hello\r\n world", "Re: Hello world"),
            ("📨 Invoice", "Re: 📨 Invoice"),
        ] {
            assert_eq!(reply_context(&original(input)).unwrap().subject, expected);
        }
    }

    #[test]
    fn sender_controlled_headers_cannot_inject_recipients_or_break_a_valid_parent() {
        let mut original = original("Invoice");
        for invalid in [
            "",
            "<one@example.com> <two@example.com>",
            "<id\r\nBcc: attacker@example.com>",
        ] {
            original.message_id = Some(invalid.into());
            assert!(reply_context(&original).is_err());
        }
        original.message_id = None;
        assert!(reply_context(&original).is_err());
        original.message_id = Some("<parent@example.com>".into());
        for invalid in [
            "bad ancestry\r\nBcc: attacker@example.com".to_string(),
            "x".repeat(16 * 1024 + 1),
        ] {
            original.references = Some(invalid);
            assert_eq!(
                reply_context(&original).unwrap().references,
                "<parent@example.com>"
            );
        }
        original.reply_to = vec!["bad@example.com\r\nBcc: attacker@example.com".into()];
        assert!(reply_context(&original).is_err());
        original.reply_to = vec!["reply@example.com".into(); 51];
        assert!(reply_context(&original).is_err());
    }

    #[test]
    fn replies_enforce_original_subject_and_thread_headers_and_keep_composer_features() {
        let reply = reply_context(&original("Invoice")).unwrap();
        let mut data = draft();
        let email = serde_json::to_value(compose_email(&data, Some(&reply)).unwrap()).unwrap();
        assert_eq!(email["subject"], "Re: Invoice");
        assert_eq!(email["headers"]["In-Reply-To"], "<parent@example.com>");
        assert_eq!(
            email["headers"]["References"],
            "<root@example.com> <parent@example.com>"
        );
        assert_eq!(email["html"], "<p>My reply</p>");
        assert_eq!(email["cc"][0], "copy@example.com");
        assert_eq!(email["bcc"][0], "hidden@example.com");
        assert!(email.get("reply_to").is_none());
        assert_eq!(email["attachments"][0]["filename"], "reply.txt");
        assert_eq!(email["attachments"][0]["content"], serde_json::json!([120]));
        let ordinary = serde_json::to_value(compose_email(&data, None).unwrap()).unwrap();
        assert_eq!(ordinary["subject"], "Untrusted subject");
        assert!(ordinary.get("headers").is_none());
        data.subject = "Bad\r\nBcc: injected@example.com".into();
        assert!(compose_email(&data, None).is_err());
    }

    #[test]
    fn outgoing_reply_to_is_optional_validated_and_independent_of_thread_headers() {
        let mut data = draft();
        let ordinary = serde_json::to_value(compose_email(&data, None).unwrap()).unwrap();
        assert!(ordinary.get("reply_to").is_none());
        data.reply_to = Some("replies@example.com".into());
        let reply = reply_context(&original("Invoice")).unwrap();
        for context in [None, Some(&reply)] {
            let email = serde_json::to_value(compose_email(&data, context).unwrap()).unwrap();
            assert_eq!(email["reply_to"], serde_json::json!(["replies@example.com"]));
            if context.is_some() {
                assert_eq!(email["headers"]["In-Reply-To"], "<parent@example.com>");
            } else {
                assert!(email.get("headers").is_none());
            }
        }
        for invalid in ["", "not-an-email", "a@example.com, b@example.com", "valid@example.com\r\nBcc: injected@example.com"] {
            data.reply_to = Some(invalid.into());
            assert!(compose_email(&data, None).is_err());
        }
    }

    #[test]
    fn attachment_data_is_bounded_validated_and_counted_across_files() {
        let mut total = 0;
        let file = AttachmentPayload {
            filename: "file.bin".into(),
            content: general_purpose::STANDARD.encode([0, 255]),
        };
        assert_eq!(attachment_bytes(&file, &mut total).unwrap(), vec![0, 255]);
        assert_eq!(total, 2);
        total = crate::attachments::MAX_ATTACHMENT_BYTES - 1;
        assert!(attachment_bytes(&file, &mut total).is_err());
        let invalid = AttachmentPayload {
            filename: "file.bin".into(),
            content: "invalid!".into(),
        };
        assert!(attachment_bytes(&invalid, &mut 0).is_err());
        let invalid = AttachmentPayload {
            filename: "../file.bin".into(),
            content: "".into(),
        };
        assert!(attachment_bytes(&invalid, &mut 0).is_err());
    }
}
