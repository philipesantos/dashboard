pub mod install;
pub mod name;
#[cfg(feature = "shell-probe")]
mod probe;
#[cfg(feature = "shell-probe")]
mod probe_source;
pub mod widgets;
mod window;

use std::sync::Arc;

use tauri::{Manager, State};

use install::{Approved, Judged, Started};
use widgets::{FrameInfo, Host, Rendered};

/// Expands the shared command list into `COMMANDS` and the invoke handler that registers them,
/// so the list the surface test checks and the commands Tauri registers cannot drift.
macro_rules! commands {
    ($($command:ident),* $(,)?) => {
        /// The app's host commands, as the invoke handler registers them.
        pub const COMMANDS: &[&str] = &[$(stringify!($command)),*];

        fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
            #[cfg(feature = "shell-probe")]
            return tauri::generate_handler![
                $($command,)*
                probe::shell_probe_report,
                probe::shell_probe_ungranted,
                probe::shell_probe_tree
            ];
            #[cfg(not(feature = "shell-probe"))]
            return tauri::generate_handler![$($command),*];
        }
    };
}
include!("commands.rs");

/// What a command answers when the widget host could not start or stopped.
const HOST_STOPPED: &str = "the widget host stopped";

/// Runs `call` on the widget host, on Tauri's blocking pool, never on the main thread (FR-017).
async fn on_host<T: Send + 'static>(
    host: &State<'_, Arc<Host>>,
    call: impl FnOnce(&widgets::Widgets) -> T + Send + 'static,
) -> Result<T, String> {
    let host = Arc::clone(host);
    tauri::async_runtime::spawn_blocking(move || host.widgets().map(call))
        .await
        .map_err(|_| HOST_STOPPED.to_owned())?
}

/// The widgets the dashboard shows, each with its drawn name and icon.
#[tauri::command]
async fn widgets_list(host: State<'_, Arc<Host>>) -> Result<Vec<FrameInfo>, String> {
    on_host(&host, |widgets| widgets.list()).await
}

/// A widget's content, checked, or why there is none.
#[tauri::command]
async fn widget_render(host: State<'_, Arc<Host>>, handle: String) -> Result<Rendered, String> {
    on_host(&host, move |widgets| widgets.render(&handle)).await
}

/// Runs an action a widget declares, from its button or form, and answers its new content.
#[tauri::command]
async fn widget_act(
    host: State<'_, Arc<Host>>,
    handle: String,
    action: String,
    values: serde_json::Value,
) -> Result<Rendered, String> {
    on_host(&host, move |widgets| widgets.act(&handle, &action, &values)).await
}

/// Starts installing the widget of the repository the person typed in the chrome: fetches,
/// compiles and checks its latest release, and answers the installation dialog's model.
#[tauri::command]
async fn install_start(host: State<'_, Arc<Host>>, address: String) -> Result<Started, String> {
    on_host(&host, move |widgets| widgets.install_start(&address)).await
}

/// Asks whether the installed widget `handle` has a newer release, and answers the dialog's model
/// of it, or none.
#[tauri::command]
async fn update_start(host: State<'_, Arc<Host>>, handle: String) -> Result<Started, String> {
    on_host(&host, move |widgets| widgets.update_start(&handle)).await
}

/// The warning and the choice digest for the optionals the person turned on in the dialog.
#[tauri::command]
async fn request_judge(
    host: State<'_, Arc<Host>>,
    request: String,
    optional_on: serde_json::Value,
) -> Result<Judged, String> {
    on_host(&host, move |widgets| {
        widgets.request_judge(&request, &optional_on)
    })
    .await
}

/// The person approved, in the host dialog, what it showed.
#[tauri::command]
async fn request_approve(
    host: State<'_, Arc<Host>>,
    request: String,
    choice: String,
    optional_on: serde_json::Value,
) -> Result<Approved, String> {
    on_host(&host, move |widgets| {
        widgets.request_approve(&request, &choice, &optional_on)
    })
    .await
}

/// The person cancelled the host dialog, or the window closed with it open.
#[tauri::command]
async fn request_cancel(host: State<'_, Arc<Host>>, request: String) -> Result<(), String> {
    on_host(&host, move |widgets| widgets.request_cancel(&request)).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(feature = "shell-probe")]
    probe::gate();

    tauri::Builder::default()
        .invoke_handler(invoke_handler())
        // The window closing, as when the app quits, takes the safe choice of a dialog that is
        // open and stops an installation that compiles (feature 010, FR-023)
        .on_window_event(|window, event| {
            if matches!(
                event,
                tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
            ) && let Some(host) = window.try_state::<Arc<Host>>()
            {
                host.stop_requests();
            }
        })
        .setup(|app| {
            // The probe installs widgets: it runs on a data directory of its own, emptied first,
            // so a run never finds the last one's
            #[cfg(feature = "shell-probe")]
            let data_dir = probe::data_dir();
            #[cfg(not(feature = "shell-probe"))]
            let data_dir = app.path().app_data_dir()?;
            let host = Arc::new(Host::new(data_dir));
            app.manage(Arc::clone(&host));
            window::build(app)?;
            // Compiles the bundled packages while the window loads
            std::thread::spawn(move || {
                if let Err(error) = host.widgets() {
                    eprintln!("{error}");
                }
            });
            #[cfg(feature = "shell-probe")]
            probe::setup(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
