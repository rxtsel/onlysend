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

fn attachment_bytes(attachment: &AttachmentPayload, total: &mut u64) -> Result<Vec<u8>, String> {
    let limit = crate::attachments::MAX_ATTACHMENT_BYTES;
    if attachment.filename.is_empty()
        || attachment
            .filename
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '/' | '\\'))
    {
        return Err("Invalid attachment filename".into());
    }
    if attachment.content.len() as u64 > (limit + 2) / 3 * 4 {
        return Err("Attachments must total no more than 25 MB".into());
    }
    let bytes = general_purpose::STANDARD
        .decode(&attachment.content)
        .map_err(|_| "Invalid attachment data")?;
    *total += bytes.len() as u64;
    if *total > limit {
        return Err("Attachments must total no more than 25 MB".into());
    }
    Ok(bytes)
}

#[tauri::command]
pub async fn send_email(
    app: AppHandle<Wry>,
    account_id: String,
    data: EmailComposerData,
) -> Result<(), String> {
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
    let mut total = 0;
    for att in &data.attachments {
        let bytes = attachment_bytes(att, &mut total)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attachment_data_is_bounded_validated_and_counted_across_files() {
        let mut total = 0;
        let file = AttachmentPayload {
            filename: "file.bin".into(),
            content: general_purpose::STANDARD.encode([0, 255]),
        };
        assert_eq!(attachment_bytes(&file, &mut total).unwrap(), vec![0, 255]);
        assert_eq!(total, 2);
        total = crate::attachments::MAX_ATTACHMENT_BYTES - 1;
        assert!(attachment_bytes(&file, &mut total).is_err());
        let invalid = AttachmentPayload {
            filename: "file.bin".into(),
            content: "invalid!".into(),
        };
        assert!(attachment_bytes(&invalid, &mut 0).is_err());
        let invalid = AttachmentPayload {
            filename: "../file.bin".into(),
            content: "".into(),
        };
        assert!(attachment_bytes(&invalid, &mut 0).is_err());
    }
}
