//! Optional headless entry point using the same collector as the desktop page.
#[path = "../git_activity.rs"]
mod git_activity;
use std::{env, fs, path::PathBuf, sync::atomic::AtomicBool};
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: git-calendar <config.json> <cache-directory> <report.json>".into());
    }
    let config = serde_json::from_slice(&fs::read(&args[0]).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let report = git_activity::collect(
        config,
        &PathBuf::from(&args[1]),
        &AtomicBool::new(false),
        |s| eprintln!("{s}"),
    )?;
    fs::write(
        &args[2],
        serde_json::to_vec(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
