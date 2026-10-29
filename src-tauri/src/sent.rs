use crate::infrastructure::database::{
    local_state,
    mail::{self, Mailbox},
    sync,
};
use crate::infrastructure::pagination::{email_page, list_options, EmailPage};
use crate::oauth;
use crate::permissions;
use resend_rs::{ConfigBuilder, Resend};
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
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub body_format_version: u8,
    #[serde(default)]
    pub attachments: Vec<crate::attachments::AttachmentInfo>,
    pub created_at: String,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    pub reply_to: Option<String>,
    #[serde(default)]
    pub last_event: Option<String>,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub message_id: Option<String>,
}

impl SentEmail {
    fn can_reuse(&self) -> bool {
        self.body_format_version > 0 && (self.thread_id.is_some() || self.body_format_version >= 2)
    }
}

// Keep last_event as a string: the SDK's closed enum rejects suppressed.
#[derive(Debug, Deserialize)]
struct RemoteSentEmail {
    id: String,
    from: String,
    to: Vec<String>,
    subject: String,
    created_at: String,
    html: Option<String>,
    text: Option<String>,
    cc: Option<Vec<String>>,
    bcc: Option<Vec<String>>,
    reply_to: Option<Vec<String>>,
    last_event: Option<String>,
    #[serde(default)]
    message_id: Option<String>,
    #[serde(default)]
    headers: Option<std::collections::HashMap<String, String>>,
}

fn sent_reply_thread_id(email: &RemoteSentEmail) -> Option<String> {
    let header = |name: &str| {
        email
            .headers
            .as_ref()?
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    };
    header("References")
        .and_then(crate::inbound::parsed_message_ids)
        .and_then(|ids| ids.first().cloned())
        .or_else(|| {
            header("In-Reply-To")
                .and_then(crate::inbound::parsed_message_ids)
                .and_then(|ids| ids.first().cloned())
        })
}

fn sent_message_id(email: &RemoteSentEmail) -> Option<String> {
    email
        .message_id
        .as_deref()
        .and_then(crate::inbound::parsed_message_ids)
        .and_then(|ids| ids.first().map(|id| format!("<{id}>")))
        .or_else(|| {
            email
                .headers
                .as_ref()?
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("Message-ID"))
                .and_then(|(_, value)| crate::inbound::parsed_message_ids(value))
                .and_then(|ids| ids.first().map(|id| format!("<{id}>")))
        })
}

fn sent_thread_id(email: &RemoteSentEmail) -> Option<String> {
    sent_reply_thread_id(email).or_else(|| {
        sent_message_id(email)
            .as_deref()
            .and_then(crate::inbound::parsed_message_ids)
            .and_then(|ids| ids.first().cloned())
    })
}

