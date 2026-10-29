use crate::infrastructure::database::{
    local_state,
    mail::{self, Mailbox},
};
use crate::infrastructure::pagination::{email_page, list_options, EmailPage};
use resend_rs::types::GetInboundEmailOptions;
use resend_rs::{types::InboundAttachment, Resend};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Wry};

use crate::oauth;
use crate::permissions;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboundEmailDto {
    pub id: String,
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub created_at: String,
    /// Domain this email was addressed to (derived from the recipients).
    pub domain: String,
    /// All envelope recipient domains, including unknown/deleted domains.
    pub domains: Vec<String>,
}

pub type InboundAttachmentDto = crate::attachments::AttachmentInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboundEmailDetailDto {
    pub id: String,
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub created_at: String,
    /// Original HTML source; the reader must prepare a sanitized document before display.
    pub html: Option<String>,
    #[serde(default)]
    pub html_source_version: u8,
    pub text: Option<String>,
    pub attachments: Vec<InboundAttachmentDto>,
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub reply_to: Vec<String>,
    #[serde(default)]
    pub references: Option<String>,
    #[serde(default)]
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub received_for: Vec<String>,
}

impl InboundEmailDetailDto {
    fn needs_html_upgrade(&self) -> bool {
        self.html.is_some() && self.html_source_version == 0
    }

    fn can_reuse(&self, for_reply: bool) -> bool {
        if for_reply {
            self.message_id.is_some()
        } else {
            !self.needs_html_upgrade()
        }
    }
}

fn original_recipients(
    headers: &std::collections::HashMap<String, String>,
    fallback: &[String],
) -> Vec<String> {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("to"))
        .and_then(|(_, value)| mailparse::addrparse(value).ok())
        .map(|addresses| {
            addresses
                .iter()
                .flat_map(|address| match address {
                    mailparse::MailAddr::Single(mailbox) => vec![mailbox.addr.clone()],
                    mailparse::MailAddr::Group(group) => group
                        .addrs
                        .iter()
                        .map(|mailbox| mailbox.addr.clone())
                        .collect(),
                })
                .collect()
        })
        .filter(|addresses: &Vec<String>| !addresses.is_empty())
        .unwrap_or_else(|| fallback.to_vec())
}

pub(crate) fn parsed_message_ids(value: &str) -> Option<Vec<String>> {
    let values = serde_json::from_str::<Vec<String>>(value).ok();
    let ids: Vec<String> = if let Some(values) = values {
        values
            .into_iter()
            .flat_map(|value| {
                mailparse::msgidparse(&value)
                    .map(|ids| ids.iter().cloned().collect::<Vec<_>>())
                    .unwrap_or_default()
            })
            .collect()
    } else {
        mailparse::msgidparse(value).ok()?.iter().cloned().collect()
    };
    Some(
        ids.into_iter()
            .filter(|id| {
                !id.is_empty()
                    && !id
                        .chars()
                        .any(|ch| ch.is_control() || ch.is_whitespace() || matches!(ch, '<' | '>'))
            })
            .collect(),
    )
}

pub(crate) fn thread_id(detail: &InboundEmailDetailDto) -> Option<String> {
    detail
        .references
        .as_deref()
        .and_then(parsed_message_ids)
        .and_then(|ids| ids.first().cloned())
        .or_else(|| {
            detail
                .in_reply_to
                .as_deref()
                .and_then(parsed_message_ids)
                .and_then(|ids| ids.first().cloned())
        })
        .or_else(|| {
            detail
                .message_id
                .as_deref()
                .and_then(parsed_message_ids)
                .and_then(|ids| ids.first().cloned())
        })
}

fn map_attachment(a: &InboundAttachment) -> InboundAttachmentDto {
    InboundAttachmentDto {
        id: a.id.to_string(),
        filename: a.filename.clone(),
        content_type: a.content_type.clone(),
        size: a.size,
        content_id: a.content_id.clone(),
        content_disposition: a.content_disposition.clone(),
    }
}

async fn client(app: &AppHandle<Wry>, account_id: &str) -> Result<Resend, String> {
    let credential = oauth::get_account_credential(app, account_id).await?;
    Ok(Resend::new(&credential))
}

