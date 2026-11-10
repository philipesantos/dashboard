//! The app's single window and every protection attached to it
//! (specs/008-tauri-shell/research.md, R2, R5, R6).
//!
//! This module is part of the security core (constitution, Principle IX): every change to it
//! gets line-by-line review.

use tauri::webview::NewWindowResponse;
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
        // Developer tools only in a development build (FR-011). Release builds also lack the
        // `devtools` feature, which the surface test checks.
        .devtools(tauri::is_dev())
        .build()?;

    // The engine's own context menu, off in release (FR-011). WebKit engines have no such
    // setting; main.tsx cancels `contextmenu` there.
    #[cfg(windows)]
    if !tauri::is_dev() {
        window.with_webview(|webview| {
            // SAFETY: the controller is Tauri's own, alive on this (main) thread
            let result = unsafe {
                webview
                    .controller()
                    .CoreWebView2()
                    .and_then(|core| core.Settings())
                    .and_then(|settings| settings.SetAreDefaultContextMenusEnabled(false))
            };
            if let Err(error) = result {
                eprintln!("window: could not turn off WebView2's context menu: {error}");
            }
        })?;
    }
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
        assert!(!may_load(&url("http://localhost:1420/src/main.tsx"), Some(&dev)));
        assert!(!may_load(&url("http://localhost:1421/"), Some(&dev)));
        assert!(!may_load(&url("http://localhost:1420/"), None));
    }
}
