//! Attachment URLs never cross IPC. Only authenticated metadata and bounded file data do.
use crate::{
    infrastructure::database::{
        local_state,
        mail::{self, Mailbox},
    },
    oauth, permissions,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use resend_rs::{
    list_opts::ListOptions,
    types::{Attachment, ContentDisposition},
    Resend,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::Path,
    sync::Mutex,
    time::Duration,
};
use tauri::{ipc::Channel, AppHandle, State, Wry};
use tauri_plugin_dialog::DialogExt;
use tempfile::{NamedTempFile, TempDir};
use tokio::io::AsyncWriteExt;

pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
const MAX_PREVIEW_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentInfo {
    pub id: String,
    pub filename: Option<String>,
    pub content_type: String,
    pub size: Option<u32>,
    #[serde(default)]
    pub content_id: Option<String>,
    #[serde(default)]
    pub content_disposition: Option<String>,
}
impl From<&Attachment> for AttachmentInfo {
    fn from(a: &Attachment) -> Self {
        Self {
            id: a.id.to_string(),
            filename: a.filename.clone(),
            content_type: a.content_type.clone(),
            size: Some(a.size),
            content_id: a.content_id.clone(),
            content_disposition: Some(
                if matches!(a.content_disposition, ContentDisposition::Inline) {
                    "inline"
                } else {
                    "attachment"
                }
                .into(),
            ),
        }
    }
}

#[derive(Default)]
pub struct AttachmentTransfers {
    active: Mutex<HashMap<(String, String), tokio::sync::watch::Sender<bool>>>,
    // Keep files alive while external applications may still be reading them.
    // shortcut: copies live until app exit; add a bounded cache if long sessions accumulate files.
    opened: Mutex<Vec<TempDir>>,
}
impl AttachmentTransfers {
    // Once an atomic save/external launch starts, cancellation must not report a false rollback.
    fn begin_commit(
        &self,
        key: &(String, String),
        cancel: &tokio::sync::watch::Sender<bool>,
    ) -> Result<bool, String> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| "Attachment transfer lock failed")?;
        let proceed = !*cancel.borrow();
        active.remove(key);
        Ok(proceed)
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AttachmentAction {
    Preview,
    Save,
    Open,
}
#[derive(Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum AttachmentResult {
    Preview(String),
    Saved,
    Opened,
    Cancelled,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    downloaded: u64,
    total: Option<u64>,
}

fn validate_id(value: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| "Invalid attachment or email ID".into())
}
fn safe_filename(value: Option<&str>) -> String {
    let mut name: String = value
        .unwrap_or("attachment")
        .chars()
        .map(|ch| {
            if ch.is_control()
                || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
                || matches!(ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            {
                '_'
            } else {
                ch
            }
        })
        .collect();
    name = name.trim_matches([' ', '.']).to_owned();
    while name.len() > 180 {
        name.pop();
    }
    if name.is_empty() {
        name = "attachment".into();
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        name.insert(0, '_');
    }
    name
}
// shortcut: only Resend's documented CDNs are allowed; update if their storage hosts change.
fn download_url(value: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "Invalid attachment download URL")?;
    if url.scheme() != "https"
        || !matches!(
            url.host_str(),
            Some("inbound-cdn.resend.com" | "outbound-cdn.resend.com")
        )
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return Err(format!(
            "Unsupported attachment download server: {}",
            url.host_str().unwrap_or("no hostname")
        ));
    }
    Ok(url)
}
fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}
fn openable(name: &str, bytes: &[u8]) -> bool {
    let extension = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" => image_mime(bytes) == Some("image/png"),
        "jpg" | "jpeg" => image_mime(bytes) == Some("image/jpeg"),
        "gif" => image_mime(bytes) == Some("image/gif"),
        "webp" => image_mime(bytes) == Some("image/webp"),
        "pdf" => bytes.starts_with(b"%PDF-"),
        "zip" | "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" => {
            bytes.starts_with(b"PK\x03\x04")
        }
        _ => false,
    }
}
async fn metadata(
    resend: &Resend,
    mailbox: Mailbox,
    email_id: &str,
    attachment_id: &str,
) -> Result<Attachment, String> {
    let request = async {
        match mailbox {
            Mailbox::Inbox => {
                resend
                    .receiving
                    .get_attachment(attachment_id, email_id)
                    .await
            }
            Mailbox::Sent => resend.emails.get_attachment(email_id, attachment_id).await,
        }
    };
    tokio::time::timeout(Duration::from_secs(30), request)
        .await
        .map_err(|_| "Attachment request timed out".to_string())?
        .map_err(|error| permissions::map_resend_error("Could not retrieve attachment", error))
}

