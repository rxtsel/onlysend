//! Process-wide, account-independent updates. No network polling or automatic install.
use serde::Serialize;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{ipc::Channel, AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::{Mutex as AsyncMutex, OnceCell};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current_version: String,
    state: &'static str,
    version: Option<String>,
    notes: Option<String>,
    error: Option<String>,
    install_supported: bool,
    downloaded: bool,
    release_url: &'static str,
}

#[derive(Default)]
pub struct UpdateState {
    checked: OnceCell<UpdateInfo>,
    latest: Mutex<Option<UpdateInfo>>,
    pending: Mutex<Option<Update>>,
    downloaded: Mutex<Option<Arc<Vec<u8>>>>,
    installing: Arc<AsyncMutex<()>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
}

fn public_key_configured(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|value| value.get("pubkey"))
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty())
}

fn install_supported() -> bool {
    // Debian installs remain owned by the system package manager.
    !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some()
}

fn check_enabled(debug_build: bool, has_public_key: bool) -> bool {
    !debug_build && has_public_key
}

async fn check(app: &AppHandle, state: &UpdateState) -> UpdateInfo {
    let mut info = UpdateInfo {
        current_version: app.package_info().version.to_string(),
        state: "disabled",
        version: None,
        notes: None,
        error: None,
        install_supported: install_supported(),
        downloaded: false,
        release_url: "https://github.com/rxtsel/onlysend/releases/latest",
    };
    if !check_enabled(cfg!(debug_assertions), public_key_configured(app)) {
        return info;
    }
    // A short metadata deadline, but a longer deadline for binary downloads.
    let Ok(_permit) = state.installing.try_lock() else {
        info.state = "error";
        info.error = Some("An update operation is already running".into());
        return info;
    };
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        app.updater_builder()
            .timeout(Duration::from_secs(180))
            .build()?
            .check()
            .await
    })
    .await
    .map_err(|_| "Update check timed out".to_string())
    .and_then(|result| result.map_err(|error| error.to_string()));
    match result {
        Ok(Some(update)) => {
            info.version = Some(update.version.clone());
            info.notes = update.body.clone();
            match state.pending.lock() {
                Ok(mut pending) => {
                    let changed = pending.as_ref().is_none_or(|previous| {
                        previous.version != update.version
                            || previous.signature != update.signature
                            || previous.download_url != update.download_url
                    });
                    if changed {
                        if let Ok(mut bytes) = state.downloaded.lock() {
                            *bytes = None;
                        }
                    }
                    *pending = Some(update);
                    info.state = "available";
                }
                Err(_) => {
                    info.state = "error";
                    info.error = Some("Update state is unavailable".into());
                }
            }
        }
        Ok(None) => {
            info.state = "current";
            if let Ok(mut pending) = state.pending.lock() {
                *pending = None;
            }
            if let Ok(mut bytes) = state.downloaded.lock() {
                *bytes = None;
            }
        }
        Err(error) => {
            info.state = "error";
            info.error = Some(error.to_string());
            if let Ok(pending) = state.pending.lock() {
                if let Some(update) = pending.as_ref() {
                    info.version = Some(update.version.clone());
                    info.notes = update.body.clone();
                }
            }
        }
    }
    info.downloaded = state.downloaded.lock().is_ok_and(|bytes| bytes.is_some());
    if let Ok(mut latest) = state.latest.lock() {
        *latest = Some(info.clone());
    }
    info
}

#[tauri::command]
pub async fn check_app_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<UpdateInfo, String> {
    // Cache successes AND failures. Navigation, account switching and frontend
    // remounts cannot trigger another request during this process lifetime.
    let initial = state
        .checked
        .get_or_init(|| check(&app, &state))
        .await
        .clone();
    Ok(state
        .latest
        .lock()
        .map_err(|_| "Update state is unavailable")?
        .clone()
        .unwrap_or(initial))
}