/// Derives the recipient domain from the email's own addresses. Resend has
/// no server-side domain filter, so the unified inbox carries this badge.
fn recipient_domains(received_for: &[String], to: &[String]) -> Vec<String> {
    let recipients = if received_for.is_empty() {
        to
    } else {
        received_for
    };
    let mut domains = Vec::new();
    for recipient in recipients {
        if let Some((_, host)) = recipient.rsplit_once('@') {
            let host = host.trim().trim_end_matches('>').to_lowercase();
            if !host.is_empty()
                && !host.chars().any(char::is_whitespace)
                && !domains.contains(&host)
            {
                domains.push(host);
            }
        }
    }
    domains
}

pub(crate) async fn fetch_inbound_page(
    resend: &Resend,
    limit: Option<usize>,
    after: Option<&str>,
) -> Result<EmailPage<InboundEmailDto>, String> {
    let options = list_options(limit, after)?;
    // Account-wide history, one fixed-size page after the previous page's ID.
    let response = match after {
        Some(cursor) => resend.receiving.list(options.list_after(cursor)).await,
        None => resend.receiving.list(options).await,
    }
    .map_err(|e| permissions::map_resend_error("Failed to fetch inbox", e))?;
    let has_more = response.has_more;

    let items = response
        .data
        .iter()
        .map(|email| {
            let domains = recipient_domains(&email.received_for, &email.to);
            InboundEmailDto {
                id: email.id.to_string(),
                from: email.from.clone(),
                to: email.to.clone(),
                subject: email.subject.clone(),
                created_at: email.created_at.clone(),
                domain: domains.first().cloned().unwrap_or_else(|| "unknown".into()),
                domains,
            }
        })
        .collect();

    email_page(items, has_more, after, |email: &InboundEmailDto| &email.id)
}

pub(crate) async fn get_inbound_email(
    app: AppHandle<Wry>,
    account_id: String,
    email_id: String,
    require_reply_metadata: bool,
) -> Result<InboundEmailDetailDto, String> {
    if uuid::Uuid::parse_str(&email_id).is_err() {
        return Err("Invalid received email ID".into());
    }
    let database = local_state::for_account(&app, &account_id).await?;
    let cached = mail::cached_detail(
        &database,
        account_id.clone(),
        "inbox".into(),
        email_id.clone(),
    )
    .await?
    .map(|json| {
        serde_json::from_str::<InboundEmailDetailDto>(&json)
            .map_err(|_| "Could not read downloaded email".to_string())
    })
    .transpose()?;
    if let Some(detail) = cached
        .as_ref()
        .filter(|detail| detail.can_reuse(require_reply_metadata))
    {
        return Ok(detail.clone());
    }
    // Old cache entries lost their CSS; recover the source without deleting the offline copy.
    let downloaded = async {
        let resend = client(&app, &account_id).await?;
        let email = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            resend
                .receiving
                .get(&email_id, GetInboundEmailOptions::default()),
        )
        .await
        .map_err(|_| "Email download timed out".to_string())?
        .map_err(|e| permissions::map_resend_error("Failed to fetch email", e))?;

        let search_text = email
            .text
            .clone()
            .unwrap_or_else(|| mail::plain_html(email.html.as_deref().unwrap_or_default()));
        let detail = InboundEmailDetailDto {
            id: email.id.to_string(),
            from: email.from.clone(),
            to: original_recipients(&email.headers, &email.to),
            subject: email.subject.clone(),
            created_at: email.created_at.clone(),
            html: email.html.clone(),
            html_source_version: 1,
            text: email.text.clone(),
            attachments: email.attachments.iter().map(map_attachment).collect(),
            message_id: Some(email.message_id.clone()),
            reply_to: email.reply_to.clone(),
            references: email
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("references"))
                .map(|(_, value)| value.clone()),
            in_reply_to: email
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("in-reply-to"))
                .map(|(_, value)| value.clone()),
            received_for: email.received_for.clone(),
        };
        crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
        mail::store_detail(
            &database,
            account_id.clone(),
            Mailbox::Inbox,
            email_id.clone(),
            &detail,
            search_text,
        )
        .await?;
        if let Some(thread_id) = thread_id(&detail) {
            mail::store_thread_link(
                &database,
                account_id.clone(),
                Mailbox::Inbox,
                email_id.clone(),
                thread_id,
            )
            .await?;
        }
        Ok::<_, String>(detail)
    }
    .await;
    match downloaded {
        Ok(detail) => Ok(detail),
        Err(error) => {
            if let Some(detail) = cached {
                if require_reply_metadata && detail.message_id.is_none() {
                    return Err(error);
                }
                crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
                eprintln!(
                    "[WARN] Could not restore original email formatting; using downloaded copy"
                );
                Ok(detail)
            } else {
                Err(error)
            }
        }
    }
}

