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
mod report;
/// The timer, or — for a setup file and for `--uninstall` — the setup and removal windows.
/// A start that fails, and a fault later on, are told to the person (report.rs): the released
/// program has no console to print them to.
fn main() {
    let args = setup::Args::from_env();
    // A check that runs without anyone at the computer must not wait for a message box.
    let quiet = args.flag("--silent") || args.flag("--screenshot");
    report::watch_for_faults(quiet);
    let ended = match setup::start(&args) {
        setup::Start::App => studio::run(),
        setup::Start::Exit(code) => std::process::exit(code),
        setup::Start::Window(job) => setup_ui::run(job, args.value("--screenshot").map(Into::into), args.value("--demo").map(str::to_string)),
    };
    if let Err(e) = ended {
        report::could_not_start(&e, quiet);
        std::process::exit(1);
    }
}
