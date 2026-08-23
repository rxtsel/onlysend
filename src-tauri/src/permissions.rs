use serde::Serialize;

const RESEND_API_BASE: &str = "https://api.resend.com";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub full_access: bool,
}

/// Cheap permission probe: a send-only credential is rejected by any
/// non-sending endpoint, so a single GET /domains?limit=1 tells us whether
/// the credential has full access.
#[tauri::command]
pub async fn probe_full_access(credential: String) -> Result<ProbeResult, String> {
    let http = reqwest::Client::builder()
        .user_agent(concat!("OnlySend/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("[ERROR] Failed to build http client: {}", e))?;

    let response = http
        .get(format!("{RESEND_API_BASE}/domains"))
        .query(&[("limit", "1")])
        .bearer_auth(&credential)
        .send()
        .await
        .map_err(|e| format!("[ERROR] Could not reach Resend: {}", e))?;

    let status = response.status();
    if status.is_success() {
        return Ok(ProbeResult { full_access: true });
    }

    // restricted_api_key => the credential can only send emails.
    let body: serde_json::Value = response.json().await.unwrap_or_default();
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    if name == "restricted_api_key" {
        return Ok(ProbeResult { full_access: false });
    }

    Err(format!(
        "[ERROR] Permission check failed (HTTP {}): {}",
        status,
        body.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error")
    ))
}

/// Appends actionable guidance when an API/credential restriction is detected.
pub fn friendly_error(message: String) -> String {
    let lower = message.to_lowercase();
    if lower.contains("restricted_api_key") || lower.contains("restricted to only send") {
        format!(
            "{message} OnlySend needs a credential with Full access to manage your emails and domains."
        )
    } else {
        message
    }
}

/// Convenience wrapper for command error mapping.
pub fn map_resend_error<E: std::fmt::Display>(context: &str, err: E) -> String {
    friendly_error(format!("[ERROR] {}: {}", context, err))
}
