//! Sanitization for untrusted email HTML before rendering in the mail Shadow DOM.
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use ammonia::Builder;

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

// Preserve only known presentation attributes and escape values before markup insertion.
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
pub fn prepare_email_html(
    html: String,
    html_attributes: Option<HashMap<String, String>>,
    body_attributes: Option<HashMap<String, String>>,
) -> String {
    let html_attributes = root_attributes(html_attributes);
    let body_attributes = root_attributes(body_attributes);
    format!(
        "<html{html_attributes}><head></head><body{body_attributes}>{}</body></html>",
        sanitize_email_html(&html)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_active_content_and_preserves_safe_mail_markup() {
        let html = prepare_email_html(
            r#"<meta http-equiv="refresh" content="0;url=https://bad.test">
            <script>alert(1)</script><p onclick="alert(2)">Body</p>
            <iframe src="https://bad.test"></iframe>
            <a href="https://example.test/pay?a=1&amp;b=2">Pay</a>"#
                .into(),
            None,
            None,
        );
        for forbidden in [
            "<script",
            "alert(",
            "onclick",
            "<iframe",
            "http-equiv=\"refresh\"",
        ] {
            assert!(!html.contains(forbidden), "found {forbidden}");
        }
        assert!(html.contains("https://example.test/pay?a=1&amp;b=2"));
        assert!(html.contains("<body>"));
    }

    #[test]
    fn preserves_email_css_and_external_presentation_resources() {
        let html = prepare_email_html(
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
            "role=\"presentation\"",
        ] {
            assert!(html.contains(expected), "missing {expected}: {html}");
        }
        assert!(!html.contains("<script"));
    }

    #[test]
    fn preserves_root_presentation_but_rejects_unsafe_attributes() {
        let html = prepare_email_html(
            "<p>Body</p>".into(),
            Some(HashMap::from([
                ("lang".into(), "en".into()),
                ("class".into(), "brand".into()),
            ])),
            Some(HashMap::from([
                ("style".into(), "margin:0;background:#eee".into()),
                ("onload".into(), "alert(1)".into()),
                ("class".into(), "\"><script>alert(2)</script>".into()),
            ])),
        );
        assert!(html.contains("<html class=\"brand\" lang=\"en\">"));
        assert!(html.contains("<body style=\"margin:0;background:#eee\""));
        assert!(!html.contains("onload"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn unsafe_schemes_and_non_stylesheet_links_are_removed() {
        let html = sanitize_email_html(
            r#"<link rel="prefetch" href="https://example.test/page">
            <a href="data:text/html,active">Bad link</a>
            <link rel="stylesheet" href="javascript:alert(1)">"#,
        );
        assert!(!html.contains("rel=\"prefetch\""));
        assert!(!html.contains("data:text/html"));
        assert!(!html.contains("javascript:"));
    }
}
