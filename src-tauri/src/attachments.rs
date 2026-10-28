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
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{ipc::Channel, AppHandle, State, Wry};
use tauri_plugin_dialog::DialogExt;
use tempfile::{NamedTempFile, TempDir};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

type AttachmentCacheKey = (String, Mailbox, String, String);
type AttachmentSlot = Arc<tokio::sync::Mutex<Option<CachedAttachment>>>;

struct CachedAttachment {
    directory: TempDir,
    name: String,
    size: u64,
}
impl CachedAttachment {
    fn path(&self) -> PathBuf {
        self.directory.path().join(&self.name)
    }
    fn is_valid(&self) -> bool {
        std::fs::symlink_metadata(self.path()).is_ok_and(|metadata| {
            metadata.is_file() && metadata.len() == self.size && self.size <= MAX_ATTACHMENT_BYTES
        })
    }
}

struct TemporaryRegistration {
    path: PathBuf,
    files: Arc<Mutex<HashSet<PathBuf>>>,
}
impl Drop for TemporaryRegistration {
    fn drop(&mut self) {
        // Keep failed deletions registered so exit cleanup can try again.
        if self.path.try_exists().unwrap_or(true) {
            return;
        }
        if let Ok(mut files) = self.files.lock() {
            files.remove(&self.path);
        }
    }
}

// Field order matters: delete/close the temporary before releasing its registration.
struct PendingSave {
    file: NamedTempFile,
    _registration: TemporaryRegistration,
}
impl PendingSave {
    fn persist(self, destination: impl AsRef<Path>) -> Result<(), String> {
        self.file
            .persist(destination)
            .map(|_| ())
            .map_err(|_| "Could not save attachment; the previous file was not replaced".into())
    }

    fn persist_new(self, destination: &Path) -> Result<(), String> {
        match self.file.persist_noclobber(destination) {
            Ok(_) => Ok(()),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(_) => Err("Could not prepare attachment for opening".into()),
        }
    }
}

#[derive(Default)]
pub struct AttachmentTransfers {
    active: Mutex<HashMap<(String, String), tokio::sync::watch::Sender<bool>>>,
    // shortcut: session files live until app exit; add limits if long sessions accumulate files.
    files: Mutex<HashMap<AttachmentCacheKey, AttachmentSlot>>,
    directory: Mutex<Option<TempDir>>,
    pending_files: Arc<Mutex<HashSet<PathBuf>>>,
    closing: AtomicBool,
}
impl AttachmentTransfers {
    fn file_slot(&self, key: AttachmentCacheKey) -> Result<AttachmentSlot, String> {
        Ok(self
            .files
            .lock()
            .map_err(|_| "Attachment cache lock failed")?
            .entry(key)
            .or_default()
            .clone())
    }

    fn temporary_directory(&self) -> Result<TempDir, String> {
        let mut directory = self
            .directory
            .lock()
            .map_err(|_| "Attachment cache lock failed")?;
        if self.closing.load(Ordering::Relaxed) {
            return Err("OnlySend is closing".into());
        }
        if !directory.as_ref().is_some_and(|root| {
            std::fs::symlink_metadata(root.path()).is_ok_and(|metadata| metadata.is_dir())
        }) {
            *directory = Some(
                tempfile::Builder::new()
                    .prefix("onlysend-attachments-")
                    .tempdir()
                    .map_err(|_| "Could not create attachment directory")?,
            );
        }
        tempfile::Builder::new()
            .prefix("file-")
            .tempdir_in(
                directory
                    .as_ref()
                    .ok_or("Missing attachment directory")?
                    .path(),
            )
            .map_err(|_| "Could not create attachment directory".into())
    }

