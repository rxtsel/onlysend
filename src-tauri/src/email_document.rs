//! The mail frame has no app origin or IPC; CSP permits only our link bridge.
use std::collections::{HashMap, HashSet};

use ammonia::Builder;
use serde::Serialize;

fn set(items: &[&'static str]) -> HashSet<&'static str> {
    items.iter().copied().collect()
}

pub(crate) fn sanitize_email_html(html: &str) -> String {
    let tags = set(&[
        "a", "b", "blockquote", "br", "center", "code", "div", "em", "h1", "h2", "h3",
        "h4", "h5", "h6", "hr", "i", "img", "li", "ol", "p", "pre", "s", "span", "strong",
        "sub", "sup", "table", "tbody", "td", "tfoot", "th", "thead", "tr", "u", "ul", "font",
    ]);
    let attributes: HashMap<&str, HashSet<&str>> = HashMap::from([
        ("a", set(&["href", "target"])),
        ("img", set(&["src", "alt", "width", "height"])),
        ("font", set(&["color", "size", "face"])),
        ("table", set(&["border", "cellpadding", "cellspacing", "width"])),
        ("td", set(&["colspan", "rowspan", "width", "bgcolor"])),
        ("th", set(&["colspan", "rowspan", "width", "bgcolor"])),
    ]);
    Builder::default()
        .tags(tags)
        .tag_attributes(attributes)
        .generic_attributes(set(&["align", "valign", "dir", "lang", "id"]))
        .clean(html)
        .to_string()
}

#[derive(Serialize)]
pub struct EmailDocument {
    html: String,
    token: String,
}

#[tauri::command]
pub fn prepare_email_document(html: String) -> EmailDocument {
    let token = uuid::Uuid::new_v4().to_string();
    let body = sanitize_email_html(&html);
    let bridge = include_str!("email-links.js").replace("__ONLYSEND_TOKEN__", &token);
    // Remote images retain the existing behavior until the privacy setting lands.
    // No remote script, stylesheet, frame, font, form, base URL or network API.
    let csp = format!(
        "default-src 'none'; script-src 'nonce-{token}'; script-src-attr 'none'; \
         style-src 'unsafe-inline'; img-src https: http: data:; \
         base-uri 'none'; form-action 'none'"
    );
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">\
         <meta name=\"referrer\" content=\"no-referrer\">\
         <script nonce=\"{token}\">{bridge}</script></head><body>{body}</body></html>"
    );
    EmailDocument { html, token }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_app_bridge_gets_a_nonce_and_senders_cannot_replace_the_policy() {
        let document = prepare_email_document(
            r#"<meta http-equiv="refresh" content="0;url=https://evil.test">
            <meta http-equiv="Content-Security-Policy" content="script-src * 'unsafe-inline'">
            <base href="https://evil.test"><script nonce="sender">alert(1)</script>
            <p onclick="alert(2)">Body</p><iframe src="https://evil.test"></iframe>
            <a href="https://example.com/pay?a=1&amp;b=2" target="_blank"><img src="https://example.com/logo.png"></a>"#.into(),
        );
        assert_eq!(document.html.matches("<script ").count(), 1);
        assert_eq!(document.html.matches("http-equiv=\"Content-Security-Policy\"").count(), 1);
        assert!(document.html.contains(&format!("script-src 'nonce-{}'", document.token)));
        assert!(!document.html.contains("__ONLYSEND_TOKEN__"));
        for forbidden in ["alert(", "onclick", "<base", "<iframe", "http-equiv=\"refresh\""] {
            assert!(!document.html.contains(forbidden), "found {forbidden}");
        }
        assert!(document.html.contains("https://example.com/pay?a=1&amp;b=2"));
    }

    #[test]
    fn buttons_and_fragment_targets_survive_without_active_content() {
        let html = sanitize_email_html(
            r#"<table width="600"><tr><td id="payment">
            <a href="https://example.com/pay">Pay online</a></td></tr></table>"#,
        );
        for expected in ["id=\"payment\"", "width=\"600\"", "https://example.com/pay", "Pay online"] {
            assert!(html.contains(expected), "missing {expected}: {html}");
        }
    }

    #[test]
    fn each_document_has_a_different_bridge_identity() {
        assert_ne!(prepare_email_document("one".into()).token, prepare_email_document("two".into()).token);
    }
}
