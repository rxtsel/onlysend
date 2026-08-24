use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State, Wry};
use tauri_plugin_opener::OpenerExt;

use crate::infrastructure::credentials_store as store;
use crate::infrastructure::settings_store as sstore;

const RESEND_API_BASE: &str = "https://api.resend.com";
const REDIRECT_URI: &str = "onlysend://oauth/callback";
const CLIENT_NAME: &str = "OnlySend";
const SCOPE: &str = "full_access";

/// Refresh 60s before actual expiry to avoid clock-skew failures.
const EXPIRY_MARGIN_SECS: i64 = 60;

#[derive(Debug, Clone, Serialize)]
struct PendingFlow {
    state: String,
    code_verifier: String,
}

#[derive(Default)]
pub struct OAuthState {
    pending: Mutex<Option<PendingFlow>>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct RegisterRequest<'a> {
    client_name: &'a str,
    redirect_uris: [&'a str; 1],
    grant_types: [&'a str; 2],
    response_types: [&'a str; 1],
    token_endpoint_auth_method: &'a str,
    scope: &'a str,
}

#[derive(Deserialize)]
struct RegisterResponse {
    client_id: String,
}

#[derive(Deserialize)]
struct TokenSuccess {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: i64,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Deserialize)]
struct TokenError {
    error: String,
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn generate_random_string(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

fn code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("OnlySend/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("failed to build http client")
}

/// Returns the registered client id, registering a new public client if needed.
async fn ensure_client_id(app: &AppHandle<Wry>) -> Result<String, String> {
    if let Some(id) = store::load_client_id(app)? {
        return Ok(id);
    }

    let body = RegisterRequest {
        client_name: CLIENT_NAME,
        redirect_uris: [REDIRECT_URI],
        grant_types: ["authorization_code", "refresh_token"],
        response_types: ["code"],
        token_endpoint_auth_method: "none",
        scope: SCOPE,
    };

    let response = http_client()
        .post(format!("{RESEND_API_BASE}/oauth/register"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("[ERROR] Failed to register OAuth client: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "[ERROR] Resend rejected the OAuth client registration (HTTP {})",
            response.status()
        ));
    }

    let parsed: RegisterResponse = response
        .json()
        .await
        .map_err(|e| format!("[ERROR] Invalid registration response: {}", e))?;

    store::save_client_id(app, &parsed.client_id)?;

    Ok(parsed.client_id)
}

/// Starts the authorization-code + PKCE flow: registers the client if needed,
/// stores the pending flow, opens the Resend consent screen in the browser and
/// returns the authorization URL as a copy fallback.
#[tauri::command]
pub async fn connect_resend(
    app: AppHandle<Wry>,
    state: State<'_, OAuthState>,
) -> Result<String, String> {
    let client_id = ensure_client_id(&app).await?;

    let verifier = generate_random_string(64);
    let challenge = code_challenge(&verifier);

    // Opaque state for CSRF protection.
    let csrf_state = generate_random_string(32);

    *state
        .pending
        .lock()
        .map_err(|_| "[ERROR] OAuth state corrupted".to_string())? = Some(PendingFlow {
        state: csrf_state.clone(),
        code_verifier: verifier,
    });

    let authorize_url = format!(
        "{RESEND_API_BASE}/oauth/authorize?client_id={client_id}&response_type=code&redirect_uri={redirect_uri}&scope={scope}&state={state_value}&code_challenge={challenge}&code_challenge_method=S256",
        redirect_uri = urlencoding_form(REDIRECT_URI),
        scope = urlencoding_form(SCOPE),
        state_value = urlencoding_form(&csrf_state),
        challenge = urlencoding_form(&challenge),
    );

    app.opener()
        .open_url(authorize_url.clone(), None::<&str>)
        .map_err(|e| format!("[ERROR] Failed to open browser: {}", e))?;

    Ok(authorize_url)
}

/// Minimal percent-encoding for query parameter values.
fn urlencoding_form(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// Handles an incoming `onlysend://oauth/callback?...` deep link URL.
pub fn handle_deep_link(app: AppHandle<Wry>, url: String) {
    tauri::async_runtime::spawn(async move {
        let result = complete_flow(&app, &url).await;
        match result {
            Ok(()) => {
                println!("[INFO] Resend OAuth flow completed successfully");

                // Warn when the grant only allows sending: OnlySend needs
                // full_access for listing emails, domains, etc.
                let send_only = store::load_oauth(&app)
                    .ok()
                    .flatten()
                    .map(|r| !r.scope.contains("full_access"))
                    .unwrap_or(false);

                let _ = app.emit(
                    "oauth://done",
                    serde_json::json!({
                        "success": true,
                        "warning": if send_only { Some("send_only") } else { None },
                    }),
                );
            }
            Err(err) => {
                println!("[WARN] Resend OAuth flow failed: {}", err);
                let _ = app.emit(
                    "oauth://done",
                    serde_json::json!({ "success": false, "error": err }),
                );
            }
        }
    });
}

async fn complete_flow(app: &AppHandle<Wry>, url: &str) -> Result<(), String> {
    let state: State<OAuthState> = app.state();

    let pending = {
        let mut guard = state
            .pending
            .lock()
            .map_err(|_| "[ERROR] OAuth state corrupted".to_string())?;
        guard.take()
    }
    .ok_or_else(|| "[ERROR] No OAuth flow is pending".to_string())?;

    let params = parse_query(url);

    if let Some(error) = params.get("error") {
        let description = params.get("error_description").cloned().unwrap_or_default();
        return Err(format!("Authorization failed: {} {}", error, description).trim_end().to_string());
    }

    let returned_state = params
        .get("state")
        .ok_or_else(|| "[ERROR] Callback missing state".to_string())?;
    if returned_state != &pending.state {
        return Err("[ERROR] Invalid state in callback".to_string());
    }

    let code = params
        .get("code")
        .ok_or_else(|| "[ERROR] Callback missing code".to_string())?;

    let client_id = get_client_id(app)?;
    exchange_code(app, code, &pending.code_verifier, &client_id).await
}

async fn exchange_code(
    app: &AppHandle<Wry>,
    code: &str,
    verifier: &str,
    client_id: &str,
) -> Result<(), String> {
    let form = [
        ("grant_type", "authorization_code"),
        ("client_id", client_id),
        ("code", code),
        ("redirect_uri", REDIRECT_URI),
        ("code_verifier", verifier),
    ];

    persist_tokens(app, client_id, form).await
}

async fn persist_tokens(
    app: &AppHandle<Wry>,
    client_id: &str,
    form: [(&str, &str); 5],
) -> Result<(), String> {
    let response = http_client()
        .post(format!("{RESEND_API_BASE}/oauth/token"))
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("[ERROR] Token request failed: {}", e))?;

    let status = response.status();
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("[ERROR] Invalid token response: {}", e))?;

    if !status.is_success() {
        let err: Result<TokenError, _> = serde_json::from_value(body.clone());

        // Reuse of a rotated refresh token revokes the whole grant.
        if matches!(&err, Ok(e) if e.error == "invalid_grant") {
            store::clear_oauth(app)?;
            return Err("Resend authorization expired. Please connect again.".to_string());
        }

        return Err(match err {
            Ok(e) => format!("[ERROR] Token request failed: {}", e.error),
            Err(_) => format!("[ERROR] Token request failed (HTTP {})", status),
        });
    }

    let tokens: TokenSuccess = serde_json::from_value(body)
        .map_err(|e| format!("[ERROR] Invalid token payload: {}", e))?;

    let refresh_token = tokens
        .refresh_token
        .or_else(|| {
            store::load_oauth(app)
                .ok()
                .flatten()
                .map(|r| r.refresh_token)
        })
        .ok_or_else(|| "[ERROR] Missing refresh token".to_string())?;

    let record = store::OAuthRecord {
        client_id: client_id.to_string(),
        refresh_token,
        access_token: tokens.access_token,
        expires_at: unix_now() + tokens.expires_in - EXPIRY_MARGIN_SECS,
        scope: tokens.scope.unwrap_or_default(),
    };

    // Single-credential invariant: connecting via OAuth replaces any stored API key.
    store::delete_api_key(app.clone())?;

    store::save_oauth(app, &record)
}

fn get_client_id(app: &AppHandle<Wry>) -> Result<String, String> {
    // Prefer the client id from an existing OAuth grant, falling back to the
    // one persisted by DCR (no OAuth record exists yet mid-flow).
    if let Some(record) = store::load_oauth(app)? {
        return Ok(record.client_id);
    }

    store::load_client_id(app)?
        .ok_or_else(|| "[ERROR] Not connected with Resend".to_string())
}

/// Returns a valid bearer credential: API key first, otherwise an OAuth access
/// token, refreshing it when close to expiry.
pub(crate) async fn get_credential(app: &AppHandle<Wry>) -> Result<String, String> {
    if let Some(api_key) = store::load_api_key(app)? {
        return Ok(api_key);
    }

    let record =
        store::load_oauth(app)?.ok_or_else(|| "[ERROR] Not authenticated".to_string())?;

    if unix_now() < record.expires_at {
        return Ok(record.access_token);
    }

    let form = [
        ("grant_type", "refresh_token"),
        ("client_id", record.client_id.as_str()),
        ("refresh_token", record.refresh_token.as_str()),
    ];

    let response = http_client()
        .post(format!("{RESEND_API_BASE}/oauth/token"))
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("[ERROR] Refresh request failed: {}", e))?;

    let status = response.status();
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("[ERROR] Invalid refresh response: {}", e))?;

    if !status.is_success() {
        let err: Result<TokenError, _> = serde_json::from_value(body.clone());

        if matches!(&err, Ok(e) if e.error == "invalid_grant") {
            store::clear_oauth(app)?;
            return Err("Resend authorization expired. Please reconnect from settings.".to_string());
        }

        return Err(match err {
            Ok(e) => format!("[ERROR] Refresh failed: {}", e.error),
            Err(_) => format!("[ERROR] Refresh failed (HTTP {})", status),
        });
    }

    let tokens: TokenSuccess = serde_json::from_value(body)
        .map_err(|e| format!("[ERROR] Invalid refresh payload: {}", e))?;

    // Rotation: always persist the newest refresh token.
    let new_record = store::OAuthRecord {
        client_id: record.client_id.clone(),
        refresh_token: tokens.refresh_token.unwrap_or_else(|| record.refresh_token.clone()),
        access_token: tokens.access_token.clone(),
        expires_at: unix_now() + tokens.expires_in - EXPIRY_MARGIN_SECS,
        scope: tokens.scope.unwrap_or(record.scope.clone()),
    };

    store::save_oauth(app, &new_record)?;

    Ok(new_record.access_token)
}

/// Revokes the refresh token (killing the whole grant) and clears local state.
#[tauri::command]
/// Full credential logout: revokes the OAuth grant (if any), removes any
/// stored API key and resets setup flags. Local data (identities, read
/// markers, cached setup) is intentionally preserved so it revives on
/// reconnect.
pub async fn disconnect_resend(app: AppHandle<Wry>) -> Result<(), String> {
    let record = store::load_oauth(&app)?;

    if let Some(record) = record {
        let form = [
            ("token", record.refresh_token.as_str()),
            ("client_id", record.client_id.as_str()),
        ];

        // Best effort: RFC 7009 says the endpoint always returns 200 anyway.
        let _ = http_client()
            .post(format!("{RESEND_API_BASE}/oauth/revoke"))
            .form(&form)
            .send()
            .await;
    }

    store::clear_oauth(&app)?;
    store::delete_api_key(app.clone())?;
    sstore::mark_setup_incomplete(app.clone())?;
    sstore::set_inbox_enabled(app.clone(), false)?;

    println!("[INFO] Resend disconnected (credentials cleared, local data preserved)");

    Ok(())
}

/// Parses query parameters out of a callback URL.
fn parse_query(url: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();

    if let Some(query) = url.split_once('?').map(|(_, q)| q) {
        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (key, value) = match pair.split_once('=') {
                Some((k, v)) => (k, v),
                None => (pair, ""),
            };
            map.insert(percent_decode(key), percent_decode(value));
        }
    }

    map
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }

    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{parse_query, percent_decode};

    #[test]
    fn parses_standard_callback_url() {
        let q = parse_query("onlysend://oauth/callback?code=abc123&state=xyz");
        assert_eq!(q.get("code").map(String::as_str), Some("abc123"));
        assert_eq!(q.get("state").map(String::as_str), Some("xyz"));
    }

    #[test]
    fn percent_decode_handles_encoded_characters() {
        assert_eq!(percent_decode("a%20b%2Bc"), "a b+c");
        assert_eq!(percent_decode("plain"), "plain");
    }

    #[test]
    fn percent_decode_ignores_invalid_sequences_without_panicking() {
        // Trailing % with missing hex digits must not panic.
        assert_eq!(percent_decode("100% of %2"), "100% of %2");
    }

    #[test]
    fn parse_query_empty_returns_no_params() {
        let q = parse_query("onlysend://oauth/callback");
        assert!(q.is_empty());
    }
}
