#[cfg(feature = "shell-probe")]
mod probe;
mod window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(feature = "shell-probe")]
    probe::gate();

    let builder = tauri::Builder::default();
    #[cfg(feature = "shell-probe")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        probe::shell_probe_report,
        probe::shell_probe_ungranted
    ]);

    builder
        .setup(|app| {
            window::build(app)?;
            #[cfg(feature = "shell-probe")]
            probe::setup(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
