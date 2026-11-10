//! The app's single window and every protection attached to it.
//!
//! This module is part of the security core (constitution, Principle IX): every change to it
//! gets line-by-line review.

use tauri::{WebviewUrl, WebviewWindowBuilder};

/// The label of the app's only window.
pub const MAIN: &str = "main";

/// Builds the `main` window.
pub fn build(app: &tauri::App) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("dashboard")
        .inner_size(800.0, 600.0)
        .build()?;
    Ok(())
}
