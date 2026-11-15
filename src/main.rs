#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod timer;
mod studio;
mod win;
mod lang;
mod strings;
mod icon_art;
mod pack;
mod shell;
mod setup;
mod setup_ui;
/// The timer, or — for a setup file and for `--uninstall` — the setup and removal windows.
fn main() -> eframe::Result {
    let args = setup::Args::from_env();
    match setup::start(&args) {
        setup::Start::App => studio::run(),
        setup::Start::Exit(code) => std::process::exit(code),
        setup::Start::Window(job) => setup_ui::run(job, args.value("--screenshot").map(Into::into), args.value("--demo").map(str::to_string)),
    }
}
