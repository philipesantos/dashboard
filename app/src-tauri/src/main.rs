// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    // The compile process, before Tauri or anything else of the app starts: the app compiles
    // every installed package by starting itself with this one argument
    // (specs/010-plugin-installation/research.md, R2)
    if runtime::compile::is_compile_invocation() {
        return runtime::compile::child_main();
    }
    dashboard_lib::run();
    ExitCode::SUCCESS
}
