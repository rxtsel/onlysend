use crate::oauth;
use crate::permissions;
use base64::{engine::general_purpose, Engine as _};
use resend_rs::types::{CreateAttachment, CreateEmailBaseOptions};
use resend_rs::Resend;
use serde::Deserialize;
use tauri::{AppHandle, Wry};

#[derive(Debug, Deserialize)]
pub struct AttachmentPayload {
    pub filename: String,
    pub content: String, // base64
}

#[derive(Debug, Deserialize)]
pub struct EmailComposerData {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,

    #[serde(default)]
    pub cc: Vec<String>,

    #[serde(default)]
    pub bcc: Vec<String>,

    #[serde(rename = "replyTo")]
    pub reply_to: Option<String>,

    pub content: String,

    #[serde(rename = "messageId")]
    pub message_id: Option<String>,

    #[serde(default)]
    pub attachments: Vec<AttachmentPayload>,
}

#[tauri::command]
pub async fn send_email(app: AppHandle<Wry>, account_id: String, data: EmailComposerData) -> Result<(), String> {
    // Bearer credential: API key or OAuth access token
    let credential = oauth::get_account_credential(&app, &account_id).await?;

    let resend = Resend::new(&credential);

    // Email base
    let mut email =
        CreateEmailBaseOptions::new(data.from.as_str(), data.to.clone(), data.subject.as_str())
            .with_html(data.content.as_str());

    // CC
    for cc in &data.cc {
        email = email.with_cc(cc.as_str());
    }

    // BCC
    for bcc in &data.bcc {
        email = email.with_bcc(bcc.as_str());
    }

    // Reply-To
    if let Some(ref reply) = data.reply_to {
        email = email.with_reply(reply.as_str());
    }

    // In-Reply-To
    if let Some(ref msg_id) = data.message_id {
        email = email.with_header("In-Reply-To", msg_id.as_str());
    }

    // Attachments
    for att in &data.attachments {
        // content come as base64
        let bytes = general_purpose::STANDARD
            .decode(&att.content)
            .map_err(|e| format!("[ERROR] Failed to decode attachment {}: {e}", att.filename))?;

        let attachment = CreateAttachment::from_content(bytes).with_filename(att.filename.as_str());

        email = email.with_attachment(attachment);
    }

    // Send
    resend
        .emails
        .send(email)
        .await
        .map_err(|e| permissions::friendly_error(format!("[ERROR] Error sending email: {e}")))?;

    Ok(())
}