    fn save_temporary(&self, destination: &Path) -> Result<PendingSave, String> {
        let mut files = self
            .pending_files
            .lock()
            .map_err(|_| "Attachment save lock failed")?;
        if self.closing.load(Ordering::Relaxed) {
            return Err("OnlySend is closing".into());
        }
        let file = NamedTempFile::new_in(destination.parent().ok_or("Invalid save destination")?)
            .map_err(|_| "Could not create temporary attachment")?;
        let path = file.path().to_owned();
        files.insert(path.clone());
        Ok(PendingSave {
            file,
            _registration: TemporaryRegistration {
                path,
                files: self.pending_files.clone(),
            },
        })
    }

    pub(crate) fn cleanup_on_exit(&self) {
        self.closing.store(true, Ordering::Relaxed);
        if let Ok(mut active) = self.active.lock() {
            for cancel in active.values() {
                let _ = cancel.send(true);
            }
            active.clear();
        }
        if let Ok(mut files) = self.files.lock() {
            files.clear();
        }
        // Destination-side temporaries are outside the session root; do not rely on Drop.
        if let Ok(mut files) = self.pending_files.lock() {
            for path in files.drain() {
                if let Err(error) = std::fs::remove_file(path) {
                    if error.kind() != std::io::ErrorKind::NotFound {
                        eprintln!("[WARN] Could not remove a temporary attachment save; a file may still be in use");
                    }
                }
            }
        }
        // Tauri may exit without running destructors. Remove the session root explicitly.
        if let Ok(directory) = self.directory.lock() {
            if let Some(directory) = directory.as_ref() {
                if let Err(error) = std::fs::remove_dir_all(directory.path()) {
                    if error.kind() != std::io::ErrorKind::NotFound {
                        eprintln!("[WARN] Could not remove all temporary attachments; a file may still be in use");
                    }
                }
            }
        }
    }

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

async fn cached_file<F, Download>(
    slot: &tokio::sync::Mutex<Option<CachedAttachment>>,
    fetch: F,
) -> Result<PathBuf, String>
where
    F: FnOnce() -> Download,
    Download: std::future::Future<Output = Result<CachedAttachment, String>>,
{
    // Concurrent actions for the same attachment share one completed download.
    let mut cached = slot.lock().await;
    if let Some(file) = cached.as_ref().filter(|file| file.is_valid()) {
        return Ok(file.path());
    }
    *cached = None;
    let file = fetch().await?;
    let path = file.path();
    *cached = Some(file);
    Ok(path)
}

async fn preview_source(path: &Path) -> Result<String, String> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| "Could not read cached attachment. Try again.")?;
    let mut bytes = Vec::new();
    file.take(MAX_PREVIEW_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| "Could not read image preview")?;
    if bytes.len() as u64 > MAX_PREVIEW_BYTES {
        return Err("Image previews support files up to 5 MB. Save this file instead.".into());
    }
    let mime = image_mime(&bytes)
        .ok_or("Preview supports PNG, JPEG, GIF and WebP images only. Save this file instead.")?;
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

async fn copy_for_save(
    source: &Path,
    destination: &Path,
    state: &AttachmentTransfers,
) -> Result<PendingSave, String> {
    let temporary = state.save_temporary(destination)?;
    let input = tokio::fs::File::open(source)
        .await
        .map_err(|_| "Could not read cached attachment. Try again.")?;
    let expected = input
        .metadata()
        .await
        .map_err(|_| "Could not inspect cached attachment")?
        .len();
    if expected > MAX_ATTACHMENT_BYTES {
        return Err("Attachment is too large".into());
    }
    let mut input = input.take(MAX_ATTACHMENT_BYTES + 1);
    let mut output = tokio::fs::File::from_std(
        temporary
            .file
            .reopen()
            .map_err(|_| "Could not write temporary attachment")?,
    );
    let copied = tokio::io::copy(&mut input, &mut output)
        .await
        .map_err(|_| "Could not copy attachment. Check available disk space.")?;
    if copied != expected || copied > MAX_ATTACHMENT_BYTES {
        return Err("Cached attachment changed while copying. Try again.".into());
    }
    output
        .flush()
        .await
        .map_err(|_| "Could not finish copying attachment")?;
    output
        .sync_all()
        .await
        .map_err(|_| "Could not save attachment to disk")?;
    Ok(temporary)
}

