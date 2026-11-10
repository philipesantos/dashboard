//! The app's single window and every protection attached to it
//! (specs/008-tauri-shell/research.md, R2, R5, R6, R14).
//!
//! This module is part of the security core (constitution, Principle IX): every change to it
//! gets line-by-line review.

use tauri::webview::{DownloadEvent, NewWindowResponse, PermissionResponse};
use tauri::{Url, WebviewUrl, WebviewWindowBuilder};

/// The label of the app's only window.
pub const MAIN: &str = "main";

/// The origin the app's own page is served from: Tauri's protocol on macOS and Linux, and its
/// `http` form on Windows (`tauri-2.12.5/scripts/core.js:16-18`). Each system accepts only its
/// own: elsewhere the other form is a real request to the machine's port 80, or the system's
/// handler for an unknown scheme.
#[cfg(windows)]
const APP_ORIGIN: &str = "http://tauri.localhost";
#[cfg(not(windows))]
const APP_ORIGIN: &str = "tauri://localhost";

/// The paths of the app's own page. Every other file of the bundle is a resource, never a page.
/// The window first loads `tauri://localhost`, whose path is empty: `tauri` is not a scheme the
/// URL standard knows, so no `/` is implied.
const PAGE_PATHS: &[&str] = &["", "/", "/index.html"];

/// Runs before any page script, in every frame, and takes away what the engine would draw or
/// open for the page, outside the host frame (FR-021; Principles V and VII reserve such UI to the
/// host's dialogs), and what the content policy cannot cover. Each replacement is locked, so
/// later page code cannot put the engine's back. Page code is only the bundle, by the content
/// policy, so this guards against injected markup and mistakes in our own code.
const ENGINE_UI_SCRIPT: &str = r#"(() => {
  const lock = (target, name, value) =>
    Object.defineProperty(target, name, { value, writable: false, configurable: false });

  // The engine's dialogs return at once with what a dismissed dialog returns
  lock(window, "alert", () => undefined);
  lock(window, "confirm", () => false);
  lock(window, "prompt", () => null);
  lock(window, "print", () => undefined);

  // No file picker: no click activates a file input, and no picker API is left
  const click = HTMLInputElement.prototype.click;
  lock(HTMLInputElement.prototype, "click", function () {
    if (this.type !== "file") click.call(this);
  });
  lock(HTMLInputElement.prototype, "showPicker", () => undefined);
  for (const name of ["showOpenFilePicker", "showSaveFilePicker", "showDirectoryPicker"]) {
    if (name in window) lock(window, name, undefined);
  }
  window.addEventListener(
    "click",
    (event) => {
      if (event.target instanceof HTMLInputElement && event.target.type === "file") {
        event.preventDefault();
        event.stopImmediatePropagation();
      }
    },
    true,
  );

  // No WebRTC: its connections are outside the content policy
  for (const name of Object.getOwnPropertyNames(window)) {
    if (/RTCPeerConnection$|^RTCDataChannel$/.test(name)) lock(window, name, undefined);
  }

  // WEBKITGTK-ONLY(window-close): WebKitGTK destroys the webview on window.close()
  lock(window, "close", () => undefined);
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
    if url.port().is_none() && origin == APP_ORIGIN {
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
        .initialization_script_for_all_frames(ENGINE_UI_SCRIPT)
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
    //   have no such setting; main.tsx cancels `contextmenu` there;
    // - the browser's own shortcut keys, off in release (FR-011, FR-021): reload, print, find,
    //   and the like.
    #[cfg(windows)]
    window.with_webview(|webview| {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2Settings3, ICoreWebView2Settings4,
        };
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
                        settings
                            .cast::<ICoreWebView2Settings3>()?
                            .SetAreBrowserAcceleratorKeysEnabled(false)?;
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

    /// The app's origin on this system, and the other system's form, which must be refused.
    #[cfg(windows)]
    const ORIGINS: (&str, &str) = ("http://tauri.localhost", "tauri://localhost");
    #[cfg(not(windows))]
    const ORIGINS: (&str, &str) = ("tauri://localhost", "http://tauri.localhost");

    #[test]
    fn only_the_apps_own_page_may_load() {
        let (own, other) = ORIGINS;
        for allowed in [
            own.to_string(),
            format!("{own}/"),
            format!("{own}/index.html"),
            format!("{own}/index.html?x=1#top"),
        ] {
            assert!(may_load(&url(&allowed), None), "{allowed}");
        }
        for refused in [
            format!("{other}/"),
            format!("{other}/index.html"),
            format!("{own}/vite.svg"),
            format!("{own}/assets/index.js"),
            "https://probe.invalid/".to_string(),
            "https://tauri.localhost/".to_string(),
            "http://tauri.localhost.invalid/".to_string(),
            "http://tauri.localhost:8080/".to_string(),
            "tauri://localhost.invalid/".to_string(),
            "file:///etc/hostname".to_string(),
            "data:text/html,hi".to_string(),
            "javascript:alert(1)".to_string(),
            format!("blob:{own}/1234"),
            "about:blank".to_string(),
        ] {
            assert!(!may_load(&url(&refused), None), "{refused}");
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
