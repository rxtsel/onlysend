mod config;
mod domains;
mod email;
mod inbound;
mod oauth;
mod permissions;
mod sent;

pub mod infrastructure;

use tauri::Manager;
use tauri_plugin_store::StoreExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(desktop)]
    let builder = {
        // Must be the first plugin: forwards deep links from secondary
        // instances to this one (the deep-link event fires automatically).
        let builder = builder.plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}));
        builder.plugin(tauri_plugin_deep_link::init())
    };

    builder
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .manage(oauth::OAuthState::default())
        .setup(|app| {
            // Initialize the store
            let _store = app.store("settings.json")?;
            println!("🚀 Store initialized");

            // Register immediately, initialize off the async executor. Existing
            // JSON commands remain authoritative until explicit migrations land.
            let database = infrastructure::database::Database::new(
                app.path()
                    .app_data_dir()?
                    .join(infrastructure::database::DATABASE_FILE),
            );
            app.manage(database.clone());
            tauri::async_runtime::spawn(async move {
                if let Err(error) = database.initialize().await {
                    // Do not destroy JSON state or stop the app for a cache failure.
                    // Future database adapters retry and return the same error.
                    eprintln!("[WARN] Local database initialization failed: {error}");
                }
            });

            // D: Proactive credential warm-up — refresh before the UI fires
            // its parallel loads. Per-account single-flight makes this
            // race-safe. Silent: failures surface on actual use.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(account_id) = infrastructure::credentials_store::active_account_id(&handle) {
                    if let Err(err) = oauth::get_account_credential(&handle, &account_id).await {
                        println!("[INFO] Credential warm-up skipped: {err}");
                    }
                }
            });

            #[cfg(desktop)]
            {
                use tauri_plugin_deep_link::DeepLinkExt;

                // Deep links are only registered on install; force-register so
                // `tauri dev` and AppImage installs work out of the box.
                // Non-fatal: missing xdg tooling must not crash the app.
                #[cfg(any(target_os = "linux", all(debug_assertions, windows)))]
                if let Err(err) = app.deep_link().register_all() {
                    println!("[WARN] Could not register deep links: {err}");
                    println!("[WARN] 'Connect with Resend' may not work until the app is installed.");
                }

                let handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    for url in event.urls() {
                        oauth::handle_deep_link(handle.clone(), url.to_string());
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            infrastructure::credentials_store::has_api_key,
            infrastructure::settings_store::get_onboarding_state,
            infrastructure::settings_store::set_inbox_enabled,
            infrastructure::settings_store::get_inbound_setup_cache,
            infrastructure::settings_store::save_inbound_setup_cache,
            infrastructure::settings_store::mark_setup_complete,
            infrastructure::credentials_store::get_connection_status,
            infrastructure::credentials_store::save_api_key,
            infrastructure::credentials_store::get_api_key,
            infrastructure::credentials_store::delete_api_key,
            infrastructure::credentials_store::list_accounts,
            infrastructure::credentials_store::set_active_account,
            oauth::remove_account,
            infrastructure::settings_store::save_selected_domain,
            infrastructure::settings_store::get_selected_domain,
            infrastructure::settings_store::get_active_domain,
            oauth::connect_resend,
            oauth::disconnect_resend,
            permissions::probe_full_access,
            email::send_email,
            infrastructure::settings_store::get_domain_preferences,
            infrastructure::settings_store::save_domain_preferences,
            domains::list_domains,
            domains::create_domain,
            domains::delete_domain,
            domains::get_domain,
            domains::verify_domain,
            domains::set_domain_receiving,
            inbound::list_inbound_emails,
            inbound::get_inbound_email,
            infrastructure::settings_store::get_read_inbound_ids,
            infrastructure::settings_store::mark_inbound_read,
            config::list_from_emails,
            config::create_from_email,
            config::update_from_email,
            config::delete_from_email,
            sent::list_sent_emails,
            sent::get_sent_email
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
