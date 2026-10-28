//! External navigation is an explicit action, never a webview navigation.
use tauri::{AppHandle, Wry};
#[cfg(not(target_os = "linux"))]
use tauri_plugin_opener::OpenerExt;

fn browser_url(value: &str) -> Result<reqwest::Url, String> {
    if value.chars().any(char::is_control) {
        return Err("Invalid browser URL".into());
    }
    let url = reqwest::Url::parse(value).map_err(|_| "Invalid browser URL")?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Only HTTP and HTTPS links without embedded credentials can be opened".into());
    }
    Ok(url)
}

#[cfg(target_os = "linux")]
fn browser_command(
    target: impl AsRef<std::ffi::OsStr>,
    app_dir: Option<&std::path::Path>,
) -> std::process::Command {
    let mut command = std::process::Command::new("gio");
    command.arg("open").arg(target);
    if let Some(app_dir) = app_dir {
        // AppImage libraries belong to OnlySend, not to the host browser/launcher.
        for key in [
            "LD_LIBRARY_PATH",
            "LD_PRELOAD",
            "GIO_MODULE_DIR",
            "GIO_EXTRA_MODULES",
            "GTK_PATH",
            "GTK_EXE_PREFIX",
            "GTK_DATA_PREFIX",
            "GDK_PIXBUF_MODULE_FILE",
            "GDK_PIXBUF_MODULEDIR",
            "GSETTINGS_SCHEMA_DIR",
        ] {
            command.env_remove(key);
        }
        for key in ["PATH", "XDG_DATA_DIRS"] {
            if let Some(value) = std::env::var_os(key) {
                let paths = std::env::split_paths(&value).filter(|path| !path.starts_with(app_dir));
                if let Ok(value) = std::env::join_paths(paths) {
                    command.env(key, value);
                }
            }
        }
    }
    command
}

#[cfg(target_os = "linux")]
fn launch_desktop(target: impl AsRef<std::ffi::OsStr>) -> Result<(), String> {
    let app_dir = std::env::var_os("APPDIR").map(std::path::PathBuf::from);
    let status = browser_command(target, app_dir.as_deref())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| format!("Could not start the desktop launcher (gio): {error}. Check that GLib tools are installed."))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "The system launcher failed ({status}). Check your default application associations."
        ))
    }
}

// Only the attachment service calls this, after checking the downloaded file's type.
pub(crate) async fn open_attachment_file(
    app: AppHandle<Wry>,
    path: std::path::PathBuf,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "linux")]
        {
            let _ = app;
            launch_desktop(path)
        }
        #[cfg(not(target_os = "linux"))]
        {
            app.opener()
                .open_path(path.to_string_lossy().into_owned(), None::<&str>)
                .map_err(|_| "Could not open the default application".to_string())
        }
    })
    .await
    .map_err(|_| "Could not run the desktop launcher".to_string())?
}

pub(crate) async fn open_browser(app: AppHandle<Wry>, value: String) -> Result<(), String> {
    let url = browser_url(&value)?;
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "linux")]
        {
            let _ = app;
            launch_desktop(url.as_str())
        }
        #[cfg(not(target_os = "linux"))]
        {
            app.opener()
                .open_url(url.as_str(), None::<&str>)
                .map_err(|_| "Could not open the default browser".to_string())
        }
    })
    .await
    .map_err(|_| "Could not run the browser launcher".to_string())?
}

#[tauri::command]
pub async fn open_external_url(app: AppHandle<Wry>, url: String) -> Result<(), String> {
    open_browser(app, url).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_links_require_web_urls_without_credentials_or_control_characters() {
        for url in [
            "https://example.com/logo?a=1&b=2#section",
            "http://localhost:1420/",
        ] {
            assert!(browser_url(url).is_ok());
        }
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,test",
            "mailto:user@example.com",
            "onlysend://oauth",
            "--help",
            "/tmp/file",
            "https://user:password@example.com",
            "https://example.com/\nsecret",
        ] {
            assert!(browser_url(url).is_err(), "accepted {url:?}");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn appimage_cleanup_is_child_only_and_keeps_the_url_as_one_argument() {
        let url = "https://example.com/?a=1&b=two";
        let command = browser_command(url, Some(std::path::Path::new("/tmp/OnlySend.AppDir")));
        assert_eq!(command.get_program(), "gio");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            vec![std::ffi::OsStr::new("open"), std::ffi::OsStr::new(url)]
        );
        assert!(command
            .get_envs()
            .any(|(key, value)| key == "LD_LIBRARY_PATH" && value.is_none()));
        assert!(!command
            .get_envs()
            .any(|(key, _)| key == "DBUS_SESSION_BUS_ADDRESS"
                || key == "DISPLAY"
                || key == "WAYLAND_DISPLAY"));
        assert!(!browser_command(url, None)
            .get_envs()
            .any(|(key, _)| key == "LD_LIBRARY_PATH"));
    }
}