#[tauri::command]
pub async fn open_inbound_email(
    app: AppHandle<Wry>,
    account_id: String,
    email_id: String,
) -> Result<InboundEmailDetailDto, String> {
    let database = local_state::for_account(&app, &account_id).await?;
    mail::ensure_visible(&database, account_id.clone(), Mailbox::Inbox, email_id.clone()).await?;
    let detail =
        get_inbound_email(app.clone(), account_id.clone(), email_id.clone(), false).await?;
    if crate::infrastructure::settings_store::mark_inbound_read(
        app.clone(),
        account_id.clone(),
        email_id.clone(),
    )
    .await
    .is_err()
    {
        eprintln!("[WARN] Could not mark inbox message as read");
    } else if app
        .emit(
            "inbox-read",
            serde_json::json!({ "accountId": account_id, "emailId": email_id }),
        )
        .is_err()
    {
        eprintln!("[WARN] Could not publish inbox read state");
    }
    Ok(detail)
}

#[cfg(test)]
mod tests {
    use super::{fetch_inbound_page, recipient_domains, InboundEmailDetailDto};
    use crate::email_html::sanitize_email_html;
    use crate::infrastructure::pagination_test_server::mock_history;

    #[tokio::test]
    async fn inbound_sdk_pages_traverse_history_and_keep_every_receiving_domain() {
        let (resend, server, expected) = mock_history("/emails/receiving");
        let mut after = None;
        let mut ids = Vec::new();
        for _ in 0..4 {
            let page = fetch_inbound_page(&resend, Some(100), after.as_deref())
                .await
                .unwrap();
            for email in &page.items {
                assert_eq!(email.domains, vec!["a.example", "b.example"]);
                ids.push(email.id.clone());
            }
            assert_eq!(page.has_more, ids.len() < expected.len());
            after = page.next_cursor;
        }
        server.join().unwrap();
        assert_eq!(ids, expected);
        assert!(after.is_none());
    }

    #[test]
    fn thread_id_uses_first_reference_then_parent_then_message_id() {
        let mut detail: InboundEmailDetailDto = serde_json::from_value(serde_json::json!({
            "id":"email", "from":"sender@example.test", "to":["inbox@example.test"],
            "subject":"Re: hello", "createdAt":"2026-01-01T00:00:00Z", "html":null,
            "text":"reply", "attachments":[], "messageId":"<reply@example.test>",
            "inReplyTo":"<parent@example.test>", "references":"[\"<root@example.test>\",\"<parent@example.test>\"]"
        })).unwrap();
        assert_eq!(
            super::thread_id(&detail).as_deref(),
            Some("root@example.test")
        );
        detail.references = None;
        assert_eq!(
            super::thread_id(&detail).as_deref(),
            Some("parent@example.test")
        );
        detail.in_reply_to = None;
        assert_eq!(
            super::thread_id(&detail).as_deref(),
            Some("reply@example.test")
        );
    }

    #[test]
    fn original_to_header_overrides_resend_envelope_alias() {
        let headers = std::collections::HashMap::from([(
            "To".to_string(),
            "inbox@example.test, Other <other@example.com>".to_string(),
        )]);
        assert_eq!(
            super::original_recipients(&headers, &["resend@resend.app".into()]),
            vec!["inbox@example.test", "other@example.com"]
        );
    }

