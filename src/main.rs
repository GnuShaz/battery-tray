#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod config;
mod model;
mod render;
mod sources;
#[cfg(windows)]
mod app;
#[cfg(windows)]
mod dump;

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--dump-hid") {
        return dump::run(args.get(i + 1).and_then(|s| dump::parse_id(s)));
    }
    app::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("battery-tray работает только на Windows. `cargo test` кладёт превью иконок в target/preview.");
}
