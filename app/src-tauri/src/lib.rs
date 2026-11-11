pub mod name;
#[cfg(feature = "shell-probe")]
mod probe;
pub mod widgets;
mod window;

use std::sync::Arc;

use tauri::{Manager, State};

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
                probe::shell_probe_ungranted
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(feature = "shell-probe")]
    probe::gate();

    tauri::Builder::default()
        .invoke_handler(invoke_handler())
        .setup(|app| {
            let host = Arc::new(Host::new(app.path().app_data_dir()?));
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
