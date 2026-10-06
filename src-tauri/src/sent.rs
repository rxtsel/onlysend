use crate::infrastructure::database::{
    local_state,
    mail::{self, Mailbox},
    sync,
};
use crate::infrastructure::pagination::{email_page, list_options, EmailPage};
use crate::oauth;
use crate::permissions;
use resend_rs::Resend;
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
    account_id: String,
    limit: Option<usize>,
    after: Option<String>,
) -> Result<EmailPage<SentEmail>, String> {
    let _ = list_options(limit, after.as_deref())?;
    let database = local_state::for_account(&app, &account_id).await?;
    let result = async {
        let credential = oauth::get_account_credential(&app, &account_id).await?;
        let resend = Resend::new(&credential);
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            fetch_sent_page(&resend, limit, after.as_deref()),
        )
        .await
        .map_err(|_| "Sent request timed out".to_string())?
    }
    .await;
    match result {
        Ok(page) => {
            crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
            mail::store_page(
                &database,
                account_id.clone(),
                Mailbox::Sent,
                limit.unwrap_or(12),
                after.clone(),
                &page,
            )
            .await?;
            if after.is_none() {
                if sync::start(&app, account_id, Mailbox::Sent, limit.unwrap_or(12), false)
                    .await
                    .is_err()
                {
                    eprintln!("[WARN] Could not start sent archive download");
                }
            }
            Ok(page)
        }
        Err(error) => mail::cached_page(
            &database,
            account_id,
            Mailbox::Sent,
            limit.unwrap_or(12),
            after,
            Some(error.clone()),
        )
        .await?
        .ok_or(error),
    }
}

pub(crate) async fn fetch_sent_page(
    resend: &Resend,
    limit: Option<usize>,
    after: Option<&str>,
) -> Result<EmailPage<SentEmail>, String> {
    let options = list_options(limit, after)?;
    // Each request fetches one remote page, retaining the captured account.
    let response = match after {
        Some(cursor) => resend.emails.list(options.list_after(cursor)).await,
        None => resend.emails.list(options).await,
    }
    .map_err(|e| permissions::map_resend_error("Failed to fetch sent emails", e))?;
    let has_more = response.has_more;

    let sent_emails: Vec<SentEmail> = response
        .data
        .into_iter()
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

    email_page(sent_emails, has_more, after, |email| &email.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::pagination_test_server::mock_history;

    #[tokio::test]
    async fn sent_sdk_pages_traverse_history_beyond_previous_limits() {
        let (resend, server, expected) = mock_history("/emails");
        let mut after = None;
        let mut ids = Vec::new();
        for _ in 0..4 {
            let page = fetch_sent_page(&resend, Some(100), after.as_deref())
                .await
                .unwrap();
            ids.extend(page.items.iter().map(|email| email.id.clone()));
            assert_eq!(page.has_more, ids.len() < expected.len());
            after = page.next_cursor;
        }
        server.join().unwrap();
        assert_eq!(ids, expected);
        assert!(after.is_none());
    }
}

#[tauri::command]
pub async fn get_sent_email(
    app: AppHandle<Wry>,
    account_id: String,
    email_id: String,
) -> Result<SentEmail, String> {
    let database = local_state::for_account(&app, &account_id).await?;
    if let Some(json) = mail::cached_detail(
        &database,
        account_id.clone(),
        "sent".into(),
        email_id.clone(),
    )
    .await?
    {
        return serde_json::from_str(&json).map_err(|_| "Could not read downloaded email".into());
    }
    // Bearer credential: API key or OAuth access token
    let credential = oauth::get_account_credential(&app, &account_id).await?;

    let resend = Resend::new(&credential);

    // Get specific email by ID
    let email = resend
        .emails
        .get(&email_id)
        .await
        .map_err(|e| permissions::map_resend_error("Failed to fetch email", e))?;

    let search_text = email
        .text
        .clone()
        .unwrap_or_else(|| mail::plain_html(email.html.as_deref().unwrap_or_default()));
    let detail = SentEmail {
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
    };
    crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
    mail::store_detail(
        &database,
        account_id,
        Mailbox::Sent,
        email_id,
        &detail,
        search_text,
    )
    .await?;
    Ok(detail)
}