#[tauri::command]
pub async fn list_email_attachments(
    app: AppHandle<Wry>,
    account_id: String,
    mailbox: Mailbox,
    email_id: String,
) -> Result<Vec<AttachmentInfo>, String> {
    validate_id(&email_id)?;
    let database = local_state::for_account(&app, &account_id).await?;
    let credential = oauth::get_account_credential(&app, &account_id).await?;
    let resend = Resend::new(&credential);
    let mut after: Option<String> = None;
    let mut seen = HashSet::new();
    let mut ids = HashSet::new();
    let mut items = Vec::new();
    loop {
        let options = ListOptions::default().with_limit(100);
        let request = async {
            match (mailbox, after.as_deref()) {
                (Mailbox::Inbox, Some(cursor)) => {
                    resend
                        .receiving
                        .list_attachments(&email_id, options.list_after(cursor))
                        .await
                }
                (Mailbox::Inbox, None) => {
                    resend.receiving.list_attachments(&email_id, options).await
                }
                (Mailbox::Sent, Some(cursor)) => {
                    resend
                        .emails
                        .list_attachments(&email_id, options.list_after(cursor))
                        .await
                }
                (Mailbox::Sent, None) => resend.emails.list_attachments(&email_id, options).await,
            }
        };
        let page = tokio::time::timeout(Duration::from_secs(30), request)
            .await
            .map_err(|_| "Attachment list timed out".to_string())?
            .map_err(|error| permissions::map_resend_error("Could not list attachments", error))?;
        let cursor = page.data.last().map(|a| a.id.to_string());
        items.extend(
            page.data
                .iter()
                .filter(|item| ids.insert(item.id.to_string()))
                .map(AttachmentInfo::from),
        );
        if !page.has_more {
            break;
        }
        let cursor = cursor.ok_or("Attachment list has no continuation")?;
        if !seen.insert(cursor.clone()) {
            return Err("Attachment list cursor repeated".into());
        }
        after = Some(cursor);
    }
    crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
    mail::store_attachments(&database, account_id, mailbox, email_id, &items).await?;
    Ok(items)
}

#[tauri::command]
pub fn cancel_attachment_download(
    state: State<'_, AttachmentTransfers>,
    account_id: String,
    request_id: String,
) -> Result<(), String> {
    if let Some(cancel) = state
        .active
        .lock()
        .map_err(|_| "Attachment transfer lock failed")?
        .get(&(account_id, request_id))
    {
        let _ = cancel.send(true);
    }
    Ok(())
}

async fn download(
    resend: &Resend,
    mailbox: Mailbox,
    email_id: &str,
    attachment_id: &str,
    mut attachment: Attachment,
    target: &NamedTempFile,
    limit: u64,
    cancel: &tokio::sync::watch::Sender<bool>,
    progress: &Channel<DownloadProgress>,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| "Could not initialize attachment download")?;
    let mut response = None;
    for attempt in 0..2 {
        if *cancel.borrow() {
            return Err("Download cancelled".into());
        }
        if u64::from(attachment.size) > limit {
            return Err(format!(
                "This action supports files up to {} MB",
                limit / 1024 / 1024
            ));
        }
        let result = client
            .get(download_url(&attachment.download_url)?)
            .send()
            .await
            .map_err(|_| "Could not download attachment. Check your connection.")?;
        if result.status() == reqwest::StatusCode::FORBIDDEN && attempt == 0 {
            attachment = metadata(resend, mailbox, email_id, attachment_id).await?;
            continue;
        }
        if !result.status().is_success() {
            return Err(format!(
                "Attachment server returned {}",
                result.status().as_u16()
            ));
        }
        response = Some(result);
        break;
    }
    let response = response.ok_or("Attachment download link expired")?;
    write_response(
        response,
        target,
        u64::from(attachment.size),
        limit,
        progress,
    )
    .await
}

