//! The mail frame has no app origin or IPC; CSP permits only our link bridge.
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use ammonia::Builder;
use serde::Serialize;

fn set(items: &[&'static str]) -> HashSet<&'static str> {
    items.iter().copied().collect()
}

pub(crate) fn sanitize_email_html(html: &str) -> String {
    let tags = set(&[
        "a",
        "b",
        "blockquote",
        "br",
        "center",
        "code",
        "div",
        "em",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hr",
        "i",
        "img",
        "li",
        "ol",
        "p",
        "pre",
        "s",
        "span",
        "strong",
        "sub",
        "sup",
        "table",
        "tbody",
        "td",
        "tfoot",
        "th",
        "thead",
        "tr",
        "u",
        "ul",
        "font",
        "style",
        "link",
        "caption",
        "colgroup",
        "col",
        "dl",
        "dt",
        "dd",
        "small",
        "big",
        "strike",
    ]);
    let attributes: HashMap<&str, HashSet<&str>> = HashMap::from([
        ("a", set(&["href", "target"])),
        (
            "img",
            set(&["src", "srcset", "sizes", "alt", "width", "height"]),
        ),
        (
            "link",
            set(&["href", "rel", "type", "media", "crossorigin", "integrity"]),
        ),
        ("col", set(&["span", "width"])),
        ("colgroup", set(&["span", "width"])),
        ("font", set(&["color", "size", "face"])),
        (
            "table",
            set(&["border", "cellpadding", "cellspacing", "width"]),
        ),
        ("td", set(&["colspan", "rowspan", "width", "bgcolor"])),
        ("th", set(&["colspan", "rowspan", "width", "bgcolor"])),
    ]);
    Builder::default()
        .tags(tags)
        .rm_clean_content_tags(["style"])
        .add_clean_content_tags(["title"])
        .tag_attributes(attributes)
        .generic_attributes(set(&[
            "align",
            "valign",
            "dir",
            "lang",
            "id",
            "class",
            "style",
            "bgcolor",
            "background",
            "width",
            "height",
            "role",
            "aria-label",
            "aria-hidden",
        ]))
        .add_url_schemes(["data"])
        .attribute_filter(|tag, name, value| {
            if tag == "link" && name == "rel" {
                return value
                    .split_ascii_whitespace()
                    .any(|part| part.eq_ignore_ascii_case("stylesheet"))
                    .then_some(Cow::Borrowed("stylesheet"));
            }
            if tag == "a"
                && name == "href"
                && value.trim_start().to_ascii_lowercase().starts_with("data:")
            {
                return None;
            }
            Some(Cow::Borrowed(value))
        })
        .clean(html)
        .to_string()
}

#[derive(Serialize)]
pub struct EmailDocument {
    html: String,
    token: String,
}

// Ammonia parses fragments, so document-level attributes are supplied by DOMParser.
// Attribute names are fixed here; values are escaped, never interpolated as markup.
fn root_attributes(attributes: Option<HashMap<String, String>>) -> String {
    let mut output = String::new();
    if let Some(attributes) = attributes {
        for name in [
            "style",
            "class",
            "id",
            "dir",
            "lang",
            "bgcolor",
            "background",
            "text",
            "link",
            "vlink",
            "alink",
            "marginwidth",
            "marginheight",
            "topmargin",
            "leftmargin",
            "bottommargin",
            "rightmargin",
        ] {
            if let Some(value) = attributes.get(name) {
                let value = value
                    .replace('&', "&amp;")
                    .replace('"', "&quot;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                output.push_str(&format!(" {name}=\"{value}\""));
            }
        }
    }
    output
}

#[tauri::command]
pub fn prepare_email_document(
    html: String,
    html_attributes: Option<HashMap<String, String>>,
    body_attributes: Option<HashMap<String, String>>,
) -> EmailDocument {
    let token = uuid::Uuid::new_v4().to_string();
    let body = sanitize_email_html(&html);
    let bridge = include_str!("email-links.js").replace("__ONLYSEND_TOKEN__", &token);
    // shortcut: remote presentation loads automatically; add consent controls if requested.
    // Sender scripts and app-origin access stay blocked.
    let csp = format!(
        "default-src 'none'; script-src 'nonce-{token}'; script-src-attr 'none'; \
         style-src 'unsafe-inline' https: http: data:; img-src https: http: data:; \
         font-src https: http: data:; \
         base-uri 'none'; form-action 'none'"
    );
    let html_attributes = root_attributes(html_attributes);
    let body_attributes = root_attributes(body_attributes);
    let html = format!(
        "<!doctype html><html{html_attributes}><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">\
         <meta name=\"referrer\" content=\"no-referrer\">\
         <script nonce=\"{token}\">{bridge}</script></head><body{body_attributes}>{body}</body></html>"
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
            None, None,
        );
        assert_eq!(document.html.matches("<script ").count(), 1);
        assert_eq!(
            document
                .html
                .matches("http-equiv=\"Content-Security-Policy\"")
                .count(),
            1
        );
        assert!(document
            .html
            .contains(&format!("script-src 'nonce-{}'", document.token)));
        assert!(!document.html.contains("__ONLYSEND_TOKEN__"));
        for forbidden in [
            "alert(",
            "onclick",
            "<base",
            "<iframe",
            "http-equiv=\"refresh\"",
        ] {
            assert!(!document.html.contains(forbidden), "found {forbidden}");
        }
        assert!(document
            .html
            .contains("https://example.com/pay?a=1&amp;b=2"));
    }

    #[test]
    fn document_title_does_not_leak_into_the_body_or_remove_matching_headings() {
        let document = prepare_email_document(
            r#"<!doctype html><html><head><title>Verify your email address</title>
            <style>h1 { color: #ff6633; }</style></head><body>
            <h1>Verify your email address</h1><p>Message content</p></body></html>"#
                .into(),
            None,
            None,
        );
        assert!(!document.html.contains("<title"));
        assert_eq!(
            document.html.matches("Verify your email address").count(),
            1
        );
        assert!(document.html.contains("<h1>Verify your email address</h1>"));
        assert!(document.html.contains("h1 { color: #ff6633; }"));
        assert!(document.html.contains("Message content"));
    }

    #[test]
    fn buttons_and_fragment_targets_survive_without_active_content() {
        let html = sanitize_email_html(
            r#"<table width="600"><tr><td id="payment">
            <a href="https://example.com/pay">Pay online</a></td></tr></table>"#,
        );
        for expected in [
            "id=\"payment\"",
            "width=\"600\"",
            "https://example.com/pay",
            "Pay online",
        ] {
            assert!(html.contains(expected), "missing {expected}: {html}");
        }
    }

    #[test]
    fn preserves_payment_button_css_images_and_external_presentation_resources() {
        let document = prepare_email_document(
            include_str!("../tests/fixtures/styled-email.html").into(),
            None,
            None,
        );
        for expected in [
            "<style>",
            "@media",
            "@import",
            "@font-face",
            "class=\"payment-button\"",
            "display:inline-block",
            "padding:14px 24px",
            "Pay online",
            "https://example.com/email.css",
            "https://example.com/brand.woff2",
            "https://example.com/background.png",
            "srcset=",
            "data:image/png;base64,",
            "style-src 'unsafe-inline' https: http: data:",
            "font-src https: http: data:",
            "img-src https: http: data:",
            "role=\"presentation\"",
        ] {
            assert!(
                document.html.contains(expected),
                "missing {expected}: {}",
                document.html
            );
        }
        assert!(document.html.contains("script-src-attr 'none'"));
        assert_eq!(document.html.matches("<script ").count(), 1);
    }

    #[test]
    fn root_presentation_is_preserved_but_attributes_cannot_inject_code() {
        let document = prepare_email_document(
            "<p>Body</p>".into(),
            Some(HashMap::from([
                ("lang".into(), "en".into()),
                ("class".into(), "brand".into()),
            ])),
            Some(HashMap::from([
                ("style".into(), "margin:0;background:#eee".into()),
                ("onload".into(), "alert(1)".into()),
                (
                    "class".into(),
                    "\"><script nonce=\"injected\">alert(2)</script>".into(),
                ),
            ])),
        );
        assert!(document.html.contains("<html class=\"brand\" lang=\"en\">"));
        assert!(document
            .html
            .contains("<body style=\"margin:0;background:#eee\""));
        assert!(!document.html.contains("onload"));
        assert!(!document.html.contains("<script nonce=\"injected\""));
        assert_eq!(document.html.matches("<script ").count(), 1);
    }

    #[test]
    fn non_stylesheet_links_and_data_navigation_are_not_enabled() {
        let html = sanitize_email_html(
            r#"<link rel="prefetch" href="https://example.com/page">
            <a href="data:text/html,active">Bad link</a>
            <link rel="stylesheet" href="javascript:alert(1)">"#,
        );
        assert!(!html.contains("rel=\"prefetch\""));
        assert!(!html.contains("data:text/html"));
        assert!(!html.contains("javascript:"));
    }

    #[test]
    fn each_document_has_a_different_bridge_identity() {
        assert_ne!(
            prepare_email_document("one".into(), None, None).token,
            prepare_email_document("two".into(), None, None).token
        );
    }
}
