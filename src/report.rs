//! Telling the person when the program cannot go on.
//!
//! The released program has no console (main.rs `windows_subsystem`), so a start that fails or a
//! fault in the middle of an event would only make the window vanish. Both are written to a log
//! file beside the program instead and, unless the run is a check that must not stop for
//! anyone (`--silent`, `--screenshot`), shown in a message box (an alert on macOS).

use crate::lang::{t, tf};
use std::fmt::Display;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The log beside the program (studio.rs `app_root`); a removal deletes it by this name.
pub const LOG_NAME: &str = "error-log.txt";
/// Where the log goes when the program's own folder cannot be written.
const LOG_NAME_ELSEWHERE: &str = "EventStudio-error-log.txt";
/// A log that has grown beyond this starts again.
const LOG_MOST: u64 = 256 * 1024;

/// Seconds since 1970 as `2026-11-19 08:09:10 UTC` (the days become a date by the usual
/// arithmetic on 400-year eras that start on 1 March).
fn stamp(secs: u64) -> String {
    let (days, rest) = ((secs / 86_400) as i64, secs % 86_400);
    let z = days + 719_468;
    let (era, day_of_era) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let march_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * march_month + 2) / 5 + 1;
    let month = if march_month < 10 { march_month + 3 } else { march_month - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC", rest / 3600, rest / 60 % 60, rest % 60)
}

fn append(path: &Path, text: &str) -> std::io::Result<()> {
    let again = std::fs::metadata(path).is_ok_and(|m| m.len() > LOG_MOST);
    let mut file = std::fs::OpenOptions::new().create(true).append(!again).write(again).truncate(again).open(path)?;
    file.write_all(text.as_bytes())
}

/// One entry, as it stands in the log.
fn entry(secs: u64, what: &str) -> String {
    format!("[{}] Event Studio {}\n{}\n\n", stamp(secs), env!("CARGO_PKG_VERSION"), what.trim_end())
}

/// Adds an entry to the log in the first of `folders` that can be written, and says which file that was.
fn log_in(folders: &[(PathBuf, &str)], what: &str) -> Option<PathBuf> {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let text = entry(secs, what);
    folders.iter().map(|(folder, name)| folder.join(name)).find(|file| append(file, &text).is_ok())
}

/// The log beside the program, or in the temp folder where that cannot be written.
fn log(what: &str) -> Option<PathBuf> {
    log_in(&[(crate::studio::app_root(), LOG_NAME), (std::env::temp_dir(), LOG_NAME_ELSEWHERE)], what)
}

/// What the message box says: what happened, the fault itself, and where the log is.
fn message(headline: &str, detail: &str, log: Option<&Path>) -> String {
    let mut text = format!("{headline}\n\n{detail}");
    if let Some(file) = log {
        text.push_str("\n\n");
        text.push_str(&tf!("자세한 내용을 이 파일에 적어 두었습니다:\n{0}", file.display()));
    }
    text
}

/// A message box in front of every window, the floating timer included.
#[cfg(windows)]
fn show(text: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_TOPMOST};
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (text, title) = (wide(text), wide("Event Studio"));
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST) };
}

/// An alert of the system's own, asked of `osascript`; the text goes along as an argument, so
/// nothing in it is read as script.
#[cfg(target_os = "macos")]
fn show(text: &str) {
    let script = ["-e", "on run argv", "-e", "display alert \"Event Studio\" message (item 1 of argv) as critical", "-e", "end run", "--"];
    if std::process::Command::new("/usr/bin/osascript").args(script).arg(text).status().is_err() {
        eprintln!("{text}");
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn show(text: &str) {
    eprintln!("{text}");
}

/// From here on a fault (panic) anywhere in the program is written to the log. One on the main
/// thread ends the program, so it is also shown; one on a helper thread (a sound, the global
/// hotkeys, a shortcut being written) leaves the timer running and is only logged. `quiet`:
/// nothing is shown.
pub fn watch_for_faults(quiet: bool) {
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        before(info);
        let thread = std::thread::current();
        let detail = info.to_string();
        let log = log(&format!("fault on thread '{}': {detail}", thread.name().unwrap_or("unnamed")));
        if thread.name() == Some("main") && !quiet {
            show(&message(t("Event Studio에 문제가 생겨 끝납니다."), &detail, log.as_deref()));
        }
    }));
}

/// The window could not be opened at all (no usable graphics driver, most of the time).
pub fn could_not_start(error: &dyn Display, quiet: bool) {
    let detail = error.to_string();
    eprintln!("Event Studio could not start: {detail}");
    let log = log(&format!("could not start: {detail}"));
    if !quiet {
        let headline = format!(
            "{}\n{}",
            t("Event Studio를 시작하지 못했습니다."),
            t("그래픽 드라이버가 없거나 원격 데스크톱으로 연결한 화면에서는 창을 열지 못할 수 있습니다.")
        );
        show(&message(&headline, &detail, log.as_deref()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_are_calendar_dates() {
        assert_eq!(stamp(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(stamp(951_782_399), "2000-02-28 23:59:59 UTC");
        assert_eq!(stamp(951_782_400), "2000-02-29 00:00:00 UTC");
        assert_eq!(stamp(1_709_208_000), "2024-02-29 12:00:00 UTC");
        assert_eq!(stamp(1_795_075_750), "2026-11-19 08:09:10 UTC");
        assert_eq!(stamp(4_102_444_799), "2099-12-31 23:59:59 UTC");
    }

    #[test]
    fn the_log_grows_entry_by_entry_and_starts_again_when_it_is_large() {
        let dir = std::env::temp_dir().join(format!("EventStudio-report-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(LOG_NAME);
        append(&file, &entry(0, "first\n")).unwrap();
        append(&file, &entry(60, "second")).unwrap();
        let version = env!("CARGO_PKG_VERSION");
        let both = format!("[1970-01-01 00:00:00 UTC] Event Studio {version}\nfirst\n\n[1970-01-01 00:01:00 UTC] Event Studio {version}\nsecond\n\n");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), both);
        std::fs::write(&file, vec![b'x'; LOG_MOST as usize + 1]).unwrap();
        append(&file, "new\n").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "new\n");
        // A folder that cannot be written is passed over for the next one.
        let nowhere = dir.join("no such folder");
        assert_eq!(log_in(&[(nowhere.clone(), LOG_NAME), (dir.clone(), LOG_NAME_ELSEWHERE)], "third"), Some(dir.join(LOG_NAME_ELSEWHERE)));
        assert!(std::fs::read_to_string(dir.join(LOG_NAME_ELSEWHERE)).unwrap().ends_with("third\n\n"));
        assert_eq!(log_in(&[(nowhere, LOG_NAME)], "fourth"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_message_names_the_fault_and_the_log() {
        let file = Path::new("error-log.txt");
        assert_eq!(message("끝납니다.", "panicked at src/x.rs:1:2", Some(file)), "끝납니다.\n\npanicked at src/x.rs:1:2\n\n자세한 내용을 이 파일에 적어 두었습니다:\nerror-log.txt");
        assert_eq!(message("끝납니다.", "why", None), "끝납니다.\n\nwhy");
    }
}
