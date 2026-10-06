use std::collections::{HashMap, HashSet};

use ammonia::Builder;
use resend_rs::list_opts::ListOptions;
use resend_rs::types::GetInboundEmailOptions;
use resend_rs::{Resend, types::InboundAttachment};
use serde::Serialize;
use tauri::{AppHandle, Wry};

use crate::oauth;
use crate::permissions;

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboundAttachmentDto {
    pub id: String,
    pub filename: Option<String>,
    pub content_type: String,
    pub size: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
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

fn set(items: &[&'static str]) -> HashSet<&'static str> {
    items.iter().copied().collect()
}

/// Allowlist sanitizer for untrusted email HTML. Scripts, frames and forms
/// are removed here; the frontend renders the result inside a fully
/// sandboxed iframe as the second security boundary.
fn sanitize_email_html(html: &str) -> String {
    let tags = set(&[
        "a", "b", "blockquote", "br", "center", "code", "div", "em", "h1",
        "h2", "h3", "h4", "h5", "h6", "hr", "i", "img", "li", "ol", "p",
        "pre", "s", "span", "strong", "sub", "sup", "table", "tbody", "td",
        "tfoot", "th", "thead", "tr", "u", "ul", "font",
    ]);

    let tag_attributes: HashMap<&str, HashSet<&str>> = HashMap::from([
        ("a", set(&["href", "target"])),
        ("img", set(&["src", "alt", "width", "height"])),
        ("font", set(&["color", "size", "face"])),
        (
            "table",
            set(&["border", "cellpadding", "cellspacing", "width"]),
        ),
        ("td", set(&["colspan", "rowspan", "width", "bgcolor"])),
        ("th", set(&["colspan", "rowspan", "width", "bgcolor"])),
    ]);

    let generic_attributes = set(&["align", "valign", "dir", "lang"]);

    Builder::default()
        .tags(tags)
        .tag_attributes(tag_attributes)
        .generic_attributes(generic_attributes)
        .clean(html)
        .to_string()
}

async fn client(app: &AppHandle<Wry>, account_id: &str) -> Result<Resend, String> {
    let credential = oauth::get_account_credential(app, account_id).await?;
    Ok(Resend::new(&credential))
}

/// Derives the recipient domain from the email's own addresses. Resend has
/// no server-side domain filter, so the unified inbox carries this badge.
fn recipient_domains(received_for: &[String], to: &[String]) -> Vec<String> {
    let recipients = if received_for.is_empty() { to } else { received_for };
    let mut domains = Vec::new();
    for recipient in recipients {
        if let Some((_, host)) = recipient.rsplit_once('@') {
            let host = host.trim().trim_end_matches('>').to_lowercase();
            if !host.is_empty() && !host.chars().any(char::is_whitespace) && !domains.contains(&host) {
                domains.push(host);
            }
        }
    }
    domains
}

#[tauri::command]
pub async fn list_inbound_emails(
    app: AppHandle<Wry>, account_id: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<InboundEmailDto>, String> {
    let resend = client(&app, &account_id).await?;

    // Unified across every enabled domain: one request feeds all views.
    let lim = limit.unwrap_or(12);
    let off = offset.unwrap_or(0);
    let list_opts = ListOptions::default().with_limit((lim + off).clamp(1, 100) as u8);

    let response = resend
        .receiving
        .list(list_opts)
        .await
        .map_err(|e| permissions::map_resend_error("Failed to fetch inbox", e))?;

    let items = response
        .data
        .iter()
        .skip(off)
        .take(lim)
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

    Ok(items)
}

#[tauri::command]
pub async fn get_inbound_email(
    app: AppHandle<Wry>, account_id: String,
    email_id: String,
) -> Result<InboundEmailDetailDto, String> {
    let resend = client(&app, &account_id).await?;

    let email = resend
        .receiving
        .get(&email_id, GetInboundEmailOptions::default())
        .await
        .map_err(|e| permissions::map_resend_error("Failed to fetch email", e))?;

    Ok(InboundEmailDetailDto {
        id: email.id.to_string(),
        from: email.from.clone(),
        to: email.to.clone(),
        subject: email.subject.clone(),
        created_at: email.created_at.clone(),
        html: email.html.as_deref().map(sanitize_email_html),
        text: email.text.clone(),
        attachments: email.attachments.iter().map(map_attachment).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::{sanitize_email_html, recipient_domains};

    #[test]
    fn receiving_domains_use_every_envelope_recipient_not_sender_or_header() {
        let envelope = vec!["one@a.example".into(), "Two <two@B.example>".into(), "again@a.example".into()];
        let header = vec!["other@c.example".into()];
        assert_eq!(recipient_domains(&envelope, &header), vec!["a.example", "b.example"]);
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
