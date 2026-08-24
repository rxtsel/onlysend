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

async fn client(app: &AppHandle<Wry>) -> Result<Resend, String> {
    let credential = oauth::get_credential(app).await?;
    Ok(Resend::new(&credential))
}

#[tauri::command]
pub async fn list_domains(app: AppHandle<Wry>) -> Result<Vec<DomainDto>, String> {
    let resend = client(&app).await?;

    let response = resend
        .domains
        .list(ListOptions::default())
        .await
        .map_err(|e| permissions::map_resend_error("Failed to list domains", e))?;

    Ok(response.data.iter().map(to_domain_dto).collect())
}

#[tauri::command]
pub async fn create_domain(
    app: AppHandle<Wry>,
    name: String,
    region: Option<String>,
    enable_receiving: Option<bool>,
) -> Result<DomainDetailDto, String> {
    let enable_receiving = enable_receiving.unwrap_or(false);

    // Without inbound, the plain SDK path covers everything.
    if !enable_receiving {
        let resend = client(&app).await?;

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
    let credential = oauth::get_credential(&app).await?;

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
pub async fn get_domain(app: AppHandle<Wry>, domain_id: String) -> Result<DomainDetailDto, String> {
    let resend = client(&app).await?;

    match resend.domains.get(&domain_id).await {
        Ok(domain) => Ok(to_detail(&domain)),
        Err(crate_err) => {
            // Fallback: fetch raw and map leniently, so an unexpected field
            // in the SDK's typed struct can't break the feature. The raw
            // body is logged to diagnose the crate failure.
            println!("[INFO] domains.get via SDK failed (raw fallback in use): {crate_err}");
            get_domain_raw(&app, &domain_id).await
        }
    }
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

async fn get_domain_raw(
    app: &AppHandle<Wry>,
    domain_id: &str,
) -> Result<DomainDetailDto, String> {
    let credential = oauth::get_credential(app).await?;

    let http = reqwest::Client::builder()
        .user_agent(concat!("OnlySend/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("[ERROR] Failed to build http client: {}", e))?;

    let response = http
        .get(format!("https://api.resend.com/domains/{domain_id}"))
        .bearer_auth(&credential)
        .send()
        .await
        .map_err(|e| format!("[ERROR] Domain request failed: {e}"))?;

    let status = response.status();
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("[ERROR] Invalid domain response: {e}"))?;

    if !status.is_success() {
        println!("[WARN] Raw domain body on failure: {body}");
        return Err(format!(
            "[ERROR] Resend rejected the domain lookup (HTTP {}): {}",
            status,
            body.get("message").and_then(|v| v.as_str()).unwrap_or("")
        ));
    }

    let detail = parse_raw_domain(body)?;
    println!(
        "[INFO] Raw domain fallback used for {}: status={}",
        detail.name, detail.status
    );
    Ok(detail)
}

/// Lenient parser for raw `/domains` payloads (create + get fallback).
fn parse_raw_domain(body: serde_json::Value) -> Result<DomainDetailDto, String> {
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
    app: AppHandle<Wry>,
    domain_id: String,
) -> Result<bool, String> {
    let resend = client(&app).await?;

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
    app: AppHandle<Wry>,
    domain_id: String,
    enable: bool,
) -> Result<DomainDetailDto, String> {
    let credential = oauth::get_credential(&app).await?;

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

    get_domain(app, domain_id).await
}

#[tauri::command]
pub async fn verify_domain(
    app: AppHandle<Wry>,
    domain_id: String,
) -> Result<DomainDetailDto, String> {
    let resend = client(&app).await?;

    // Triggers the asynchronous verification process.
    let _ = resend
        .domains
        .verify(&domain_id)
        .await
        .map_err(|e| permissions::map_resend_error("Failed to trigger verification", e))?;

    // Return the (now pending) state so the UI can start polling.
    get_domain(app, domain_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

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
