//! Account-owned list reads: local filtering, previews and offline fallback.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Wry};

use crate::infrastructure::{
    database::{
        local_state,
        mail::{self, LocalCursor, LocalQuery, Mailbox, Sort},
    },
    pagination::{EmailPage, PageCache},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListRequest {
    mailbox: Mailbox,
    limit: usize,
    after: Option<String>,
    domain: Option<String>,
    search: Option<String>,
    #[serde(default)]
    refresh: bool,
}

#[derive(Serialize)]
pub struct ListPage {
    #[serde(flatten)]
    page: EmailPage<Value>,
    domains: Vec<String>,
}

fn local_query(request: &ListRequest) -> Result<LocalQuery, String> {
    let after = request
        .after
        .as_deref()
        .map(|cursor| {
            if cursor.len() > 4096 {
                return Err("Invalid local cursor".to_string());
            }
            serde_json::from_str::<LocalCursor>(cursor)
                .map_err(|_| "Invalid local cursor".to_string())
        })
        .transpose()?;
    Ok(LocalQuery {
        mailbox: request.mailbox,
        limit: Some(request.limit),
        sort: Sort::Newest,
        domain: request.domain.clone(),
        search: request.search.clone(),
        recipient: None,
        unread_only: false,
        after,
    })
}

fn list_page(
    local: mail::LocalPage,
    domains: Vec<String>,
    remote_error: Option<String>,
) -> Result<ListPage, String> {
    if local.downloaded_at.is_none() {
        if let Some(error) = remote_error {
            return Err(error);
        }
    }
    let next_cursor = local
        .next_cursor
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|_| "Could not encode local cursor")?;
    Ok(ListPage {
        page: EmailPage {
            items: local.items,
            has_more: local.has_more,
            next_cursor,
            cache: local.downloaded_at.map(|downloaded_at| PageCache {
                downloaded_at,
                error: remote_error,
            }),
        },
        domains,
    })
}

#[tauri::command]
pub async fn load_mail_list(
    app: AppHandle<Wry>,
    account_id: String,
    request: ListRequest,
) -> Result<ListPage, String> {
    let database = local_state::for_account(&app, &account_id).await?;
    let query = local_query(&request)?;
    // Validate limits and cursor ownership before making any remote request.
    let mut local = mail::query(&database, account_id.clone(), query.clone()).await?;
    let mut remote_error = None;
    if request.refresh && request.after.is_none() {
        let refresh = match request.mailbox {
            Mailbox::Inbox => {
                crate::notifications::refresh_inbox(app.clone(), account_id.clone(), false).await
            }
            Mailbox::Sent => {
                crate::sent::list_sent_emails(app.clone(), account_id.clone(), Some(100), None)
                    .await
                    .and_then(|page| match page.cache.and_then(|cache| cache.error) {
                        Some(error) => Err(error),
                        None => Ok(()),
                    })
            }
        };
        match refresh {
            Ok(()) => {
                local = mail::query(&database, account_id.clone(), query).await?;
            }
            Err(error) => {
                remote_error = Some(error);
            }
        }
    }
    crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
    let domains = database
        .run(move |connection| {
            let mut statement = connection.prepare(
                "SELECT DISTINCT domain FROM email_domains WHERE account_id=?1 ORDER BY domain",
            )?;
            let rows = statement
                .query_map([account_id], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await?;
    list_page(local, domains, remote_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(downloaded_at: Option<i64>) -> mail::LocalPage {
        mail::LocalPage {
            items: vec![serde_json::json!({ "id": "retained" })],
            has_more: false,
            next_cursor: None,
            sync: None,
            is_partial: true,
            downloaded_at,
        }
    }

    #[test]
    fn offline_fallback_retains_mail_and_error_but_unknown_history_is_not_empty() {
        let page = list_page(
            local(Some(123)),
            vec!["a.example".into()],
            Some("offline".into()),
        )
        .unwrap();
        assert_eq!(page.page.items[0]["id"], "retained");
        let cache = page.page.cache.unwrap();
        assert_eq!(cache.downloaded_at, 123);
        assert_eq!(cache.error.as_deref(), Some("offline"));
        assert_eq!(page.domains, vec!["a.example"]);
        assert!(list_page(local(None), vec![], Some("offline".into())).is_err());
        assert!(list_page(local(None), vec![], None)
            .unwrap()
            .page
            .cache
            .is_none());
    }

    #[test]
    fn local_cursor_is_structured_and_not_a_remote_id() {
        let mut request = ListRequest {
            mailbox: Mailbox::Inbox,
            limit: 50,
            after: None,
            domain: Some("example.com".into()),
            search: Some("invoice".into()),
            refresh: false,
        };
        let query = local_query(&request).unwrap();
        assert_eq!(query.domain.as_deref(), Some("example.com"));
        assert_eq!(query.search.as_deref(), Some("invoice"));
        request.after = Some("remote-email-id".into());
        assert!(local_query(&request).is_err());
        request.after = Some("x".repeat(4097));
        assert!(local_query(&request).is_err());
    }
}
