use crate::email_document::sanitize_email_html;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboundAttachmentDto {
    pub id: String,
    pub filename: Option<String>,
    pub content_type: String,
    pub size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboundEmailDetailDto {
    pub id: String,
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub created_at: String,
    /// Sanitized HTML body, when available.
    pub html: Option<String>,
    pub text: Option<String>,
    pub attachments: Vec<InboundAttachmentDto>,
}

fn map_attachment(a: &InboundAttachment) -> InboundAttachmentDto {
    InboundAttachmentDto {
        id: a.id.to_string(),
        filename: a.filename.clone(),
        content_type: a.content_type.clone(),
        size: a.size,
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
) -> Result<InboundEmailDetailDto, String> {
    let database = local_state::for_account(&app, &account_id).await?;
    if let Some(json) = mail::cached_detail(
        &database,
        account_id.clone(),
        "inbox".into(),
        email_id.clone(),
    )
    .await?
    {
        let mut detail: InboundEmailDetailDto = serde_json::from_str(&json)
            .map_err(|_| "Could not read downloaded email".to_string())?;
        detail.html = detail.html.as_deref().map(sanitize_email_html);
        return Ok(detail);
    }
    let resend = client(&app, &account_id).await?;

    let email = resend
        .receiving
        .get(&email_id, GetInboundEmailOptions::default())
        .await
        .map_err(|e| permissions::map_resend_error("Failed to fetch email", e))?;

    let search_text = email
        .text
        .clone()
        .unwrap_or_else(|| mail::plain_html(email.html.as_deref().unwrap_or_default()));
    let detail = InboundEmailDetailDto {
        id: email.id.to_string(),
        from: email.from.clone(),
        to: email.to.clone(),
        subject: email.subject.clone(),
        created_at: email.created_at.clone(),
        html: email.html.as_deref().map(sanitize_email_html),
        text: email.text.clone(),
        attachments: email.attachments.iter().map(map_attachment).collect(),
    };
    crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
    mail::store_detail(
        &database,
        account_id,
        Mailbox::Inbox,
        email_id,
        &detail,
        search_text,
    )
    .await?;
    Ok(detail)
}

#[tauri::command]
pub async fn open_inbound_email(
    app: AppHandle<Wry>,
    account_id: String,
    email_id: String,
) -> Result<InboundEmailDetailDto, String> {
    let detail = get_inbound_email(app.clone(), account_id.clone(), email_id.clone()).await?;
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
    use super::{fetch_inbound_page, recipient_domains, sanitize_email_html};
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
