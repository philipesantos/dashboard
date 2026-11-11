#[cfg(feature = "shell-probe")]
mod probe;
mod window;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(feature = "shell-probe")]
    probe::gate();

    tauri::Builder::default()
        .invoke_handler(invoke_handler())
        .setup(|app| {
            window::build(app)?;
            #[cfg(feature = "shell-probe")]
            probe::setup(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