async fn write_response(
    mut response: reqwest::Response,
    target: &NamedTempFile,
    expected: u64,
    limit: u64,
    progress: &Channel<DownloadProgress>,
) -> Result<(), String> {
    if expected > limit
        || response
            .content_length()
            .is_some_and(|length| length > limit)
    {
        return Err("Attachment is too large".into());
    }
    let total = Some(expected);
    let mut last_progress = std::time::Instant::now();
    let mut output = tokio::fs::File::from_std(
        target
            .reopen()
            .map_err(|_| "Could not write temporary attachment")?,
    );
    let mut downloaded = 0;
    loop {
        let next = tokio::time::timeout(Duration::from_secs(15), response.chunk())
            .await
            .map_err(|_| "Attachment download stalled".to_string())?
            .map_err(|_| "Attachment download interrupted")?;
        let Some(chunk) = next else {
            break;
        };
        downloaded += chunk.len() as u64;
        if downloaded > limit {
            return Err("Attachment is too large".into());
        }
        output
            .write_all(&chunk)
            .await
            .map_err(|_| "Could not write attachment. Check available disk space.")?;
        if downloaded == expected || last_progress.elapsed() >= Duration::from_millis(100) {
            progress
                .send(DownloadProgress { downloaded, total })
                .map_err(|_| "Download cancelled")?;
            last_progress = std::time::Instant::now();
        }
    }
    if downloaded != expected {
        return Err("Attachment download was incomplete".into());
    }
    output
        .flush()
        .await
        .map_err(|_| "Could not finish writing attachment")?;
    output
        .sync_all()
        .await
        .map_err(|_| "Could not save attachment to disk")?;
    Ok(())
}