async fn working_copy(source: &Path, state: &AttachmentTransfers) -> Result<PathBuf, String> {
    let directory = source
        .parent()
        .ok_or("Invalid cached attachment")?
        .join("opened");
    // Never recreate the session root if exit cleanup has already removed it.
    match tokio::fs::create_dir(&directory).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err("Could not create attachment working directory".into()),
    }
    if !std::fs::symlink_metadata(&directory).is_ok_and(|metadata| metadata.is_dir()) {
        return Err("Invalid attachment working directory".into());
    }
    let destination = directory.join(source.file_name().ok_or("Invalid cached attachment name")?);
    match tokio::fs::symlink_metadata(&destination).await {
        Ok(metadata) if metadata.is_file() => return Ok(destination),
        Ok(_) => return Err("Invalid attachment working copy".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Could not inspect attachment working copy".into()),
    }
    // A second click must not overwrite edits if another action created this copy first.
    copy_for_save(source, &destination, state)
        .await?
        .persist_new(&destination)?;
    if !std::fs::symlink_metadata(&destination).is_ok_and(|metadata| metadata.is_file()) {
        return Err("Invalid attachment working copy".into());
    }
    Ok(destination)
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
// shortcut: only known Resend CDNs are allowed; update if their storage hosts change.
fn download_url(value: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "Invalid attachment download URL")?;
    if url.scheme() != "https"
        || !matches!(
            url.host_str(),
            Some("inbound-cdn.resend.com" | "outbound-cdn.resend.com" | "cdn.resend.app")
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
        // A cache hit needs the local account, not an online OAuth refresh.
        crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
        let slot = state.file_slot((
            account_id.clone(),
            mailbox,
            email_id.clone(),
            attachment_id.clone(),
        ))?;
        let source = cached_file(&slot, || async {
            let credential = oauth::get_account_credential(&app, &account_id).await?;
            let resend = Resend::new(&credential);
            let attachment = metadata(&resend, mailbox, &email_id, &attachment_id).await?;
            let name = safe_filename(attachment.filename.as_deref());
            let directory = state.temporary_directory()?;
            let temporary = NamedTempFile::new_in(directory.path())
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
                return Err("Download cancelled".into());
            }
            let size = temporary
                .as_file()
                .metadata()
                .map_err(|_| "Could not inspect cached attachment")?
                .len();
            temporary
                .persist(directory.path().join(&name))
                .map_err(|_| "Could not cache attachment")?;
            Ok(CachedAttachment {
                directory,
                name,
                size,
            })
        })
        .await?;
        crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
        if *cancel.borrow() {
            return Ok(AttachmentResult::Cancelled);
        }
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid cached attachment name")?;
        match action {
            AttachmentAction::Preview => {
                Ok(AttachmentResult::Preview(preview_source(&source).await?))
            }
            AttachmentAction::Save => {
                let (send, receive) = tokio::sync::oneshot::channel();
                app.dialog()
                    .file()
                    .set_file_name(name)
                    .save_file(move |path| {
                        let _ = send.send(path);
                    });
                let Some(path) = receive.await.map_err(|_| "Could not open save dialog")? else {
                    return Ok(AttachmentResult::Cancelled);
                };
                let path = path
                    .into_path()
                    .map_err(|_| "Unsupported save destination")?;
                let temporary = copy_for_save(&source, &path, &state).await?;
                crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
                if !state.begin_commit(&key, &cancel)? {
                    return Ok(AttachmentResult::Cancelled);
                }
                tauri::async_runtime::spawn_blocking(move || temporary.persist(path))
                    .await
                    .map_err(|_| "Could not finish saving attachment")??;
                Ok(AttachmentResult::Saved)
            }
            AttachmentAction::Open => {
                let source = working_copy(&source, &state).await?;
                let mut header = [0; 1024];
                let mut file = std::fs::File::open(&source)
                    .map_err(|_| "Could not read cached attachment. Try again.")?;
                let length = file
                    .read(&mut header)
                    .map_err(|_| "Could not inspect attachment")?;
                if !openable(name, &header[..length]) {
                    return Err(
                        "This file cannot be opened automatically. Use Save as instead.".into(),
                    );
                }
                crate::infrastructure::credentials_store::account_credential(&app, &account_id)?;
                if !state.begin_commit(&key, &cancel)? {
                    return Ok(AttachmentResult::Cancelled);
                }
                crate::external::open_attachment_file(app.clone(), source).await?;
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
    if *cancel.borrow() {
        Ok(AttachmentResult::Cancelled)
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached_fixture(state: &AttachmentTransfers, bytes: &[u8]) -> CachedAttachment {
        let directory = state.temporary_directory().unwrap();
        let name = "invoice.pdf".to_string();
        std::fs::write(directory.path().join(&name), bytes).unwrap();
        CachedAttachment {
            directory,
            name,
            size: bytes.len() as u64,
        }
    }

    #[tokio::test]
    async fn external_edits_do_not_change_or_invalidate_the_original_attachment() {
        let state = AttachmentTransfers::default();
        let original = cached_fixture(&state, b"%PDF-original");
        let source = original.path();
        let opened = working_copy(&source, &state).await.unwrap();
        assert_ne!(opened, source);
        for edit in [
            b"%PDF-modified".as_slice(),
            b"%PDF-modified with annotations".as_slice(),
        ] {
            std::fs::write(&opened, edit).unwrap();
            assert!(original.is_valid());
            assert_eq!(std::fs::read(&source).unwrap(), b"%PDF-original");
            assert_eq!(working_copy(&source, &state).await.unwrap(), opened);
            assert_eq!(std::fs::read(&opened).unwrap(), edit);
        }
        copy_for_save(&source, &opened, &state)
            .await
            .unwrap()
            .persist_new(&opened)
            .unwrap();
        assert_eq!(
            std::fs::read(&opened).unwrap(),
            b"%PDF-modified with annotations"
        );
        let export = tempfile::tempdir().unwrap();
        let destination = export.path().join("saved.pdf");
        copy_for_save(&source, &destination, &state)
            .await
            .unwrap()
            .persist(&destination)
            .unwrap();
        assert_eq!(std::fs::read(destination).unwrap(), b"%PDF-original");
        assert_eq!(
            std::fs::read(opened).unwrap(),
            b"%PDF-modified with annotations"
        );
        assert!(state.pending_files.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn simultaneous_external_opens_publish_one_working_copy_without_leftover_temporaries() {
        let state = AttachmentTransfers::default();
        let original = cached_fixture(&state, b"%PDF-original");
        let source = original.path();
        let (first, second) =
            tokio::join!(working_copy(&source, &state), working_copy(&source, &state));
        let opened = first.unwrap();
        assert_eq!(second.unwrap(), opened);
        assert_eq!(std::fs::read(&opened).unwrap(), b"%PDF-original");
        assert_eq!(
            std::fs::read_dir(opened.parent().unwrap()).unwrap().count(),
            1
        );
        assert!(state.pending_files.lock().unwrap().is_empty());
    }

    #[test]
    fn exit_cleans_destination_temporaries_even_while_their_owner_is_alive() {
        let state = AttachmentTransfers::default();
        let registry = Arc::downgrade(&state.pending_files);
        let exports = tempfile::tempdir().unwrap();
        let destination = exports.path().join("invoice.pdf");
        std::fs::write(&destination, b"original saved document").unwrap();
        let temporary = state.save_temporary(&destination).unwrap();
        let path = temporary.file.path().to_owned();
        std::fs::write(&path, b"partial attachment").unwrap();
        assert!(state.pending_files.lock().unwrap().contains(&path));
        state.cleanup_on_exit();
        #[cfg(unix)]
        assert!(!path.exists());
        assert_eq!(
            std::fs::read(&destination).unwrap(),
            b"original saved document"
        );
        assert!(state.pending_files.lock().unwrap().is_empty());
        assert!(state.save_temporary(&destination).is_err());
        drop(temporary);
        assert!(!path.exists());
        assert_eq!(std::fs::read_dir(exports.path()).unwrap().count(), 1);
        drop(state);
        assert!(registry.upgrade().is_none());
    }

    #[tokio::test]
    async fn session_cache_reuses_bytes_offline_and_save_copies_without_moving_the_source() {
        let state = AttachmentTransfers::default();
        let key = (
            "account".into(),
            Mailbox::Inbox,
            "email".into(),
            "file".into(),
        );
        let slot = state.file_slot(key).unwrap();
        let source = cached_file(&slot, || async {
            Ok(cached_fixture(&state, b"%PDF-original"))
        })
        .await
        .unwrap();
        let reused = cached_file(&slot, || async { Err("network is offline".to_string()) })
            .await
            .unwrap();
        assert_eq!(source, reused);
        let saved = tempfile::tempdir().unwrap();
        let destination = saved.path().join("invoice.pdf");
        std::fs::write(&destination, b"previous file").unwrap();
        let copy = copy_for_save(&source, &destination, &state).await.unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"previous file");
        assert_eq!(std::fs::read(&source).unwrap(), b"%PDF-original");
        copy.persist(&destination).unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"%PDF-original");
        std::fs::write(&destination, b"user edits").unwrap();
        assert_eq!(std::fs::read(&source).unwrap(), b"%PDF-original");
        let unfinished = state.temporary_directory().unwrap();
        std::fs::write(unfinished.path().join("partial"), b"partial").unwrap();
        state.cleanup_on_exit();
        assert!(!source.exists());
        assert!(!unfinished.path().exists());
        assert_eq!(std::fs::read(destination).unwrap(), b"user edits");
        assert!(state.files.lock().unwrap().is_empty());
        assert!(state.temporary_directory().is_err());
    }

    #[test]
    fn session_cache_is_scoped_by_account_mailbox_email_and_attachment_not_filename() {
        let state = AttachmentTransfers::default();
        let key = ("a".into(), Mailbox::Inbox, "email".into(), "file".into());
        let first = state.file_slot(key.clone()).unwrap();
        assert!(Arc::ptr_eq(&first, &state.file_slot(key).unwrap()));
        for key in [
            ("b".into(), Mailbox::Inbox, "email".into(), "file".into()),
            ("a".into(), Mailbox::Sent, "email".into(), "file".into()),
            ("a".into(), Mailbox::Inbox, "other".into(), "file".into()),
            ("a".into(), Mailbox::Inbox, "email".into(), "other".into()),
        ] {
            assert!(!Arc::ptr_eq(&first, &state.file_slot(key).unwrap()));
        }
    }

    #[tokio::test]
    async fn simultaneous_actions_download_the_same_file_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let state = AttachmentTransfers::default();
        let slot = tokio::sync::Mutex::new(None);
        let calls = AtomicUsize::new(0);
        let (first, second) = tokio::join!(
            cached_file(&slot, || async {
                calls.fetch_add(1, Ordering::Relaxed);
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(cached_fixture(&state, b"file"))
            }),
            cached_file(&slot, || async {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(cached_fixture(&state, b"file"))
            }),
        );
        assert_eq!(first.unwrap(), second.unwrap());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn missing_or_incomplete_files_are_evicted_and_can_be_downloaded_again() {
        let state = AttachmentTransfers::default();
        let slot = tokio::sync::Mutex::new(None);
        for invalid in ["missing", "truncated", "root"] {
            let source = cached_file(&slot, || async { Ok(cached_fixture(&state, b"complete")) })
                .await
                .unwrap();
            match invalid {
                "missing" => std::fs::remove_file(&source).unwrap(),
                "truncated" => std::fs::write(&source, b"short").unwrap(),
                _ => std::fs::remove_dir_all(
                    state.directory.lock().unwrap().as_ref().unwrap().path(),
                )
                .unwrap(),
            }
            assert!(cached_file(&slot, || async { Err("offline".to_string()) })
                .await
                .is_err());
            assert!(slot.lock().await.is_none());
            let recovered =
                cached_file(&slot, || async { Ok(cached_fixture(&state, b"complete")) })
                    .await
                    .unwrap();
            assert_eq!(std::fs::read(recovered).unwrap(), b"complete");
        }
    }

    #[tokio::test]
    async fn a_cancelled_download_is_not_published_and_does_not_block_the_next_click() {
        let state = AttachmentTransfers::default();
        let slot = tokio::sync::Mutex::new(None);
        let mut partial = None;
        let result = tokio::time::timeout(
            Duration::from_millis(10),
            cached_file(&slot, || async {
                let file = cached_fixture(&state, b"partial");
                partial = Some(file.path());
                std::future::pending::<()>().await;
                Ok(file)
            }),
        )
        .await;
        assert!(result.is_err());
        assert!(!partial.unwrap().exists());
        assert!(slot.lock().await.is_none());
        let source = cached_file(&slot, || async { Ok(cached_fixture(&state, b"complete")) })
            .await
            .unwrap();
        assert_eq!(std::fs::read(source).unwrap(), b"complete");
    }

    #[tokio::test]
    async fn cached_previews_keep_the_size_limit_and_failed_saves_leave_destinations_untouched() {
        let state = AttachmentTransfers::default();
        let cached = cached_fixture(&state, b"%PDF-original");
        assert!(preview_source(&cached.path()).await.is_err());
        let image = cached_fixture(&state, b"\x89PNG\r\n\x1a\n");
        assert!(preview_source(&image.path())
            .await
            .unwrap()
            .starts_with("data:image/png;base64,"));
        let oversized = cached_fixture(&state, b"");
        std::fs::File::create(oversized.path())
            .unwrap()
            .set_len(MAX_PREVIEW_BYTES + 1)
            .unwrap();
        assert!(preview_source(&oversized.path())
            .await
            .unwrap_err()
            .contains("5 MB"));
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("saved.pdf");
        std::fs::write(&destination, b"original destination").unwrap();
        assert!(
            copy_for_save(&directory.path().join("missing"), &destination, &state)
                .await
                .is_err()
        );
        assert_eq!(
            std::fs::read(&destination).unwrap(),
            b"original destination"
        );
        std::fs::File::create(oversized.path())
            .unwrap()
            .set_len(MAX_ATTACHMENT_BYTES + 1)
            .unwrap();
        assert!(copy_for_save(&oversized.path(), &destination, &state)
            .await
            .is_err());
        assert_eq!(
            std::fs::read(&destination).unwrap(),
            b"original destination"
        );
        assert_eq!(std::fs::read(cached.path()).unwrap(), b"%PDF-original");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        assert!(state.pending_files.lock().unwrap().is_empty());
    }

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
        let signed = "https://cdn.resend.app/attachments/invoice.pdf?Signature=example%2Bsignature&Expires=123";
        assert_eq!(download_url(signed).unwrap().as_str(), signed);
        for url in [
            "http://inbound-cdn.resend.com/a",
            "https://inbound-cdn.resend.com.evil.test/a",
            "https://user:pass@outbound-cdn.resend.com/a",
            "https://127.0.0.1/a",
            "file:///tmp/a",
            "https://outbound-cdn.resend.com:444/a",
            "http://cdn.resend.app/a",
            "https://cdn.resend.app.evil.test/a",
            "https://evil.cdn.resend.app/a",
            "https://user:pass@cdn.resend.app/a",
            "https://cdn.resend.app:444/a",
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
