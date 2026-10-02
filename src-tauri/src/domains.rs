use resend_rs::types::{
    Domain, DomainCapabilityStatus, DomainRecord, DomainRecordStatus, DomainStatus,
};
use resend_rs::{list_opts::ListOptions, Resend};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Wry};

use crate::oauth;
use crate::permissions;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDto {
    /// Record group: "SPF" | "DKIM" | "Receiving MX" | "Tracking" | ...
    pub group: String,
    /// "TXT" | "MX" | "CNAME" | ...
    pub record_type: String,
    pub name: String,
    pub value: String,
    /// "pending" | "verified" | "failed" | "temporary_failure" | "not_started"
    pub status: String,
    pub ttl: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDto {
    pub id: String,
    pub name: String,
    /// "pending" | "verified" | "failed" | "not_started" |
    /// "partially_verified" | "partially_failed"
    pub status: String,
    pub capabilities: CapabilitiesDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesDto {
    pub sending: String,
    pub receiving: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDetailDto {
    pub id: String,
    pub name: String,
    pub status: String,
    pub capabilities: CapabilitiesDto,
    pub records: Vec<RecordDto>,
}

fn capability_str(status: DomainCapabilityStatus) -> String {
    match status {
        DomainCapabilityStatus::Enabled => "enabled".to_string(),
        DomainCapabilityStatus::Disabled => "disabled".to_string(),
        _ => format!("{status:?}").to_lowercase(),
    }
}

fn to_detail(domain: &Domain) -> DomainDetailDto {
    DomainDetailDto {
        id: domain.id.to_string(),
        name: domain.name.clone(),
        status: domain_status_str(domain.status),
        capabilities: CapabilitiesDto {
            sending: capability_str(domain.capabilities.sending),
            receiving: capability_str(domain.capabilities.receiving),
        },
        records: map_records(domain),
    }
}

fn domain_status_str(status: DomainStatus) -> String {
    match status {
        DomainStatus::Pending => "pending".to_string(),
        DomainStatus::Verified => "verified".to_string(),
        DomainStatus::Failed => "failed".to_string(),
        DomainStatus::NotStarted => "not_started".to_string(),
        DomainStatus::PartiallyVerified => "partially_verified".to_string(),
        DomainStatus::PartiallyFailed => "partially_failed".to_string(),
    }
}

fn record_status_str(status: DomainRecordStatus) -> String {
    match status {
        DomainRecordStatus::Pending => "pending".to_string(),
        DomainRecordStatus::Verified => "verified".to_string(),
        DomainRecordStatus::Failed => "failed".to_string(),
        DomainRecordStatus::TemporaryFailure => "temporary_failure".to_string(),
        DomainRecordStatus::NotStarted => "not_started".to_string(),
    }
}

fn map_records(domain: &Domain) -> Vec<RecordDto> {
    domain
        .records
        .as_ref()
        .map(|records| {
            records
                .iter()
                .map(|record| {
                    let (group, record_type, name, value, status, ttl, priority) = match record {
                        DomainRecord::DomainSpfRecord(r) => (
                            "SPF",
                            format!("{:?}", r.r#type),
                            &r.name,
                            &r.value,
                            record_status_str(r.status),
                            r.ttl.clone(),
                            r.priority,
                        ),
                        DomainRecord::DomainDkimRecord(r) => (
                            "DKIM",
                            format!("{:?}", r.r#type),
                            &r.name,
                            &r.value,
                            record_status_str(r.status),
                            r.ttl.clone(),
                            None,
                        ),
                        DomainRecord::ReceivingRecord(r) => (
                            "Receiving MX",
                            format!("{:?}", r.r#type),
                            &r.name,
                            &r.value,
                            record_status_str(r.status),
                            r.ttl.clone(),
                            Some(r.priority),
                        ),
                        DomainRecord::TrackingRecord(r) => (
                            "Tracking",
                            format!("{:?}", r.r#type),
                            &r.name,
                            &r.value,
                            record_status_str(r.status),
                            r.ttl.clone(),
                            None,
                        ),
                        DomainRecord::TrackingCaaRecord(r) => (
                            "Tracking CAA",
                            format!("{:?}", r.r#type),
                            &r.name,
                            &r.value,
                            record_status_str(r.status),
                            r.ttl.clone(),
                            None,
                        ),
                    };

                    RecordDto {
                        group: group.to_string(),
                        record_type,
                        name: name.clone(),
                        value: value.clone(),
                        status,
                        ttl,
                        priority,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn to_domain_dto(domain: &Domain) -> DomainDto {
    DomainDto {
        id: domain.id.to_string(),
        name: domain.name.clone(),
        status: domain_status_str(domain.status),
        capabilities: CapabilitiesDto {
            sending: capability_str(domain.capabilities.sending),
            receiving: capability_str(domain.capabilities.receiving),
        },
    }
}

async fn client(app: &AppHandle<Wry>, account_id: &str) -> Result<Resend, String> {
    let credential = oauth::get_account_credential(app, account_id).await?;
    Ok(Resend::new(&credential))
}

#[tauri::command]
pub async fn list_domains(app: AppHandle<Wry>, account_id: String) -> Result<Vec<DomainDto>, String> {
    let resend = client(&app, &account_id).await?;

    let response = resend
        .domains
        .list(ListOptions::default())
        .await
        .map_err(|e| permissions::map_resend_error("Failed to list domains", e))?;

    Ok(response.data.iter().map(to_domain_dto).collect())
}

#[tauri::command]
pub async fn create_domain(
    app: AppHandle<Wry>, account_id: String,
    name: String,
    region: Option<String>,
    enable_receiving: Option<bool>,
) -> Result<DomainDetailDto, String> {
    let enable_receiving = enable_receiving.unwrap_or(false);

    // Without inbound, the plain SDK path covers everything.
    if !enable_receiving {
        let resend = client(&app, &account_id).await?;

        let mut options = resend_rs::types::CreateDomainOptions::new(&name);
        if let Some(r) = region.as_deref().and_then(region_to_enum) {
            options = options.with_region(r);
        }

        let domain = resend
            .domains
            .create(options)
            .await
            .map_err(|e| permissions::map_resend_error("Failed to create domain", e))?;

        return Ok(to_detail(&domain));
    }

    // With inbound ON we must send capabilities in the same request, and the
    // SDK has no setter for them: raw POST it is (single request).
    let credential = oauth::get_account_credential(&app, &account_id).await?;

    let http = reqwest::Client::builder()
        .user_agent(concat!("OnlySend/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("[ERROR] Failed to build http client: {}", e))?;

    let mut body = serde_json::json!({
        "name": name,
        "capabilities": {
            "sending": "enabled",
            "receiving": "enabled",
        }
    });
    if let Some(r) = &region {
        body["region"] = serde_json::json!(r);
    }

    let response = http
        .post("https://api.resend.com/domains")
        .bearer_auth(&credential)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("[ERROR] Domain creation failed: {e}"))?;

    let status = response.status();
    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("[ERROR] Invalid domain response: {e}"))?;

    if !status.is_success() {
        return Err(format!(
            "[ERROR] Resend rejected the domain creation (HTTP {}): {}",
            status,
            payload.get("message").and_then(|v| v.as_str()).unwrap_or("")
        ));
    }

    parse_raw_domain(payload)
}

fn region_to_enum(value: &str) -> Option<resend_rs::types::Region> {
    use resend_rs::types::Region;
    match value {
        "us-east-1" => Some(Region::UsEast1),
        "eu-west-1" => Some(Region::EuWest1),
        "sa-east-1" => Some(Region::SaEast1),
        "ap-northeast-1" => Some(Region::ApNorthEast1),
        _ => None,
    }
}

#[tauri::command]
pub async fn get_domain(app: AppHandle<Wry>, account_id: String, domain_id: String) -> Result<DomainDetailDto, String> {
    // Resolve once. Both parsers consume the same response and credential;
    // switching accounts cannot retarget a fallback request.
    let credential = oauth::get_account_credential(&app, &account_id).await?;
    fetch_domain(&crate::infrastructure::http::client(), "https://api.resend.com", &credential, &domain_id).await
}

#[derive(Deserialize)]
struct RawDomain {
    id: String,
    name: String,
    status: serde_json::Value,
    #[serde(default)]
    capabilities: Option<serde_json::Value>,
    #[serde(default)]
    records: Option<serde_json::Value>,
}

fn json_str(value: &serde_json::Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}

async fn fetch_domain(
    http: &reqwest::Client,
    base_url: &str,
    credential: &str,
    domain_id: &str,
) -> Result<DomainDetailDto, String> {
    let response = http
        .get(format!("{base_url}/domains/{domain_id}"))
        .bearer_auth(credential)
        .send()
        .await
        .map_err(|e| format!("[ERROR] Domain request failed: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        // Do not log or echo arbitrary response bodies (or retry auth/429/5xx).
        return Err(format!("[ERROR] Resend rejected the domain lookup (HTTP {status})"));
    }
    let body: serde_json::Value = response.json().await
        .map_err(|_| "[ERROR] Invalid domain JSON response".to_string())?;
    let (detail, diagnostic) = decode_domain(body)?;
    if let Some(diagnostic) = diagnostic {
        println!("[WARN] Domain SDK schema mismatch: {diagnostic}; compatible parser used (same response)");
    }
    Ok(detail)
}

// Only schema names, array indexes and fixed reason codes may enter logs.
// serde's full error message can contain arbitrary values from the response.
const DOMAIN_FIELDS: &[&str] = &[
    "id", "name", "status", "created_at", "region", "capabilities", "sending",
    "receiving", "records", "record", "type", "value", "ttl", "priority",
    "open_tracking", "click_tracking", "tracking_subdomain",
];

fn schema_diagnostic(error: &serde_path_to_error::Error<serde_json::Error>) -> String {
    use serde_path_to_error::Segment;
    let mut path = "$".to_string();
    for segment in error.path().iter() {
        match segment {
            Segment::Seq { index } => path.push_str(&format!("[{index}]")),
            Segment::Map { key } if DOMAIN_FIELDS.contains(&key.as_str()) => {
                path.push('.');
                path.push_str(key);
            }
            _ => path.push_str(".<unknown>"),
        }
    }
    let message = error.inner().to_string();
    let reason = if let Some(field) = message.strip_prefix("missing field `").and_then(|s| s.split('`').next()) {
        if DOMAIN_FIELDS.contains(&field) {
            path.push('.');
            path.push_str(field);
        }
        "missing_field"
    } else if message.starts_with("unknown variant") {
        "unsupported_variant"
    } else if message.starts_with("invalid type") {
        "invalid_type"
    } else {
        "incompatible_value"
    };
    format!("path={path} reason={reason}")
}

/// The live API uses "Receiving" while resend-rs (and our DTO contract)
/// names the same MX group "Receiving MX". Normalize only this known alias;
/// DNS names, values, types, statuses and unknown future groups stay untouched.
fn normalize_receiving_record_group(body: &mut serde_json::Value) {
    if let Some(records) = body.get_mut("records").and_then(serde_json::Value::as_array_mut) {
        for record in records {
            if let Some(group) = record.get_mut("record") {
                if group.as_str() == Some("Receiving") {
                    *group = serde_json::Value::String("Receiving MX".into());
                }
            }
        }
    }
}

fn decode_domain(mut body: serde_json::Value) -> Result<(DomainDetailDto, Option<String>), String> {
    normalize_receiving_record_group(&mut body);
    match serde_path_to_error::deserialize::<_, Domain>(&body) {
        Ok(domain) => Ok((to_detail(&domain), None)),
        Err(error) => {
            let diagnostic = schema_diagnostic(&error);
            let detail = parse_raw_domain(body)
                .map_err(|_| format!("[ERROR] Invalid domain payload ({diagnostic})"))?;
            Ok((detail, Some(diagnostic)))
        }
    }
}

/// Lenient parser for raw `/domains` payloads (create + same-response compatibility).
fn parse_raw_domain(mut body: serde_json::Value) -> Result<DomainDetailDto, String> {
    normalize_receiving_record_group(&mut body);
    let raw: RawDomain = serde_json::from_value(body)
        .map_err(|e| format!("[ERROR] Unexpected domain payload: {e}"))?;

    let records = raw
        .records
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .map(|rec| RecordDto {
                    group: rec
                        .get("record")
                        .map(json_str)
                        .unwrap_or_else(|| "UNKNOWN".into()),
                    record_type: rec
                        .get("type")
                        .map(json_str)
                        .unwrap_or_else(|| "UNKNOWN".into()),
                    name: rec.get("name").map(json_str).unwrap_or_default(),
                    value: rec.get("value").map(json_str).unwrap_or_default(),
                    status: rec
                        .get("status")
                        .map(json_str)
                        .unwrap_or_else(|| "not_started".into()),
                    ttl: rec.get("ttl").map(json_str).unwrap_or_default(),
                    priority: rec
                        .get("priority")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32),
                })
                .collect()
        })
        .unwrap_or_default();

    let (sending, receiving) = raw
        .capabilities
        .as_ref()
        .map(|c| {
            (
                c.get("sending")
                    .map(json_str)
                    .unwrap_or_else(|| "disabled".into()),
                c.get("receiving")
                    .map(json_str)
                    .unwrap_or_else(|| "disabled".into()),
            )
        })
        .unwrap_or_else(|| ("disabled".into(), "disabled".into()));

    Ok(DomainDetailDto {
        id: raw.id,
        name: raw.name,
        status: json_str(&raw.status),
        capabilities: CapabilitiesDto {
            sending,
            receiving,
        },
        records,
    })
}

/// Permanently deletes a domain from Resend.
#[tauri::command]
pub async fn delete_domain(
    app: AppHandle<Wry>, account_id: String,
    domain_id: String,
) -> Result<bool, String> {
    let resend = client(&app, &account_id).await?;

    let response = resend
        .domains
        .delete(&domain_id)
        .await
        .map_err(|e| permissions::map_resend_error("Failed to delete domain", e))?;

    println!(
        "[INFO] Domain {} deleted: {}",
        response.id, response.deleted
    );

    Ok(response.deleted)
}

/// Enables or disables receiving (inbound) for a domain.
///
/// Uses a raw PATCH because resend-rs does not expose a capabilities setter.
#[tauri::command]
pub async fn set_domain_receiving(
    app: AppHandle<Wry>, account_id: String,
    domain_id: String,
    enable: bool,
) -> Result<DomainDetailDto, String> {
    let credential = oauth::get_account_credential(&app, &account_id).await?;

    let http = reqwest::Client::builder()
        .user_agent(concat!("OnlySend/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("[ERROR] Failed to build http client: {}", e))?;

    let body = serde_json::json!({
        "capabilities": {
            "sending": "enabled",
            "receiving": if enable { "enabled" } else { "disabled" },
        }
    });

    let response = http
        .patch(format!(
            "https://api.resend.com/domains/{domain_id}"
        ))
        .bearer_auth(&credential)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("[ERROR] Failed to update receiving capability: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "[ERROR] Resend rejected the receiving update (HTTP {})",
            response.status()
        ));
    }

    get_domain(app, account_id, domain_id).await
}

#[tauri::command]
pub async fn verify_domain(
    app: AppHandle<Wry>, account_id: String,
    domain_id: String,
) -> Result<DomainDetailDto, String> {
    let resend = client(&app, &account_id).await?;

    // Triggers the asynchronous verification process.
    let _ = resend
        .domains
        .verify(&domain_id)
        .await
        .map_err(|e| permissions::map_resend_error("Failed to trigger verification", e))?;

    // Return the (now pending) state so the UI can start polling.
    get_domain(app, account_id, domain_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sdk_payload() -> serde_json::Value {
        serde_json::json!({
            "id": "d91cd9bd-1176-453e-8fc1-35364d380206",
            "name": "example.com", "status": "verified",
            "created_at": "2026-01-01T00:00:00Z", "region": "us-east-1",
            "capabilities": { "sending": "enabled", "receiving": "enabled" },
            "records": []
        })
    }

    #[test]
    fn sdk_compatible_response_needs_no_fallback() {
        let (detail, diagnostic) = decode_domain(sdk_payload()).unwrap();
        assert_eq!(detail.status, "verified");
        assert!(diagnostic.is_none());
    }

    fn receiving_payload(group: &str) -> serde_json::Value {
        let mut body = sdk_payload();
        body["records"] = serde_json::json!([{
            "record": group, "name": "inbound.example.com", "type": "MX",
            "ttl": "Auto", "status": "verified", "priority": 10,
            "value": "inbound-smtp.us-east-1.amazonaws.com"
        }]);
        body
    }

    #[test]
    fn live_receiving_alias_and_sdk_label_decode_without_fallback() {
        for group in ["Receiving", "Receiving MX"] {
            let body = receiving_payload(group);
            let (sdk_detail, diagnostic) = decode_domain(body.clone()).unwrap();
            assert!(diagnostic.is_none(), "{diagnostic:?}");
            let raw_detail = parse_raw_domain(body).unwrap();
            for detail in [sdk_detail, raw_detail] {
                let record = &detail.records[0];
                assert_eq!(record.group, "Receiving MX");
                assert_eq!(record.record_type, "MX");
                assert_eq!(record.name, "inbound.example.com");
                assert_eq!(record.value, "inbound-smtp.us-east-1.amazonaws.com");
                assert_eq!(record.priority, Some(10));
                assert_eq!(record.status, "verified");
            }
        }
    }

    #[test]
    fn receiving_normalization_only_changes_the_known_tag_and_is_idempotent() {
        let mut body = receiving_payload("Receiving");
        let mut expected = body.clone();
        expected["records"][0]["record"] = serde_json::json!("Receiving MX");
        normalize_receiving_record_group(&mut body);
        assert_eq!(body, expected);
        normalize_receiving_record_group(&mut body);
        assert_eq!(body, expected);
    }

    #[test]
    fn unknown_record_groups_still_report_schema_diagnostics() {
        let body = receiving_payload("FutureRecord");
        let (detail, diagnostic) = decode_domain(body).unwrap();
        assert_eq!(detail.records[0].group, "FutureRecord");
        assert_eq!(diagnostic.as_deref(), Some("path=$.records[0].record reason=unsupported_variant"));
    }

    #[test]
    fn incompatible_region_reports_path_without_response_values() {
        let mut body = sdk_payload();
        body["region"] = serde_json::json!("private-value-not-for-logs");
        let (detail, diagnostic) = decode_domain(body).unwrap();
        assert_eq!(detail.name, "example.com");
        let diagnostic = diagnostic.unwrap();
        assert!(diagnostic.contains("$.region"));
        assert!(diagnostic.contains("unsupported_variant"));
        assert!(!diagnostic.contains("private-value"));
        assert!(!diagnostic.contains("example.com"));
    }

    #[test]
    fn missing_sdk_field_is_identified() {
        let mut body = sdk_payload();
        body.as_object_mut().unwrap().remove("created_at");
        let (_, diagnostic) = decode_domain(body).unwrap();
        assert_eq!(diagnostic.unwrap(), "path=$.created_at reason=missing_field");
    }

    #[test]
    fn malformed_required_field_fails_without_leaking_value() {
        let mut body = sdk_payload();
        body["id"] = serde_json::json!({ "secret": "private-value" });
        let error = decode_domain(body).unwrap_err();
        assert!(!error.contains("private-value"));
        assert!(error.contains("$.id"));
    }

    // A server accepting exactly ONE request: any second fallback request
    // fails, so a successful lookup proves both parsers shared the response.
    fn one_response_server(status: &str, body: String) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_owned();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            drop(listener);
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            let mut buf = [0; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&buf[..n]);
            }
            write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        (format!("http://{address}"), server)
    }

    #[tokio::test]
    async fn schema_mismatch_uses_only_one_http_request() {
        let mut body = sdk_payload();
        body["region"] = serde_json::json!("future-region");
        let (url, server) = one_response_server("200 OK", body.to_string());
        let client = reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(5)).build().unwrap();
        let result = fetch_domain(&client, &url, "test-credential", "domain-id").await;
        server.join().unwrap();
        assert_eq!(result.unwrap().status, "verified");
    }

    #[tokio::test]
    async fn http_error_is_not_retried_or_echoed() {
        let (url, server) = one_response_server("403 Forbidden", "private-response-body".into());
        let client = reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(5)).build().unwrap();
        let result = fetch_domain(&client, &url, "test-credential", "domain-id").await;
        server.join().unwrap();
        let error = result.unwrap_err();
        assert!(error.contains("403"));
        assert!(!error.contains("private-response-body"));
    }

    #[test]
    fn region_to_enum_maps_all_known_regions() {
        assert!(matches!(
            region_to_enum("us-east-1"),
            Some(resend_rs::types::Region::UsEast1)
        ));
        assert!(matches!(
            region_to_enum("eu-west-1"),
            Some(resend_rs::types::Region::EuWest1)
        ));
        assert!(matches!(
            region_to_enum("sa-east-1"),
            Some(resend_rs::types::Region::SaEast1)
        ));
        assert!(matches!(
            region_to_enum("ap-northeast-1"),
            Some(resend_rs::types::Region::ApNorthEast1)
        ));
    }

    #[test]
    fn region_to_enum_rejects_unknown_regions() {
        assert!(region_to_enum("mars-1").is_none());
        assert!(region_to_enum("").is_none());
    }

    #[test]
    fn parse_raw_domain_full_payload() {
        let body = serde_json::json!({
            "id": "d1",
            "name": "example.com",
            "status": "pending",
            "capabilities": { "sending": "enabled", "receiving": "disabled" },
            "records": [
                {
                    "record": "SPF",
                    "name": "send",
                    "type": "MX",
                    "value": "feedback-smtp.us-east-1.amazonses.com",
                    "ttl": "Auto",
                    "status": "not_started",
                    "priority": 10
                },
                {
                    "record": "DKIM",
                    "name": "resend._domainkey",
                    "type": "TXT",
                    "value": "p=abc",
                    "ttl": "Auto",
                    "status": "verified"
                },
                {
                    "record": "Receiving MX",
                    "name": "example.com",
                    "type": "MX",
                    "value": "inbound-smtp.us-east-1.amazonaws.com",
                    "ttl": "Auto",
                    "status": "pending",
                    "priority": 10
                }
            ]
        });

        let d = parse_raw_domain(body).expect("should parse");

        assert_eq!(d.id, "d1");
        assert_eq!(d.name, "example.com");
        assert_eq!(d.status, "pending");
        assert_eq!(d.capabilities.sending, "enabled");
        assert_eq!(d.capabilities.receiving, "disabled");
        assert_eq!(d.records.len(), 3);

        let spf = &d.records[0];
        assert_eq!(spf.group, "SPF");
        assert_eq!(spf.record_type, "MX");
        assert_eq!(spf.priority, Some(10));

        let dkim = &d.records[1];
        assert_eq!(dkim.group, "DKIM");
        assert_eq!(dkim.status, "verified");
        assert_eq!(dkim.priority, None);
    }

    #[test]
    fn parse_raw_domain_missing_optionals_defaults() {
        let body = serde_json::json!({
            "id": "d2",
            "name": "bare.dev",
            "status": "not_started"
        });

        let d = parse_raw_domain(body).expect("should parse minimal payload");

        assert_eq!(d.records.len(), 0);
        assert_eq!(d.capabilities.sending, "disabled");
        assert_eq!(d.capabilities.receiving, "disabled");
    }

    #[test]
    fn parse_raw_domain_unknown_record_group_is_preserved() {
        let body = serde_json::json!({
            "id": "d3",
            "name": "x.dev",
            "status": "verified",
            "records": [
                { "record": "SomethingNew", "type": "TXT", "name": "n", "value": "v", "status": "pending" }
            ]
        });

        let d = parse_raw_domain(body).expect("should parse");
        // Unknown groups must survive so the UI can show whatever Resend adds.
        assert_eq!(d.records[0].group, "SomethingNew");
        assert_eq!(d.records[0].status, "pending");
    }
}
