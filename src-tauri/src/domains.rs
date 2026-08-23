use resend_rs::types::{
    Domain, DomainCapabilityStatus, DomainRecord, DomainRecordStatus, DomainStatus,
};
use resend_rs::{list_opts::ListOptions, Resend};
use serde::Serialize;
use tauri::{AppHandle, Wry};

use crate::oauth;
use crate::permissions;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDto {
    /// "TXT" | "MX" | "CNAME" | ...
    pub record_type: String,
    pub name: String,
    pub value: String,
    /// "pending" | "verified" | "failed" | "temporary_failure" | "not_started"
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDto {
    pub id: String,
    pub name: String,
    /// "pending" | "verified" | "failed" | "not_started" |
    /// "partially_verified" | "partially_failed"
    pub status: String,
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
                .map(|record| match record {
                    DomainRecord::DomainSpfRecord(r) => RecordDto {
                        record_type: format!("{:?}", r.r#type),
                        name: r.name.clone(),
                        value: r.value.clone(),
                        status: record_status_str(r.status),
                    },
                    DomainRecord::DomainDkimRecord(r) => RecordDto {
                        record_type: format!("{:?}", r.r#type),
                        name: r.name.clone(),
                        value: r.value.clone(),
                        status: record_status_str(r.status),
                    },
                    DomainRecord::ReceivingRecord(r) => RecordDto {
                        record_type: format!("{:?}", r.r#type),
                        name: r.name.clone(),
                        value: r.value.clone(),
                        status: record_status_str(r.status),
                    },
                    DomainRecord::TrackingRecord(r) => RecordDto {
                        record_type: format!("{:?}", r.r#type),
                        name: r.name.clone(),
                        value: r.value.clone(),
                        status: record_status_str(r.status),
                    },
                    DomainRecord::TrackingCaaRecord(r) => RecordDto {
                        record_type: format!("{:?}", r.r#type),
                        name: r.name.clone(),
                        value: r.value.clone(),
                        status: record_status_str(r.status),
                    },
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
pub async fn create_domain(app: AppHandle<Wry>, name: String) -> Result<DomainDetailDto, String> {
    let resend = client(&app).await?;

    let domain = resend
        .domains
        .create(resend_rs::types::CreateDomainOptions::new(&name))
        .await
        .map_err(|e| permissions::map_resend_error("Failed to create domain", e))?;

    Ok(to_detail(&domain))
}

#[tauri::command]
pub async fn get_domain(app: AppHandle<Wry>, domain_id: String) -> Result<DomainDetailDto, String> {
    let resend = client(&app).await?;

    let domain = resend
        .domains
        .get(&domain_id)
        .await
        .map_err(|e| permissions::map_resend_error("Failed to fetch domain", e))?;

    Ok(to_detail(&domain))
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