impl From<RemoteSentEmail> for SentEmail {
    fn from(email: RemoteSentEmail) -> Self {
        let thread_id = sent_thread_id(&email);
        let message_id = sent_message_id(&email);
        Self {
            id: email.id,
            from: email.from,
            to: email.to,
            subject: email.subject,
            html: email.html,
            text: email.text,
            // v2 distinguishes absent remote thread metadata from an old cache missing it.
            body_format_version: 2,
            attachments: Vec::new(),
            created_at: email.created_at,
            cc: email.cc.unwrap_or_default(),
            bcc: email.bcc.unwrap_or_default(),
            last_event: email.last_event,
            reply_to: email
                .reply_to
                .filter(|rt| !rt.is_empty())
                .map(|rt| rt.join(", ")),
            thread_id,
            message_id,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RemoteSentPage {
    data: Vec<RemoteSentEmail>,
    has_more: bool,
}

async fn sent_response<T: serde::de::DeserializeOwned>(
    resend: &Resend,
    request: reqwest::RequestBuilder,
    context: &str,
) -> Result<T, String> {
    // Keep the SDK's HTTP error mapping and rate limiting, but decode our own DTO.
    let config = ConfigBuilder::new(resend.api_key())
        .client(resend.client())
        .build();
    let response = config
        .send(
            request
                .bearer_auth(resend.api_key())
                .header("User-Agent", resend.user_agent()),
        )
        .await
        .map_err(|e| permissions::map_resend_error(context, e))?;
    response
        .json()
        .await
        .map_err(|_| format!("[ERROR] {context}: incompatible email response"))
}

pub(crate) async fn list_sent_emails(
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
    let request = resend.client().get(format!("{}emails", resend.base_url()));
    let request = match after {
        Some(cursor) => request.query(&options.list_after(cursor)),
        None => request.query(&options),
    };
    let response: RemoteSentPage =
        sent_response(resend, request, "Failed to fetch sent emails").await?;
    let has_more = response.has_more;

    let sent_emails: Vec<SentEmail> = response
        .data
        .into_iter()
        .map(|email| SentEmail {
            html: None,
            text: None,
            ..email.into()
        })
        .collect();

    email_page(sent_emails, has_more, after, |email| &email.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::pagination_test_server::mock_history;

    #[test]
    fn sent_reply_headers_map_to_the_oldest_message_in_the_conversation() {
        let email: RemoteSentEmail = serde_json::from_value(serde_json::json!({
            "id":"email-id", "from":"sender@example.test", "to":["recipient@example.test"],
            "subject":"Re: Subject", "created_at":"2026-01-01T00:00:00Z",
            "message_id":"<reply@example.test>", "headers":{
                "In-Reply-To":"<parent@example.test>",
                "References":"<root@example.test> <parent@example.test>"
            }
        }))
        .unwrap();
        assert_eq!(sent_thread_id(&email).as_deref(), Some("root@example.test"));
    }

    #[test]
    fn sent_responses_accept_suppressed_and_future_events_with_nullable_fields() {
        for event in ["suppressed", "future_event", "delivered"] {
            let value = serde_json::json!({
                "id": "01a17f8f-b52b-7b2e-847a-609984c9bd58",
                "from": "Support <support@example.com>",
                "to": ["recipient@example.com"],
                "subject": "Test Email",
                "created_at": "2026-10-27 22:51:27.060000+00",
                "last_event": event,
                "message_id": null,
                "cc": null, "bcc": null, "reply_to": null,
                "html": null, "text": "Plain text body"
            });
            let email: RemoteSentEmail = serde_json::from_value(value.clone()).unwrap();
            let detail: SentEmail = email.into();
            assert!(detail.html.is_none());
            assert_eq!(detail.text.as_deref(), Some("Plain text body"));
            assert_eq!(detail.last_event.as_deref(), Some(event));
            assert_eq!(serde_json::to_value(&detail).unwrap()["lastEvent"], event);
            assert!(detail.cc.is_empty());
            assert!(detail.bcc.is_empty());
            assert!(detail.reply_to.is_none());
            let page: RemoteSentPage = serde_json::from_value(serde_json::json!({
                "data": [value], "has_more": false
            }))
            .unwrap();
            assert_eq!(page.data.len(), 1);
            assert!(!page.has_more);
        }
    }

    #[test]
    fn plain_text_keeps_newlines_and_literal_markup_and_legacy_cache_is_detectable() {
        let text = "Hello <team>\n\nUse a < b & keep <script> as text.";
        let remote: RemoteSentEmail = serde_json::from_value(serde_json::json!({
            "id":"email", "from":"sender@example.com", "to":["to@example.com"],
            "subject":"Plain", "created_at":"2026-01-01T00:00:00Z",
            "html":null, "text":text, "cc":null, "bcc":null, "reply_to":null, "last_event":null
        }))
        .unwrap();
        let detail: SentEmail = remote.into();
        assert!(detail.html.is_none());
        assert_eq!(detail.text.as_deref(), Some(text));
        assert_eq!(detail.body_format_version, 2);
        assert!(detail.thread_id.is_none());
        assert!(detail.can_reuse());
        let mut older = detail.clone();
        older.body_format_version = 1;
        assert!(!older.can_reuse());
        older.thread_id = Some("root@example.test".into());
        assert!(older.can_reuse());
        let mut legacy = serde_json::to_value(&detail).unwrap();
        legacy.as_object_mut().unwrap().remove("bodyFormatVersion");
        legacy.as_object_mut().unwrap().remove("text");
        let cached: SentEmail = serde_json::from_value(legacy).unwrap();
        assert_eq!(cached.body_format_version, 0);
        assert!(!cached.can_reuse());
    }

    #[test]
    fn sent_response_preserves_recipients_and_requires_core_fields() {
        let value = serde_json::json!({
            "id": "email-id", "from": "sender@example.com", "to": ["to@example.com"],
            "subject": "Test", "created_at": "2026-10-27 22:51:27.060000+00",
            "html": "<p>Body</p>", "text": "Body",
            "cc": ["cc@example.com"], "bcc": ["bcc@example.com"],
            "reply_to": ["first@example.com", "second@example.com"]
        });
        let detail: SentEmail = serde_json::from_value::<RemoteSentEmail>(value.clone())
            .unwrap()
            .into();
        assert_eq!(detail.html.as_deref(), Some("<p>Body</p>"));
        assert!(detail.last_event.is_none());
        let mut cached = serde_json::to_value(&detail).unwrap();
        cached.as_object_mut().unwrap().remove("lastEvent");
        assert!(serde_json::from_value::<SentEmail>(cached)
            .unwrap()
            .last_event
            .is_none());
        assert_eq!(detail.cc, ["cc@example.com"]);
        assert_eq!(detail.bcc, ["bcc@example.com"]);
        assert_eq!(
            detail.reply_to.as_deref(),
            Some("first@example.com, second@example.com")
        );
        let mut invalid = value;
        invalid.as_object_mut().unwrap().remove("id");
        assert!(serde_json::from_value::<RemoteSentEmail>(invalid).is_err());
        assert!(serde_json::from_value::<RemoteSentPage>(serde_json::json!({"data": []})).is_err());
    }

    #[tokio::test]
    async fn sent_pages_traverse_history_beyond_previous_limits() {
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
    mail::ensure_visible(&database, account_id.clone(), Mailbox::Sent, email_id.clone()).await?;
    download_sent_email(app, account_id, email_id).await
}

pub(crate) async fn download_sent_email(
    app: AppHandle<Wry>,
    account_id: String,
    email_id: String,
) -> Result<SentEmail, String> {
    let database = local_state::for_account(&app, &account_id).await?;
    let cached = mail::cached_detail(
        &database,
        account_id.clone(),
        "sent".into(),
        email_id.clone(),
    )
    .await?
    .map(|json| {
        serde_json::from_str::<SentEmail>(&json)
            .map_err(|_| "Could not read downloaded email".to_string())
    })
    .transpose()?;
    if let Some(detail) = cached
        .as_ref()
        .filter(|detail| detail.can_reuse())
    {
        if let Some(thread_id) = &detail.thread_id {
            if mail::thread_id_for(
                &database,
                account_id.clone(),
                Mailbox::Sent,
                email_id.clone(),
            )
            .await?
            .is_none()
            {
                mail::store_thread_link(
                    &database,
                    account_id.clone(),
                    Mailbox::Sent,
                    email_id.clone(),
                    thread_id.clone(),
                )
                .await?;
            }
        }
        return Ok(detail.clone());
    }
    // Legacy cache merged text into HTML. Recover the source without deleting offline mail.
    let downloaded = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        // Bearer credential: API key or OAuth access token
        let credential = oauth::get_account_credential(&app, &account_id).await?;

        let resend = Resend::new(&credential);

        // Get specific email by ID
        let mut url = reqwest::Url::parse(&format!("{}emails", resend.base_url()))
            .map_err(|_| "[ERROR] Invalid Resend base URL".to_string())?;
        url.path_segments_mut()
            .map_err(|_| "[ERROR] Invalid Resend base URL".to_string())?
            .push(&email_id);
        let email: RemoteSentEmail =
            sent_response(&resend, resend.client().get(url), "Failed to fetch email").await?;

        let search_text = email
            .text
            .clone()
            .unwrap_or_else(|| mail::plain_html(email.html.as_deref().unwrap_or_default()));
        let reply_thread_id = sent_reply_thread_id(&email);
        let detail: SentEmail = email.into();
        let thread_id = detail.thread_id.clone();
        crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
        mail::store_detail(
            &database,
            account_id.clone(),
            Mailbox::Sent,
            email_id.clone(),
            &detail,
            search_text,
        )
        .await?;
        if let Some(thread_id) = reply_thread_id {
            mail::store_thread_link(
                &database,
                account_id.clone(),
                Mailbox::Sent,
                email_id.clone(),
                thread_id,
            )
            .await?;
        } else if let Some(thread_id) = thread_id {
            if mail::thread_id_for(
                &database,
                account_id.clone(),
                Mailbox::Sent,
                email_id.clone(),
            )
            .await?
            .is_none()
            {
                mail::store_thread_link(
                    &database,
                    account_id.clone(),
                    Mailbox::Sent,
                    email_id.clone(),
                    thread_id,
                )
                .await?;
            }
        }
        Ok::<_, String>(detail)
    })
    .await
    .unwrap_or_else(|_| Err("Sent email download timed out".into()));
    match downloaded {
        Ok(detail) => Ok(detail),
        Err(error) => match cached {
            Some(detail) => {
                crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
                Ok(detail)
            }
            None => Err(error),
        },
    }
}
