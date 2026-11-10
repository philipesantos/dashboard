//! The app's single window and every protection attached to it
//! (specs/008-tauri-shell/research.md, R2, R5, R6, R14).
//!
//! This module is part of the security core (constitution, Principle IX): every change to it
//! gets line-by-line review.

use tauri::webview::{DownloadEvent, NewWindowResponse, PermissionResponse};
use tauri::{Url, WebviewUrl, WebviewWindowBuilder};

/// The label of the app's only window.
pub const MAIN: &str = "main";

/// The origins the app's own page is served from: Tauri's protocol on macOS and Linux, and its
/// `http` form on Windows.
const APP_ORIGINS: &[&str] = &["tauri://localhost", "http://tauri.localhost"];

/// The paths of the app's own page. Every other file of the bundle is a resource, never a page.
/// The window first loads `tauri://localhost`, whose path is empty: `tauri` is not a scheme the
/// URL standard knows, so no `/` is implied.
const PAGE_PATHS: &[&str] = &["", "/", "/index.html"];

/// Replaces the engine's own dialogs before any page script runs, in every frame (FR-021). They
/// would draw outside the host frame, which Principles V and VII reserve to the host's dialogs.
/// Each returns at once with what a dismissed dialog returns.
const DIALOGS_SCRIPT: &str = r#"(() => {
  const replace = (name, value) =>
    Object.defineProperty(window, name, {
      value: () => value,
      writable: false,
      configurable: false,
    });
  replace("alert", undefined);
  replace("confirm", false);
  replace("prompt", null);
  replace("print", undefined);
})();"#;

/// Whether the window may load `url`: the app's own page, or the development server's page in a
/// development build. Everything else is refused: other origins, other files of the bundle,
/// `file:`, `data:`, `blob:`, `javascript:` and any other scheme (FR-009, FR-012).
fn may_load(url: &Url, dev_url: Option<&Url>) -> bool {
    if !PAGE_PATHS.contains(&url.path()) {
        return false;
    }
    // Compared by parts: `Url::origin` is opaque for a scheme it does not know, such as `tauri`
    let origin = format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default());
    if url.port().is_none() && APP_ORIGINS.contains(&origin.as_str()) {
        return true;
    }
    tauri::is_dev() && dev_url.is_some_and(|dev| dev.origin() == url.origin())
}

/// Builds the `main` window with its protections.
pub fn build(app: &tauri::App) -> tauri::Result<()> {
    let dev_url = app.config().build.dev_url.clone();
    let window = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("dashboard")
        .inner_size(800.0, 600.0)
        .on_navigation(move |url| {
            let allowed = may_load(url, dev_url.as_ref());
            if !allowed {
                #[cfg(feature = "shell-probe")]
                crate::probe::refused(crate::probe::Refusal::Navigation);
            }
            allowed
        })
        .on_new_window(|_url, _features| {
            #[cfg(feature = "shell-probe")]
            crate::probe::refused(crate::probe::Refusal::NewWindow);
            NewWindowResponse::Deny
        })
        // Every web permission (camera, microphone, location, notifications…) is denied (FR-021)
        .on_permission_request(|_webview, _kind| {
            #[cfg(feature = "shell-probe")]
            crate::probe::refused(crate::probe::Refusal::Permission);
            PermissionResponse::Deny
        })
        // No download is ever saved (FR-021)
        .on_download(|_webview, event| {
            if matches!(event, DownloadEvent::Requested { .. }) {
                #[cfg(feature = "shell-probe")]
                crate::probe::refused(crate::probe::Refusal::Download);
            }
            false
        })
        .initialization_script_for_all_frames(DIALOGS_SCRIPT)
        // No form data, password or cookie kept between runs, on any engine; the host keeps its
        // state in its own files (FR-016, HS-26)
        .incognito(true)
        .general_autofill_enabled(false)
        // Developer tools only in a development build (FR-011). Release builds also lack the
        // `devtools` feature, which the surface test checks.
        .devtools(tauri::is_dev())
        .build()?;

    // WebView2 settings that Tauri does not expose (research R6, R7):
    // - password saving, always off (FR-016, HS-26): `general_autofill_enabled` covers forms,
    //   not passwords;
    // - the engine's own context menu, off in release (FR-011). The macOS and Linux engines
    //   have no such setting; main.tsx cancels `contextmenu` there.
    #[cfg(windows)]
    window.with_webview(|webview| {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings4;
        use windows_core::Interface;

        // SAFETY: the controller is Tauri's own, alive on this (main) thread
        let result = unsafe {
            webview
                .controller()
                .CoreWebView2()
                .and_then(|core| core.Settings())
                .and_then(|settings| {
                    settings
                        .cast::<ICoreWebView2Settings4>()?
                        .SetIsPasswordAutosaveEnabled(false)?;
                    if !tauri::is_dev() {
                        settings.SetAreDefaultContextMenusEnabled(false)?;
                    }
                    Ok(())
                })
        };
        if let Err(error) = result {
            eprintln!("window: could not apply the WebView2 settings: {error}");
        }
    })?;
    #[cfg(not(windows))]
    let _ = window;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).expect("url")
    }

    #[test]
    fn only_the_apps_own_page_may_load() {
        for allowed in [
            "tauri://localhost",
            "tauri://localhost/",
            "tauri://localhost/index.html",
            "http://tauri.localhost/",
            "http://tauri.localhost/index.html?x=1#top",
        ] {
            assert!(may_load(&url(allowed), None), "{allowed}");
        }
        for refused in [
            "https://probe.invalid/",
            "tauri://localhost/vite.svg",
            "tauri://localhost/assets/index.js",
            "https://tauri.localhost/",
            "http://tauri.localhost.invalid/",
            "http://tauri.localhost:8080/",
            "tauri://localhost.invalid/",
            "file:///etc/hostname",
            "data:text/html,hi",
            "javascript:alert(1)",
            "blob:tauri://localhost/1234",
            "about:blank",
        ] {
            assert!(!may_load(&url(refused), None), "{refused}");
        }
    }

    #[test]
    fn the_development_page_may_load_only_in_development() {
        let dev = url("http://localhost:1420");
        // Unit tests build without tauri/custom-protocol, so they are a development build
        assert!(tauri::is_dev());
        assert!(may_load(&url("http://localhost:1420/"), Some(&dev)));
        assert!(!may_load(
            &url("http://localhost:1420/src/main.tsx"),
            Some(&dev)
        ));
        assert!(!may_load(&url("http://localhost:1421/"), Some(&dev)));
        assert!(!may_load(&url("http://localhost:1420/"), None));
    }
}
