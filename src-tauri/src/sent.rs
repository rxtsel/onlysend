use crate::oauth;
use crate::permissions;
use resend_rs::{list_opts::ListOptions, Resend};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Wry};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentEmail {
    pub id: String,
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub html: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    pub reply_to: Option<String>,
}

#[tauri::command]
pub async fn list_sent_emails(
    app: AppHandle<Wry>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<SentEmail>, String> {
    // Bearer credential: API key or OAuth access token
    let credential = oauth::get_credential(&app).await?;

    let resend = Resend::new(&credential);

    let lim = limit.unwrap_or(12);
    let off = offset.unwrap_or(0);

    // Request offset + limit, to manually "paginate"
    let effective_limit = (lim + off).min(255);
    let list_opts = ListOptions::default().with_limit(effective_limit as u8);

    let response = resend
        .emails
        .list(list_opts)
        .await
        .map_err(|e| permissions::map_resend_error("Failed to fetch sent emails", e))?;

    let sent_emails: Vec<SentEmail> = response
        .data
        .into_iter()
        .skip(off) // Skip previeous pages
        .take(lim) // Only take 'limit' items
        .map(|email| SentEmail {
            id: email.id.to_string(),
            from: email.from,
            to: email.to,
            subject: email.subject,
            html: None,
            created_at: email.created_at,
            cc: email.cc,
            bcc: email.bcc,
            reply_to: email.reply_to.as_ref().and_then(|rt| {
                if rt.is_empty() {
                    None
                } else {
                    Some(rt.join(", "))
                }
            }),
        })
        .collect();

    Ok(sent_emails)
}

#[tauri::command]
pub async fn get_sent_email(app: AppHandle<Wry>, email_id: String) -> Result<SentEmail, String> {
    // Bearer credential: API key or OAuth access token
    let credential = oauth::get_credential(&app).await?;

    let resend = Resend::new(&credential);

    // Get specific email by ID
    let email = resend
        .emails
        .get(&email_id)
        .await
        .map_err(|e| permissions::map_resend_error("Failed to fetch email", e))?;

    Ok(SentEmail {
        id: email.id.to_string(),
        from: email.from,
        to: email.to,
        subject: email.subject,
        html: email.html.or(email.text),
        created_at: email.created_at,
        cc: email.cc,
        bcc: email.bcc,
        reply_to: email.reply_to.as_ref().and_then(|rt| {
            if rt.is_empty() {
                None
            } else {
                Some(rt.join(", "))
            }
        }),
    })
}