#[tauri::command]
pub async fn attachment_action(
    app: AppHandle<Wry>,
    state: State<'_, AttachmentTransfers>,
    account_id: String,
    mailbox: Mailbox,
    email_id: String,
    attachment_id: String,
    request_id: String,
    action: AttachmentAction,
    on_progress: Channel<DownloadProgress>,
) -> Result<AttachmentResult, String> {
    validate_id(&email_id)?;
    validate_id(&attachment_id)?;
    validate_id(&request_id)?;
    let (cancel, mut cancellation) = tokio::sync::watch::channel(false);
    let key = (account_id.clone(), request_id);
    {
        let mut active = state
            .active
            .lock()
            .map_err(|_| "Attachment transfer lock failed")?;
        if active.contains_key(&key) {
            return Err("Attachment transfer already exists".into());
        }
        active.insert(key.clone(), cancel.clone());
    }
    let operation = async {
        let credential = oauth::get_account_credential(&app, &account_id).await?;
        let resend = Resend::new(&credential);
        let attachment = metadata(&resend, mailbox, &email_id, &attachment_id).await?;
        let name = safe_filename(attachment.filename.as_deref());
        let destination = if matches!(action, AttachmentAction::Save) {
            let (send, receive) = tokio::sync::oneshot::channel();
            app.dialog()
                .file()
                .set_file_name(&name)
                .save_file(move |path| {
                    let _ = send.send(path);
                });
            let Some(path) = receive.await.map_err(|_| "Could not open save dialog")? else {
                return Ok(AttachmentResult::Cancelled);
            };
            Some(
                path.into_path()
                    .map_err(|_| "Unsupported save destination")?,
            )
        } else {
            None
        };
        let temporary = if let Some(ref path) = destination {
            NamedTempFile::new_in(path.parent().ok_or("Invalid save destination")?)
        } else {
            NamedTempFile::new()
        }
        .map_err(|_| "Could not create temporary attachment")?;
        let limit = if matches!(action, AttachmentAction::Preview) {
            MAX_PREVIEW_BYTES
        } else {
            MAX_ATTACHMENT_BYTES
        };
        download(
            &resend,
            mailbox,
            &email_id,
            &attachment_id,
            attachment,
            &temporary,
            limit,
            &cancel,
            &on_progress,
        )
        .await?;
        crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
        if *cancel.borrow() {
            return Ok(AttachmentResult::Cancelled);
        }
        match action {
            AttachmentAction::Preview => {
                let bytes = tokio::fs::read(temporary.path())
                    .await
                    .map_err(|_| "Could not read image preview")?;
                let mime = image_mime(&bytes).ok_or(
                    "Preview supports PNG, JPEG, GIF and WebP images only. Save this file instead.",
                )?;
                Ok(AttachmentResult::Preview(format!(
                    "data:{mime};base64,{}",
                    STANDARD.encode(bytes)
                )))
            }
            AttachmentAction::Save => {
                let path = destination.ok_or("No save destination selected")?;
                if !state.begin_commit(&key, &cancel)? {
                    return Ok(AttachmentResult::Cancelled);
                }
                tauri::async_runtime::spawn_blocking(move || {
                    temporary.persist(path).map(|_| ()).map_err(|_| {
                        "Could not save attachment; the previous file was not replaced".to_string()
                    })
                })
                .await
                .map_err(|_| "Could not finish saving attachment")??;
                Ok(AttachmentResult::Saved)
            }
            AttachmentAction::Open => {
                let mut header = [0; 1024];
                let mut file = temporary
                    .reopen()
                    .map_err(|_| "Could not inspect attachment")?;
                let length = file
                    .read(&mut header)
                    .map_err(|_| "Could not inspect attachment")?;
                if !openable(&name, &header[..length]) {
                    return Err(
                        "This file cannot be opened automatically. Use Save as instead.".into(),
                    );
                }
                let directory = tempfile::Builder::new()
                    .prefix("onlysend-attachment-")
                    .tempdir()
                    .map_err(|_| "Could not create attachment directory")?;
                let path = directory.path().join(name);
                temporary
                    .persist(&path)
                    .map_err(|_| "Could not prepare attachment for opening")?;
                if !state.begin_commit(&key, &cancel)? {
                    return Ok(AttachmentResult::Cancelled);
                }
                state
                    .opened
                    .lock()
                    .map_err(|_| "Attachment file lock failed")?
                    .push(directory);
                crate::external::open_attachment_file(app.clone(), path).await?;
                Ok(AttachmentResult::Opened)
            }
        }
    };
    let result = tokio::select! {
        result = operation => result,
        _ = cancellation.changed() => Ok(AttachmentResult::Cancelled),
    };
    state
        .active
        .lock()
        .map_err(|_| "Attachment transfer lock failed")?
        .remove(&key);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn fixture_response(body: &'static [u8]) -> reqwest::Response {
        use std::io::{BufRead, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
            }
            write!(stream, "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n").unwrap();
            stream.write_all(body).unwrap();
        });
        let response = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap()
            .get(format!("http://{address}"))
            .send()
            .await
            .unwrap();
        server.join().unwrap();
        response
    }
    #[tokio::test]
    async fn bounded_streaming_preserves_existing_files_on_oversize_and_incomplete_downloads() {
        let directory = tempfile::tempdir().unwrap();
        let existing = directory.path().join("invoice.pdf");
        std::fs::write(&existing, b"original").unwrap();
        let progress = Channel::new(|_| Ok(()));
        for (expected, limit) in [(7, 3), (20, 25), (3, 5)] {
            let temporary = NamedTempFile::new_in(directory.path()).unwrap();
            let path = temporary.path().to_owned();
            let result = write_response(
                fixture_response(b"payload").await,
                &temporary,
                expected,
                limit,
                &progress,
            )
            .await;
            assert!(result.is_err());
            assert_eq!(std::fs::read(&existing).unwrap(), b"original");
            drop(temporary);
            assert!(!path.exists());
        }
        let temporary = NamedTempFile::new_in(directory.path()).unwrap();
        write_response(
            fixture_response(b"payload").await,
            &temporary,
            7,
            25,
            &progress,
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(temporary.path()).unwrap(), b"payload");
        temporary.persist(&existing).unwrap();
        assert_eq!(std::fs::read(existing).unwrap(), b"payload");
    }

    #[test]
    fn cancellation_is_account_scoped_and_cannot_race_an_atomic_commit() {
        let state = AttachmentTransfers::default();
        let key = ("account-a".to_string(), "request".to_string());
        let (cancel, _receiver) = tokio::sync::watch::channel(false);
        state
            .active
            .lock()
            .unwrap()
            .insert(key.clone(), cancel.clone());
        assert!(!state
            .active
            .lock()
            .unwrap()
            .contains_key(&("account-b".into(), "request".into())));
        assert!(state.begin_commit(&key, &cancel).unwrap());
        assert!(!state.active.lock().unwrap().contains_key(&key));
        let (cancel, _receiver) = tokio::sync::watch::channel(false);
        state
            .active
            .lock()
            .unwrap()
            .insert(key.clone(), cancel.clone());
        cancel.send(true).unwrap();
        assert!(!state.begin_commit(&key, &cancel).unwrap());
    }

    #[test]
    fn names_cannot_escape_a_directory_or_use_windows_devices() {
        for value in [
            "../../secret.txt",
            "C:\\windows\\file.pdf",
            "CON.txt",
            " . ",
            "ab\u{202e}exe.pdf",
            "line\nname.png",
        ] {
            let name = safe_filename(Some(value));
            assert!(!name.contains(['/', '\\', ':', '\n', '\u{202e}']));
            assert!(!name.starts_with('.'));
            assert!(!name.is_empty());
        }
        assert_eq!(safe_filename(Some("CON.txt")), "_CON.txt");
        assert!(safe_filename(Some(&"é".repeat(200))).len() <= 180);
    }
    #[test]
    fn only_resend_https_cdn_urls_without_redirects_or_credentials_are_accepted() {
        assert!(download_url("https://inbound-cdn.resend.com/a?signature=secret").is_ok());
        assert!(download_url("https://outbound-cdn.resend.com/a").is_ok());
        for url in [
            "http://inbound-cdn.resend.com/a",
            "https://inbound-cdn.resend.com.evil.test/a",
            "https://user:pass@outbound-cdn.resend.com/a",
            "https://127.0.0.1/a",
            "file:///tmp/a",
            "https://outbound-cdn.resend.com:444/a",
        ] {
            assert!(download_url(url).is_err());
        }
    }
    #[test]
    fn unsupported_server_errors_expose_only_the_hostname() {
        let error = download_url("https://user:private-password@storage.example.test/private-invoice.pdf?Signature=private-signature&token=private-token#private-fragment").unwrap_err();
        assert_eq!(
            error,
            "Unsupported attachment download server: storage.example.test"
        );
        assert_eq!(
            download_url("[REDACTED]").unwrap_err(),
            "Invalid attachment download URL"
        );
    }

    #[test]
    fn metadata_exposes_cid_but_never_signed_download_urls() {
        let value: Attachment = serde_json::from_value(serde_json::json!({
            "id":"2a0c9ce0-3112-4728-976e-47ddcd16a318", "filename":"logo.png", "size":20,
            "content_type":"image/png", "content_disposition":"inline", "content_id":"logo@example",
            "download_url":"https://inbound-cdn.resend.com/file?signature=private", "expires_at":"2026-10-28T12:00:00Z"
        })).unwrap();
        let json = serde_json::to_value(AttachmentInfo::from(&value)).unwrap();
        assert_eq!(json["contentId"], "logo@example");
        assert_eq!(json["contentDisposition"], "inline");
        assert!(!json.to_string().contains("signature"));
        assert!(json.get("downloadUrl").is_none());
    }

    #[test]
    fn previews_check_content_and_opening_rejects_active_files_and_spoofed_extensions() {
        assert_eq!(image_mime(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(image_mime(b"<svg onload='evil()'>"), None);
        assert!(openable("invoice.pdf", b"%PDF-1.7"));
        assert!(openable("report.docx", b"PK\x03\x04rest"));
        for name in [
            "run.exe",
            "run.desktop",
            "run.sh",
            "page.html",
            "image.svg",
            "link.url",
            "invoice.pdf.exe",
        ] {
            assert!(!openable(name, b"%PDF-1.7"));
        }
        assert!(!openable("invoice.pdf", b"MZ executable"));
        assert!(!openable("image.png", b"<svg>"));
    }
}
