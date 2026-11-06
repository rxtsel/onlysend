//! Quit and window close must wait for the frontend's draft flush.
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, ExitRequestApi, Manager, Wry};

#[derive(Default)]
pub(crate) struct ExitState(AtomicBool);

impl ExitState {
    fn needs_save(&self, code: Option<i32>) -> bool {
        // The updater already flushes drafts; Tauri cannot prevent a restart.
        code != Some(tauri::RESTART_EXIT_CODE) && !self.0.load(Ordering::SeqCst)
    }
}

pub(crate) fn on_exit_requested(app: &AppHandle<Wry>, api: &ExitRequestApi, code: Option<i32>) {
    if app.state::<ExitState>().needs_save(code) {
        api.prevent_exit();
        if app.emit("app-exit-requested", ()).is_err() {
            eprintln!("[WARN] Could not request draft save; application remains open");
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn install_quit_menu(app: &AppHandle<Wry>) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{Menu, MenuItem};
    let menu = Menu::default(app)?;
    let items = menu.items()?;
    let app_menu = items
        .first()
        .and_then(|item| item.as_submenu())
        .ok_or("Missing application menu")?;
    let position = app_menu
        .items()?
        .len()
        .checked_sub(1)
        .ok_or("Missing Quit menu item")?;
    // The default app submenu ends in native Quit, which calls AppKit terminate directly.
    // shortcut: Dock/system termination may bypass this menu; add a native termination delegate if those paths must await drafts.
    app_menu.remove_at(position)?;
    app_menu.insert(
        &MenuItem::with_id(
            app,
            "save-and-quit",
            format!("Quit {}", app.package_info().name),
            true,
            Some("Cmd+Q"),
        )?,
        position,
    )?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if event.id().as_ref() == "save-and-quit" {
            app.exit(0);
        }
    });
    Ok(())
}

#[tauri::command]
pub fn request_app_exit(app: AppHandle<Wry>) {
    app.exit(0);
}

#[tauri::command]
pub fn complete_app_exit(app: AppHandle<Wry>, approved: bool) {
    app.state::<ExitState>().0.store(approved, Ordering::SeqCst);
    if approved {
        app.exit(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quit_requires_approval_but_verified_update_restart_does_not() {
        let state = ExitState::default();
        assert!(state.needs_save(None));
        assert!(state.needs_save(Some(0)));
        assert!(!state.needs_save(Some(tauri::RESTART_EXIT_CODE)));
        state.0.store(true, Ordering::SeqCst);
        assert!(!state.needs_save(Some(0)));
        state.0.store(false, Ordering::SeqCst);
        assert!(state.needs_save(Some(0)));
    }
}
