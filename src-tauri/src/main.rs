// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if app_lib::try_run_cli_from_env() {
        return;
    }

    app_lib::run();
}
