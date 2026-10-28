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

    pub content: String,

    #[serde(rename = "replyEmailId")]
    pub reply_email_id: Option<String>,

    #[serde(default)]
    pub attachments: Vec<AttachmentPayload>,
}

fn attachment_bytes(attachment: &AttachmentPayload, total: &mut u64) -> Result<Vec<u8>, String> {
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
    references.retain(|id| id != &message_id);
    references.push(message_id.clone());
    let thread_id = references
        .first()
        .cloned()
        .unwrap_or_else(|| message_id.clone());
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
        message_id,
        references: references.join(" "),
        thread_id,
    })
}

#[tauri::command]
pub async fn prepare_reply(
    app: AppHandle<Wry>,
    account_id: String,
    email_id: String,
) -> Result<ReplyContext, String> {
    let original =
        crate::inbound::get_inbound_email(app.clone(), account_id.clone(), email_id, true).await?;
    crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
    reply_context(&original)
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
    for cc in &data.cc {
        email = email.with_cc(cc.as_str());
    }
    for bcc in &data.bcc {
        email = email.with_bcc(bcc.as_str());
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
    if let Some(reply) = reply {
        if crate::infrastructure::database::mail::store_thread_link(
            &database,
            account_id,
            crate::infrastructure::database::mail::Mailbox::Sent,
            sent.id.to_string(),
            reply.thread_id,
        )
        .await
        .is_err()
        {
            eprintln!("[WARN] Sent reply but could not save its local thread link");
        }
    }
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

    fn draft() -> EmailComposerData {
        serde_json::from_value(serde_json::json!({
            "from":"Support <support@example.com>", "to":["sender@example.com"], "subject":"Untrusted subject",
            "content":"<p>My reply</p>", "cc":["copy@example.com"], "bcc":["hidden@example.com"],
            "replyTo":"ignored@example.com", "messageId":"<ignored@example.com>", "attachments":[{"filename":"reply.txt", "content":"eA=="}]
        })).unwrap()
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
