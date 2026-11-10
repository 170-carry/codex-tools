#[path = "../../src/app_paths.rs"]
mod app_paths;
#[path = "../../src/auth.rs"]
mod auth;
#[path = "../../src/models.rs"]
mod models;
#[path = "../../src/profile_files.rs"]
mod profile_files;
#[path = "../../src/proxy_daemon.rs"]
mod proxy_daemon;
#[path = "../../src/proxy_service.rs"]
mod proxy_service;
#[path = "../../src/state.rs"]
mod state;
#[path = "../../src/store.rs"]
mod store;
#[path = "../../src/store_lock.rs"]
mod store_lock;
#[cfg(windows)]
#[path = "../../src/switch_timing.rs"]
mod switch_timing;
#[path = "../../src/usage.rs"]
mod usage;
#[path = "../../src/utils.rs"]
mod utils;

fn main() {
    if let Err(error) = proxy_daemon::run_cli_from_env() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
