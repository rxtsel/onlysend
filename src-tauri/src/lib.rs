mod config;
mod email;
mod oauth;
mod permissions;
mod profile;
mod sent;
mod store;

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
            store::has_api_key,
            store::is_authenticated,
            store::get_connection_status,
            store::save_api_key,
            store::get_api_key,
            store::delete_api_key,
            oauth::connect_resend,
            oauth::disconnect_resend,
            permissions::probe_full_access,
            email::send_email,
            config::list_from_emails,
            config::create_from_email,
            config::update_from_email,
            config::delete_from_email,
            profile::get_profile,
            profile::save_profile_command,
            sent::list_sent_emails,
            sent::get_sent_email
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