    #[test]
    fn receiving_domains_use_every_envelope_recipient_not_sender_or_header() {
        let envelope = vec![
            "one@a.example".into(),
            "Two <two@B.example>".into(),
            "again@a.example".into(),
        ];
        let header = vec!["other@c.example".into()];
        assert_eq!(
            recipient_domains(&envelope, &header),
            vec!["a.example", "b.example"]
        );
        assert_eq!(recipient_domains(&[], &header), vec!["c.example"]);
        assert!(recipient_domains(&["invalid".into()], &header).is_empty());
        assert!(recipient_domains(&[], &[]).is_empty());
    }

    #[test]
    fn legacy_html_is_upgraded_but_original_source_and_plain_text_are_reusable() {
        let source = include_str!("../tests/fixtures/styled-email.html");
        let legacy = serde_json::json!({
            "id": "email", "from": "sender@example.com", "to": ["to@example.com"],
            "subject": "Invoice", "createdAt": "2026-01-01T00:00:00Z",
            "html": source, "text": "Pay online", "attachments": []
        });
        let detail: InboundEmailDetailDto = serde_json::from_value(legacy.clone()).unwrap();
        assert!(detail.needs_html_upgrade());
        let mut current = legacy.clone();
        current["htmlSourceVersion"] = serde_json::json!(1);
        let detail: InboundEmailDetailDto = serde_json::from_value(current).unwrap();
        assert!(!detail.needs_html_upgrade());
        assert_eq!(serde_json::to_value(detail).unwrap()["html"], source);
        let mut text_only = legacy;
        text_only["html"] = serde_json::Value::Null;
        let detail: InboundEmailDetailDto = serde_json::from_value(text_only).unwrap();
        assert!(!detail.needs_html_upgrade());
    }

    #[test]
    fn legacy_downloads_remain_readable_but_reply_metadata_requires_a_refresh() {
        let mut detail: InboundEmailDetailDto = serde_json::from_value(serde_json::json!({
            "id":"email", "from":"sender@example.com", "to":["to@example.com"],
            "subject":"Invoice", "createdAt":"2026-01-01T00:00:00Z", "html":null,
            "text":"Offline body", "attachments":[]
        }))
        .unwrap();
        assert!(detail.can_reuse(false));
        assert!(!detail.can_reuse(true));
        detail.message_id = Some("<parent@example.com>".into());
        assert!(detail.can_reuse(true));
        detail.html = Some("<p>Old formatting</p>".into());
        assert!(!detail.can_reuse(false));
        assert!(detail.can_reuse(true));
    }

    #[test]
    fn strips_script_tags_and_content() {
        let html = r#"<p>ok</p><script>alert('xss')</script>"#;
        let out = sanitize_email_html(html);
        assert!(!out.contains("script"), "got: {out}");
        assert!(!out.contains("alert"));
        assert!(out.contains("ok"));
    }

    #[test]
    fn strips_event_handlers_and_javascript_urls() {
        let html = r#"<p onclick="evil()">hi</p><a href="javascript:evil()">x</a>"#;
        let out = sanitize_email_html(html);
        assert!(!out.contains("onclick"));
        assert!(!out.contains("javascript:"));
        assert!(out.contains("hi"));
    }

    #[test]
    fn strips_iframes_forms_and_object_tags() {
        let html = "<iframe src=\"//evil\"></iframe><form action=\"x\"></form><object data=\"y\"></object><p>safe</p>";
        let out = sanitize_email_html(html);
        assert!(!out.contains("iframe"));
        assert!(!out.contains("form"));
        assert!(!out.contains("object"));
        assert!(out.contains("safe"));
    }

    #[test]
    fn preserves_legitimate_structure() {
        let html = r#"<table border="0"><tr><td><b>Bold</b> and <a href="https://ok.com">link</a></td></tr></table><img src="https://img" alt="i">"#;
        let out = sanitize_email_html(html);
        assert!(out.contains("<table"));
        assert!(out.contains("<b>"));
        assert!(out.contains("href=\"https://ok.com\""));
        assert!(out.contains("<img"));
    }
}