/// Explicit user requests are separate from the single automatic startup check.
#[tauri::command]
pub async fn refresh_app_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<UpdateInfo, String> {
    Ok(check(&app, &state).await)
}

#[tauri::command]
pub async fn download_app_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
    on_progress: Channel<UpdateProgress>,
) -> Result<(), String> {
    if cfg!(debug_assertions) || !public_key_configured(&app) || !install_supported() {
        return Err("In-app installation is unavailable for this build".into());
    }
    let _permit = state
        .installing
        .try_lock()
        .map_err(|_| "An update is already installing")?;
    if state
        .downloaded
        .lock()
        .map_err(|_| "Update state is unavailable")?
        .is_some()
    {
        return Ok(());
    }
    // Keep the pending update on failure, so an explicitly requested retry is possible.
    let update = state
        .pending
        .lock()
        .map_err(|_| "Update state is unavailable")?
        .clone()
        .ok_or("No update is available")?;
    let progress = on_progress.clone();
    let mut downloaded = 0_u64;
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded = downloaded.saturating_add(chunk as u64);
                let _ = progress.send(UpdateProgress {
                    phase: "downloading",
                    downloaded,
                    total,
                });
            },
            move || {
                let _ = on_progress.send(UpdateProgress {
                    phase: "verifying",
                    downloaded: 0,
                    total: None,
                });
            },
        )
        .await
        .map_err(|error| error.to_string())?;
    // The plugin has verified the signature before returning these bytes.
    *state
        .downloaded
        .lock()
        .map_err(|_| "Update state is unavailable")? = Some(Arc::new(bytes));
    Ok(())
}

#[tauri::command]
pub async fn install_app_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<(), String> {
    if cfg!(debug_assertions) || !public_key_configured(&app) || !install_supported() {
        return Err("In-app installation is unavailable for this build".into());
    }
    let permit = state
        .installing
        .clone()
        .try_lock_owned()
        .map_err(|_| "An update is already installing")?;
    let update = state
        .pending
        .lock()
        .map_err(|_| "Update state is unavailable")?
        .clone()
        .ok_or("No update is available")?;
    let bytes = state
        .downloaded
        .lock()
        .map_err(|_| "Update state is unavailable")?
        .clone()
        .ok_or("Download and verify the update before installing")?;
    tauri::async_runtime::spawn_blocking(move || {
        // Retain the lock even if the invoking future is cancelled mid-install.
        let _permit = permit;
        update.install(bytes.as_slice())
    })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    // On Windows the updater exits the app itself. macOS/Linux restart only
    // after a signature-verified, successful install. App data is not touched.
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn snapshot(state: &'static str) -> UpdateInfo {
        UpdateInfo {
            current_version: "0.1.1".into(), state, version: None,
            notes: None, error: Some("offline".into()), install_supported: true,
            downloaded: false, release_url: "https://github.com/rxtsel/onlysend/releases/latest",
        }
    }

    #[test]
    fn never_checks_unsigned_or_development_builds() {
        assert!(!check_enabled(true, true));
        assert!(!check_enabled(false, false));
        assert!(check_enabled(false, true));
    }

    #[tokio::test]
    async fn concurrent_startup_calls_cache_failures_as_well_as_successes() {
        for status in ["error", "current"] {
            let state = UpdateState::default();
            let calls = AtomicUsize::new(0);
            let initialize = || async {
                calls.fetch_add(1, Ordering::SeqCst);
                tokio::task::yield_now().await;
                snapshot(status)
            };
            let (first, second) = tokio::join!(state.checked.get_or_init(initialize), state.checked.get_or_init(initialize));
            assert_eq!(first.state, status);
            assert_eq!(second.state, status);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn serializes_version_and_verified_state_for_both_footer_and_about() {
        let value = serde_json::to_value(snapshot("error")).unwrap();
        assert_eq!(value["currentVersion"], "0.1.1");
        assert_eq!(value["downloaded"], false);
        assert!(value["version"].is_null());
    }
}
