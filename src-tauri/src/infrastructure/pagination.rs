//! Cursor pages preserve Resend's continuation signal instead of guessing from length.
use resend_rs::list_opts::ListOptions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageCache {
    pub downloaded_at: i64,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailPage<T> {
    pub items: Vec<T>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<PageCache>,
}

pub fn list_options(limit: Option<usize>, after: Option<&str>) -> Result<ListOptions, String> {
    let limit = limit.unwrap_or(12);
    if !(1..=100).contains(&limit) {
        return Err("[ERROR] Email page limit must be between 1 and 100".into());
    }
    if after.is_some_and(|cursor| cursor.trim().is_empty()) {
        return Err("[ERROR] Email cursor must not be empty".into());
    }
    Ok(ListOptions::default().with_limit(limit as u8))
}

pub fn email_page<T>(
    items: Vec<T>,
    has_more: bool,
    after: Option<&str>,
    id: impl Fn(&T) -> &str,
) -> Result<EmailPage<T>, String> {
    let next_cursor = if has_more {
        let last = items
            .last()
            .ok_or("[ERROR] Resend returned an empty page with more emails")?;
        let cursor = id(last);
        if cursor.is_empty() || Some(cursor) == after {
            return Err("[ERROR] Resend returned a non-advancing email cursor".into());
        }
        Some(cursor.to_string())
    } else {
        None
    };
    Ok(EmailPage {
        items,
        has_more,
        next_cursor,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_use_a_fixed_limit_and_exclusive_after_cursor() {
        let options = list_options(Some(16), Some("older-message")).unwrap();
        let query = serde_json::to_value(options.list_after("older-message")).unwrap();
        assert_eq!(query["limit"], 16);
        assert_eq!(query["after"], "older-message");
        assert!(query.get("offset").is_none());
        let first = serde_json::to_value(list_options(None, None).unwrap()).unwrap();
        assert_eq!(first["limit"], 12);
        assert!(first["after"].is_null());
    }

    #[test]
    fn invalid_limits_are_rejected_before_casting_to_u8() {
        for limit in [0, 101, 255, 256, usize::MAX] {
            assert!(list_options(Some(limit), None).is_err());
        }
        assert!(list_options(Some(100), None).is_ok());
        assert!(list_options(Some(16), Some(" ")).is_err());
    }

    #[test]
    fn api_continuation_is_not_inferred_from_page_length() {
        let short = email_page(vec!["a".to_string()], true, None, String::as_str).unwrap();
        assert!(short.has_more);
        assert_eq!(short.next_cursor.as_deref(), Some("a"));
        let full = email_page(vec!["a".to_string(); 100], false, None, String::as_str).unwrap();
        assert!(!full.has_more);
        assert!(full.next_cursor.is_none());
        let empty = email_page(Vec::<String>::new(), false, None, String::as_str).unwrap();
        assert!(empty.items.is_empty());
        let dto = serde_json::to_value(short).unwrap();
        assert_eq!(dto["hasMore"], true);
        assert_eq!(dto["nextCursor"], "a");
    }

    #[test]
    fn empty_or_repeated_continuation_is_an_error_not_a_silent_end_of_history() {
        assert!(email_page(Vec::<String>::new(), true, None, String::as_str).is_err());
        assert!(email_page(vec!["a".to_string()], true, Some("a"), String::as_str).is_err());
        assert!(email_page(vec!["".to_string()], true, None, String::as_str).is_err());
    }
}
