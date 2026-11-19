//! Installing Event Studio for the signed-in person, and removing it again.
//!
//! The steps are those of an ordinary Windows setup program: copy the program, make a
//! Start-menu folder with the program and its removal in it, a desktop shortcut, and an entry
//! in Windows' list of installed apps; removal deletes exactly those things by name and a
//! folder only once it is empty. Two things differ from the usual on purpose. Setup programs
//! mostly install to Program Files as an administrator; this program keeps its settings and
//! images beside itself, so it installs for one person, where it may write, and asks for no
//! rights. And the setup file can carry the settings and images of the copy that made it.

use crate::lang::{self, t, tf, Lang};
use crate::pack::{self, Pack};
use crate::shell::{self, Folder, Link, Value};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

pub const APP_NAME: &str = "Event Studio";
pub const EXE_NAME: &str = "EventStudio.exe";
/// The name a setup file is offered under, with the version in it as on the download page.
pub const SETUP_NAME: &str = concat!("EventStudio-", env!("CARGO_PKG_VERSION"), "-Setup.exe");
/// The guide in each language, in `Lang::ALL` order: its file name and its text. All four are
/// installed, so the right one is there whichever language the program is switched to later.
const GUIDES: [(&str, &[u8]); 4] = [
    ("사용안내.txt", include_bytes!("../사용안내.txt")),
    ("Guide.txt", include_bytes!("../Guide.txt")),
    ("使用说明.txt", include_bytes!("../使用说明.txt")),
    ("使い方.txt", include_bytes!("../使い方.txt")),
];
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The licences of the libraries and fonts the program is built with, in full: they go wherever
/// the program goes (the install folder; the editor writes them out on request).
pub const LICENSES_NAME: &str = "THIRD-PARTY-LICENSES.txt";
pub const LICENSES: &[u8] = include_bytes!("../THIRD-PARTY-LICENSES.txt");

/// Where Windows keeps its list of installed apps, for the signed-in person.
const ENTRY_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\EventStudio";
const LINK_APP: &str = "Event Studio.lnk";
/// The Start-menu shortcuts to the guide and to the removal, named in the language of the
/// setup, in `Lang::ALL` order. A removal looks for all of them.
const LINK_GUIDES: [&str; 4] = ["사용안내.lnk", "Guide.lnk", "使用说明.lnk", "使い方.lnk"];
const LINK_REMOVES: [&str; 4] = ["Event Studio 제거.lnk", "Uninstall Event Studio.lnk", "卸载 Event Studio.lnk", "Event Studio のアンインストール.lnk"];

/// A program copied in but not yet put in place.
const FRESH_NAME: &str = "EventStudio.exe.new";
/// Settings and images an install replaced on request are kept here.
pub const PREVIOUS_DIR: &str = "previous-settings";
/// Everything the program itself writes beside its file (studio.rs `save`, `pick_image`, the sound picker).
const DATA_FILES: [&str; 9] = [
    "settings.json",
    "settings.backup.json",
    "settings.pending.json",
    crate::studio::UNREADABLE_NAME,
    "assets/background.png",
    "assets/brand-logo.png",
    "assets/partner-logo.png",
    "assets/alert.wav",
    "assets/alert.mp3",
];
/// The settings' fields that name a file in `assets/`.
const ASSET_FIELDS: [&str; 4] = ["background", "logo", "logo2", "sound_file"];
/// The longest folder name that still leaves room for `\assets\partner-logo.png` in a 260-character path.
const MAX_DIR_CHARS: usize = 200;
/// Written into the install folder: only a folder that has it is ever removed from, so a copy
/// used without installing (the program and its settings in any folder) cannot be "removed" by mistake.
const MARKER_NAME: &str = "install-info.txt";
const MARKER: &str = "Event Studio 설치 폴더입니다. 설치 프로그램이 만든 표시 파일이니 지우지 마세요.\r\n제거: Windows 설정 > 앱 > 설치된 앱 > Event Studio\r\n";
/// How long a removal without a window waits for the program that asked for it to end.
pub const SILENT_PATIENCE: Duration = Duration::from_secs(5);
/// In the removal window the asking program ended long before the person pressed the button.
pub const WINDOW_PATIENCE: Duration = Duration::from_millis(800);
/// Windows' "another program is using this file" and "access denied".
const SHARING_VIOLATION: i32 = 32;
const ACCESS_DENIED: i32 = 5;
/// A file just written, or just let go by a program that ended, can stay busy for a moment
/// (virus scanners open new programs): such steps get this many tries, this far apart.
const TRIES: u32 = 10;
const TRY_PAUSE: Duration = Duration::from_millis(100);

pub const RUNNING: &str = "Event Studio가 실행 중입니다. 타이머를 끝낸 뒤 다시 눌러 주세요.\n(투명 타이머는 마우스를 올리면 나오는 X 버튼으로 끝냅니다)";

/// Where the app-list entry is kept.
#[derive(Clone, Debug)]
pub enum Entry {
    Registry,
    /// For checks: the same names and values in a file.
    File(PathBuf),
}

/// Where an install puts things.
#[derive(Clone, Debug)]
pub struct Places {
    /// The install folder's usual parent.
    pub programs: PathBuf,
    pub start_menu: PathBuf,
    pub desktop: PathBuf,
    pub entry: Entry,
    /// Set when everything is kept inside one folder for a check (`--sandbox`).
    pub sandbox: Option<PathBuf>,
}

impl Places {
    pub fn real() -> Result<Places, String> {
        let find = |which, what: &str| shell::folder(which).ok_or_else(|| tf!("{0} 위치를 찾지 못했습니다.", what));
        Ok(Places {
            programs: find(Folder::Programs, t("프로그램 폴더"))?,
            start_menu: find(Folder::StartMenu, t("시작 메뉴"))?,
            desktop: find(Folder::Desktop, t("바탕 화면"))?,
            entry: Entry::Registry,
            sandbox: None,
        })
    }

    /// A whole install inside `dir`: nothing outside it is touched, the registry included.
    pub fn sandbox(dir: &Path) -> Places {
        Places {
            programs: dir.join("Programs"),
            start_menu: dir.join("StartMenu"),
            desktop: dir.join("Desktop"),
            entry: Entry::File(dir.join("app-list-entry.json")),
            sandbox: Some(dir.to_path_buf()),
        }
    }

    /// Where an earlier install is (looked up in its app-list entry), else the usual place.
    pub fn default_dir(&self) -> PathBuf {
        self.entry_text("InstallLocation")
            .map(PathBuf::from)
            .filter(|d| d.is_absolute() && d.join(EXE_NAME).is_file())
            .unwrap_or_else(|| self.programs.join(APP_NAME))
    }

    fn menu_dir(&self) -> PathBuf {
        self.start_menu.join(APP_NAME)
    }

    fn entry_text(&self, name: &str) -> Option<String> {
        match &self.entry {
            Entry::Registry => shell::entry_text(ENTRY_KEY, name),
            Entry::File(path) => {
                let all: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
                all.get(name)?.as_str().map(str::to_string)
            }
        }
    }

    fn entry_write(&self, values: &[(&str, Value)]) -> Result<(), String> {
        match &self.entry {
            Entry::Registry => shell::entry_write(ENTRY_KEY, values),
            Entry::File(path) => {
                let all: serde_json::Map<String, serde_json::Value> = values
                    .iter()
                    .map(|(name, value)| {
                        let value = match value {
                            Value::Text(t) => serde_json::Value::from(t.as_str()),
                            Value::Number(n) => serde_json::Value::from(*n),
                        };
                        (name.to_string(), value)
                    })
                    .collect();
                let text = serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?;
                std::fs::write(path, text).map_err(|e| e.to_string())
            }
        }
    }

    fn entry_delete(&self) -> Result<(), String> {
        match &self.entry {
            Entry::Registry => shell::entry_delete(ENTRY_KEY),
            Entry::File(path) => match std::fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            },
        }
    }
}

/// The person's choices for an install.
#[derive(Clone, Debug)]
pub struct Options {
    pub dir: PathBuf,
    pub desktop: bool,
    pub start_menu: bool,
    /// Use the settings the setup file carries even where settings are already saved.
    pub replace_settings: bool,
    /// The language of the setup: names the Start-menu shortcuts, and is what the program opens
    /// in when it gets no settings from the setup file or from an earlier install.
    pub language: Lang,
}

/// What became of the settings a setup file carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Carried {
    /// The setup file carries none.
    Nothing,
    Written,
    /// Settings were already there and stayed.
    KeptExisting,
    /// Settings were already there; they are now in `PREVIOUS_DIR`.
    Replaced,
}

#[derive(Clone, Debug)]
pub struct Installed {
    pub dir: PathBuf,
    pub exe: PathBuf,
    pub settings: Carried,
}

#[derive(Clone, Debug)]
pub struct Removed {
    pub dir: PathBuf,
    /// Settings and images are still in `dir`.
    pub data_kept: bool,
    /// What could not be deleted.
    pub left: Vec<PathBuf>,
}

/// What is already in a folder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Found {
    pub program: bool,
    pub settings: bool,
}

pub fn look(dir: &Path) -> Found {
    Found { program: dir.join(EXE_NAME).is_file(), settings: dir.join(pack::SETTINGS).is_file() }
}

/// A folder picked in a dialog gets the program's own folder inside it, as the folder pages of
/// setup programs do, so the files never land loose in, say, Documents.
pub fn with_app_folder(picked: &Path) -> PathBuf {
    let already = picked.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(APP_NAME));
    if already {
        picked.to_path_buf()
    } else {
        picked.join(APP_NAME)
    }
}

fn usable_dir(dir: &Path) -> Result<PathBuf, String> {
    let odd = dir.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir));
    if !dir.is_absolute() || dir.parent().is_none() || odd {
        return Err(t("설치 폴더는 드라이브 안의 폴더여야 합니다 (예: C:\\Apps\\Event Studio).").into());
    }
    if dir.to_string_lossy().chars().count() > MAX_DIR_CHARS {
        return Err(t("설치 폴더 이름이 너무 깁니다. 더 짧은 위치를 골라 주세요.").into());
    }
    // Rebuilt from its parts, so the folder is written the same way everywhere (forward slashes become backslashes).
    Ok(dir.components().collect())
}

/// Whether another program holds `file` open in a way that stops it being replaced: a running
/// program's own file, most of all.
fn in_use(file: &Path) -> bool {
    match std::fs::OpenOptions::new().write(true).open(file) {
        Err(e) => e.raw_os_error() == Some(SHARING_VIOLATION),
        Ok(_) => false,
    }
}

fn patiently<T>(mut step: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    let mut tried = 1;
    loop {
        match step() {
            Err(e) if tried < TRIES && matches!(e.raw_os_error(), Some(SHARING_VIOLATION | ACCESS_DENIED)) => {
                tried += 1;
                std::thread::sleep(TRY_PAUSE);
            }
            outcome => return outcome,
        }
    }
}

/// Deletes a file; one that cannot be deleted is noted in `left`.
fn delete(file: PathBuf, left: &mut Vec<PathBuf>) {
    if let Err(e) = patiently(|| std::fs::remove_file(&file)) {
        if e.kind() != std::io::ErrorKind::NotFound {
            left.push(file);
        }
    }
}

/// (Both are compared with their parts written out in full: a folder given by its short name,
/// `C:\PROGRA~1`, is the folder a shortcut names in full.)
fn same_path(a: &Path, b: &Path) -> bool {
    let tidy = |p: &Path| shell::long_path(p).to_string_lossy().trim_end_matches(['\\', '/']).replace('/', "\\").to_lowercase();
    tidy(a) == tidy(b)
}

fn inside(file: &Path, dir: &Path) -> bool {
    let tidy = |p: &Path| shell::long_path(p).to_string_lossy().replace('/', "\\").to_lowercase();
    tidy(file).starts_with(&format!("{}\\", tidy(dir).trim_end_matches('\\')))
}

fn problem(doing: &str, at: &Path, e: std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::InvalidData => e.to_string(),
        std::io::ErrorKind::PermissionDenied => tf!("이 폴더에는 쓸 수 없습니다. 다른 폴더를 골라 주세요.\n{0}", at.display()),
        _ => tf!("{0} 중 문제가 생겼습니다: {1}\n{2}", doing, e, at.display()),
    }
}

/// The values of the app-list entry: DisplayName, UninstallString, DisplayIcon and
/// DisplayVersion, plus what Windows asks of an entry today: the folder, the size and the two
/// "no" flags. No publisher and no web address are written.
fn entry_values(dir: &Path, exe: &Path, places: &Places) -> Vec<(&'static str, Value)> {
    let kilobytes = std::fs::metadata(exe).map(|m| m.len() / 1024).unwrap_or(0).min(u64::from(u32::MAX)) as u32;
    vec![
        ("DisplayName", Value::Text(APP_NAME.into())),
        ("DisplayVersion", Value::Text(VERSION.into())),
        ("DisplayIcon", Value::Text(exe.display().to_string())),
        ("UninstallString", Value::Text(format!("\"{}\" {}", exe.display(), remove_arguments(places)))),
        ("InstallLocation", Value::Text(dir.display().to_string())),
        ("EstimatedSize", Value::Number(kilobytes)),
        ("NoModify", Value::Number(1)),
        ("NoRepair", Value::Number(1)),
    ]
}

/// What the installed program is started with to remove itself.
fn remove_arguments(places: &Places) -> String {
    match &places.sandbox {
        Some(dir) => format!("--uninstall --sandbox \"{}\"", dir.display()),
        None => "--uninstall".to_string(),
    }
}

/// Puts the carried settings and images in place. Settings that are already there stay unless
/// `replace` is set, and then they are kept in `PREVIOUS_DIR` first.
fn unpack_settings(pack: &Pack, dir: &Path, replace: bool) -> Result<Carried, String> {
    if !pack.has(pack::SETTINGS) {
        return Ok(Carried::Nothing);
    }
    let existing = dir.join(pack::SETTINGS).is_file();
    if existing && !replace {
        return Ok(Carried::KeptExisting);
    }
    if existing {
        let keep = dir.join(PREVIOUS_DIR);
        for name in DATA_FILES {
            let from = dir.join(name);
            if from.is_file() {
                let to = keep.join(name);
                std::fs::create_dir_all(to.parent().unwrap()).map_err(|e| problem(t("이전 설정 보관"), &keep, e))?;
                std::fs::copy(&from, &to).map_err(|e| problem(t("이전 설정 보관"), &to, e))?;
            }
        }
    }
    // The settings file last: if a step fails, no settings are left that name a missing image.
    let (settings, others): (Vec<_>, Vec<_>) = pack.files.iter().partition(|i| i.name == pack::SETTINGS);
    for item in others.into_iter().chain(settings) {
        let to = dir.join(&item.name);
        let data = pack.read(&item.name).map_err(|e| problem(t("설정 복사"), &to, e))?.unwrap_or_default();
        std::fs::create_dir_all(to.parent().unwrap()).map_err(|e| problem(t("설정 복사"), &to, e))?;
        std::fs::write(&to, data).map_err(|e| problem(t("설정 복사"), &to, e))?;
    }
    Ok(if existing { Carried::Replaced } else { Carried::Written })
}

/// Installs the program `pack` holds, with what it carries.
pub fn install(pack: &Pack, places: &Places, o: &Options) -> Result<Installed, String> {
    let dir = usable_dir(&o.dir)?;
    let exe = dir.join(EXE_NAME);
    if in_use(&exe) {
        return Err(t(RUNNING).into());
    }
    std::fs::create_dir_all(&dir).map_err(|e| problem(t("폴더 만들기"), &dir, e))?;
    // Installing from the installed program itself (`--install` there) has nothing to copy.
    if !same_path(&pack.path, &exe) {
        let fresh = dir.join(FRESH_NAME);
        pack.copy_app(&fresh).map_err(|e| problem(t("프로그램 복사"), &dir, e))?;
        patiently(|| std::fs::rename(&fresh, &exe)).map_err(|e| problem(t("프로그램 복사"), &exe, e))?;
    }
    for (name, text) in GUIDES {
        let guide = dir.join(name);
        std::fs::write(&guide, text).map_err(|e| problem(t("사용안내 복사"), &guide, e))?;
    }
    let guide = dir.join(GUIDES[o.language as usize].0);
    let licenses = dir.join(LICENSES_NAME);
    std::fs::write(&licenses, LICENSES).map_err(|e| problem(t("사용안내 복사"), &licenses, e))?;
    std::fs::write(dir.join(MARKER_NAME), t(MARKER)).map_err(|e| problem(t("설치 표시"), &dir, e))?;
    let settings = unpack_settings(pack, &dir, o.replace_settings)?;
    // With no settings from the setup file or from an earlier install, the program would open in
    // Windows' language; a first settings file makes it open in the language of the setup.
    let saved = dir.join(pack::SETTINGS);
    if !saved.is_file() {
        let first = serde_json::json!({ "language": o.language.code(), "event": lang::tr(o.language, crate::studio::FIRST_EVENT) });
        std::fs::write(&saved, first.to_string()).map_err(|e| problem(t("설정 복사"), &saved, e))?;
    }

    let link = |at: PathBuf, target: &Path, arguments: &str, note: &str| {
        std::fs::create_dir_all(at.parent().unwrap()).map_err(|e| problem(t("바로 가기 만들기"), &at, e))?;
        shell::write_link(&at, &Link { target, arguments, start_in: &dir, note })
    };
    if o.start_menu {
        // The Start-menu folder holds the program, its guide and its removal.
        let menu = places.menu_dir();
        // An earlier install in another language left its guide and removal under other names.
        let mut left = Vec::new();
        for old in LINK_GUIDES.into_iter().chain(LINK_REMOVES).map(|name| menu.join(name)) {
            if old.is_file() && shell::link_target(&old).is_none_or(|target| inside(&target, &dir)) {
                delete(old, &mut left);
            }
        }
        link(menu.join(LINK_APP), &exe, "", t("브랜드 행사 타이머"))?;
        link(menu.join(LINK_GUIDES[o.language as usize]), &guide, "", t("Event Studio 사용안내"))?;
        link(menu.join(LINK_REMOVES[o.language as usize]), &exe, &remove_arguments(places), t("Event Studio 제거"))?;
    }
    if o.desktop {
        link(places.desktop.join(LINK_APP), &exe, "", t("브랜드 행사 타이머"))?;
    }
    places.entry_write(&entry_values(&dir, &exe, places))?;
    Ok(Installed { dir, exe, settings })
}

/// Removes what `install` put in `dir`: its own files and shortcuts by name, never a folder
/// that still has something in it. Settings and images stay unless `remove_data`. `patience`
/// is how long to wait for a program that is still running from `dir` to end.
pub fn uninstall(dir: &Path, places: &Places, remove_data: bool, patience: Duration) -> Result<Removed, String> {
    let dir = usable_dir(dir)?;
    let exe = dir.join(EXE_NAME);
    let marker = dir.join(MARKER_NAME);
    if !marker.is_file() {
        return Err(tf!("이 폴더에는 설치 프로그램으로 설치한 Event Studio가 없습니다.\n{0}\n(설치 없이 쓰는 프로그램은 폴더를 직접 지우면 됩니다)", dir.display()));
    }
    // The installed program that asked for this needs a moment to end.
    let asked = Instant::now();
    while in_use(&exe) {
        if asked.elapsed() >= patience {
            return Err(t(RUNNING).into());
        }
        std::thread::sleep(TRY_PAUSE);
    }
    let mut left = Vec::new();
    // A shortcut of the same name that leads to another folder belongs to another install.
    let menu = places.menu_dir();
    let in_menu = LINK_GUIDES.into_iter().chain(LINK_REMOVES).chain([LINK_APP]).map(|name| menu.join(name));
    for at in in_menu.chain([places.desktop.join(LINK_APP)]) {
        if at.is_file() && shell::link_target(&at).is_none_or(|target| inside(&target, &dir)) {
            delete(at, &mut left);
        }
    }
    let _ = std::fs::remove_dir(&menu);
    if places.entry_text("InstallLocation").is_none_or(|at| same_path(Path::new(&at), &dir)) {
        places.entry_delete()?;
    }
    // (The log of faults is the program's own by-product, not something the person set up: it goes with the program.)
    for name in GUIDES.into_iter().map(|(name, _)| name).chain([FRESH_NAME, EXE_NAME, LICENSES_NAME, crate::report::LOG_NAME]) {
        delete(dir.join(name), &mut left);
    }
    if remove_data {
        for base in [dir.join(PREVIOUS_DIR), dir.clone()] {
            for name in DATA_FILES {
                delete(base.join(name), &mut left);
            }
            let _ = std::fs::remove_dir(base.join("assets"));
        }
        let _ = std::fs::remove_dir(dir.join(PREVIOUS_DIR));
    }
    // The mark goes last, and only when the rest went: what is left can then be removed again.
    if left.is_empty() {
        delete(marker, &mut left);
    }
    let _ = std::fs::remove_dir(&dir);
    Ok(Removed { data_kept: dir.join(pack::SETTINGS).is_file(), dir, left })
}

fn retired_dir() -> PathBuf {
    std::env::temp_dir().join("EventStudioUninstall")
}

fn is_retired(name: &str) -> bool {
    name.starts_with("EventStudio-uninstall-") && name.ends_with(".exe")
}

/// `--uninstall` reaches the installed program, which cannot delete its own running file. As
/// uninstallers commonly do, it copies itself to the temp folder and the copy does the
/// removing. `pass_on` are the other options given.
pub fn hand_over(pass_on: &[String]) -> Result<(), String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = me.parent().ok_or_else(|| t("설치 폴더를 찾지 못했습니다.").to_string())?;
    let temp = retired_dir();
    let copy = temp.join(format!("EventStudio-uninstall-{}.exe", std::process::id()));
    std::fs::create_dir_all(&temp).and_then(|_| std::fs::copy(&me, &copy)).map_err(|e| problem(t("제거 준비"), &copy, e))?;
    std::process::Command::new(&copy)
        .arg("--uninstall-from")
        .arg(dir)
        .args(pass_on)
        .current_dir(&temp)
        .spawn()
        .map(|_| ())
        .map_err(|e| problem(t("제거 시작"), &copy, e))
}

/// Deletes the copies earlier removals worked from. A running program cannot delete its own
/// file (marking it for deletion at the next restart is something only an administrator may
/// do), so the next start of any Event Studio clears them.
pub fn sweep() {
    let temp = retired_dir();
    if let Ok(list) = std::fs::read_dir(&temp) {
        for file in list.flatten() {
            if is_retired(&file.file_name().to_string_lossy()) {
                let _ = std::fs::remove_file(file.path());
            }
        }
        let _ = std::fs::remove_dir(&temp);
    }
}

/// What a setup file carries for these settings: the settings text itself and the images and
/// the sound it names, read from `root`.
pub fn carried_files(settings: &str, root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = vec![(pack::SETTINGS.to_string(), settings.as_bytes().to_vec())];
    let fields: serde_json::Value = serde_json::from_str(settings).unwrap_or_default();
    for field in ASSET_FIELDS {
        let Some(name) = fields.get(field).and_then(|v| v.as_str()).map(|n| n.replace('\\', "/")) else { continue };
        if !pack::safe_name(&name) || name == pack::SETTINGS || files.iter().any(|(n, _)| *n == name) {
            continue;
        }
        let path = root.join(&name);
        let fits = std::fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() <= pack::MAX_FILE_BYTES);
        if let Some(data) = fits.then(|| std::fs::read(&path).ok()).flatten() {
            files.push((name, data));
        }
    }
    files
}

/// Writes a setup file from `program`, carrying `settings` (their text and the folder they belong to) if given.
pub fn make_setup(program: &Path, out: &Path, settings: Option<(&str, &Path)>) -> Result<Vec<String>, String> {
    let files = settings.map(|(text, root)| carried_files(text, root)).unwrap_or_default();
    pack::write_setup(program, &files, out).map_err(|e| problem(t("설치 파일 만들기"), out, e))?;
    Ok(files.into_iter().map(|(name, _)| name).collect())
}

/// Writes a setup file from the running program. It cannot be saved over the program itself.
pub fn make_setup_here(out: &Path, settings: Option<(&str, &Path)>) -> Result<Vec<String>, String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    if same_path(&me, out) {
        return Err(t("실행 중인 프로그램과 다른 이름이나 위치로 저장해 주세요.").into());
    }
    make_setup(&me, out, settings)
}

/// Opens the folder a file is in, with the file selected.
pub fn show_in_folder(file: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer.exe").raw_arg(format!("/select,\"{}\"", file.display())).spawn();
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("/usr/bin/open").arg("-R").arg(file).spawn();
    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = file;
}

/// The command line, as far as installing goes.
pub struct Args(Vec<String>);

impl Args {
    pub fn from_env() -> Self {
        Args(std::env::args().skip(1).collect())
    }
    pub fn flag(&self, name: &str) -> bool {
        self.0.iter().any(|a| a == name)
    }
    pub fn value(&self, name: &str) -> Option<&str> {
        self.0.iter().position(|a| a == name).and_then(|i| self.0.get(i + 1)).map(String::as_str)
    }
    /// Everything except `name` itself.
    fn without(&self, name: &str) -> Vec<String> {
        self.0.iter().filter(|a| *a != name).cloned().collect()
    }
}

/// The language saved in the settings of an install folder, if any.
fn saved_language(dir: &Path) -> Option<Lang> {
    let text = std::fs::read_to_string(dir.join(pack::SETTINGS)).ok()?;
    let fields: serde_json::Value = serde_json::from_str(&text).ok()?;
    Lang::from_code(fields.get("language")?.as_str()?)
}

/// A window to show instead of the timer, and the language it starts in.
pub enum Job {
    Install { pack: Pack, places: Places, language: Lang },
    Uninstall { dir: PathBuf, places: Places, language: Lang },
    Problem(String, Lang),
}

pub enum Start {
    /// The timer itself.
    App,
    Exit(i32),
    Window(Job),
}

/// Decides what this start is: the timer, a setup or removal window, or a job done without a
/// window.
///
/// - a file that carries a pack, or `--install`: install (`--silent`, `--dir <folder>`,
///   `--no-desktop`, `--no-start-menu`, `--replace-settings`)
/// - `--uninstall`: remove the install this program belongs to (`--silent`, `--remove-data`)
/// - `--make-setup <file>`: write a setup file with the settings beside this program (`--no-settings`)
/// - `--sandbox <folder>`: keep everything inside that folder (checks); `--log <file>`: write the outcome there
/// - `--write-icon <file.png>`: write the program's icon as a 1024-pixel picture and end
/// - `--lang <ko|en|zh|ja>`: the language of the window and of what is written; without it a
///   setup speaks Windows' language and a removal the language the installed program was
///   last used in
pub fn start(args: &Args) -> Start {
    let Ok(me) = std::env::current_exe() else { return Start::App };
    let silent = args.flag("--silent");
    let asked = args.value("--lang").and_then(Lang::from_code);
    let language = asked.unwrap_or_else(Lang::system);
    lang::set(language);
    let report = |outcome: &Result<String, String>| {
        if let Some(path) = args.value("--log") {
            let text = match outcome {
                Ok(said) => format!("ok\n{said}\n"),
                Err(e) => format!("error\n{e}\n"),
            };
            let _ = std::fs::write(path, text);
        }
        if outcome.is_ok() { 0 } else { 1 }
    };
    let fail = |e: String| if silent { Start::Exit(report(&Err(e))) } else { Start::Window(Job::Problem(e, lang::current())) };
    let places = || match args.value("--sandbox") {
        Some(dir) => Ok(Places::sandbox(Path::new(dir))),
        None => Places::real(),
    };

    // `--write-icon <file.png>`: the program's icon as a picture, for building the macOS app (packaging/macos).
    if let Some(out) = args.value("--write-icon") {
        let side = 1024;
        let saved = image::save_buffer(out, &crate::icon_art::icon_rgba(side), side, side, image::ColorType::Rgba8);
        return Start::Exit(report(&saved.map(|_| out.to_string()).map_err(|e| e.to_string())));
    }
    if let Some(out) = args.value("--make-setup") {
        // The settings the program itself would open (studio.rs `app_root`).
        let root = crate::studio::app_root();
        let text = if args.flag("--no-settings") { None } else { std::fs::read_to_string(root.join(pack::SETTINGS)).ok() };
        let made = make_setup_here(Path::new(out), text.as_deref().map(|text| (text, root.as_path())));
        return Start::Exit(report(&made.map(|names| names.join("\n"))));
    }
    if args.flag("--uninstall") {
        return match hand_over(&args.without("--uninstall")) {
            Ok(()) => Start::Exit(0),
            Err(e) => fail(e),
        };
    }
    if let Some(dir) = args.value("--uninstall-from") {
        let language = asked.or_else(|| saved_language(Path::new(dir))).unwrap_or(language);
        lang::set(language);
        let places = match places() {
            Ok(p) => p,
            Err(e) => return fail(e),
        };
        if silent {
            let done = uninstall(Path::new(dir), &places, args.flag("--remove-data"), SILENT_PATIENCE);
            return Start::Exit(report(&done.map(|r| format!("data_kept={}\nleft={}", r.data_kept, r.left.len()))));
        }
        return Start::Window(Job::Uninstall { dir: PathBuf::from(dir), places, language });
    }
    let pack = match Pack::open(&me) {
        Ok(Some(pack)) => pack,
        Ok(None) if args.flag("--install") => match Pack::plain(&me) {
            Ok(pack) => pack,
            Err(e) => return fail(e.to_string()),
        },
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => return fail(e.to_string()),
        _ => {
            sweep();
            return Start::App;
        }
    };
    sweep();
    let places = match places() {
        Ok(p) => p,
        Err(e) => return fail(e),
    };
    if silent {
        let o = Options {
            dir: args.value("--dir").map(PathBuf::from).unwrap_or_else(|| places.default_dir()),
            desktop: !args.flag("--no-desktop"),
            start_menu: !args.flag("--no-start-menu"),
            replace_settings: args.flag("--replace-settings"),
            language,
        };
        let done = install(&pack, &places, &o);
        return Start::Exit(report(&done.map(|i| format!("{}\nsettings={:?}", i.exe.display(), i.settings))));
    }
    Start::Window(Job::Install { pack, places, language })
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;

    /// A folder in the temp folder holding a stand-in program, a setup file and a sandbox.
    struct Bench {
        root: PathBuf,
        program: Vec<u8>,
        places: Places,
    }

    impl Bench {
        fn new(name: &str) -> Bench {
            // (The temp folder may be given by its short name; shortcuts store the full one.)
            let root = shell::long_path(&std::env::temp_dir()).join(format!("EventStudio-setup-test-{}-{name} 한글 폴더", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("source/assets")).unwrap();
            let program: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 253) as u8).collect();
            std::fs::write(root.join("source/program.exe"), &program).unwrap();
            std::fs::write(root.join("source/assets/brand-logo.png"), [1u8, 2, 3]).unwrap();
            std::fs::write(root.join("source/assets/alert.mp3"), [9u8; 40]).unwrap();
            Bench { places: Places::sandbox(&root.join("box")), root, program }
        }
        fn settings(brand: &str) -> String {
            format!(r#"{{"brand":"{brand}","logo":"assets/brand-logo.png","logo2":"","background":"assets/missing.png","sound_file":"assets\\alert.mp3"}}"#)
        }
        /// A setup file carrying settings with this brand, or none.
        fn setup(&self, brand: Option<&str>) -> Pack {
            let out = self.root.join(format!("setup-{}.exe", brand.unwrap_or("plain")));
            let text = brand.map(Self::settings);
            let source = self.root.join("source");
            make_setup(&source.join("program.exe"), &out, text.as_deref().map(|t| (t, source.as_path()))).unwrap();
            Pack::open(&out).unwrap().unwrap()
        }
        fn options(&self) -> Options {
            Options { dir: self.places.default_dir(), desktop: true, start_menu: true, replace_settings: false, language: Lang::Ko }
        }
        fn dir(&self) -> PathBuf {
            self.root.join("box").join("Programs").join(APP_NAME)
        }
        fn brand(&self) -> String {
            let text = std::fs::read_to_string(self.dir().join("settings.json")).unwrap();
            serde_json::from_str::<serde_json::Value>(&text).unwrap()["brand"].as_str().unwrap().to_string()
        }
        fn entry(&self) -> serde_json::Value {
            serde_json::from_str(&std::fs::read_to_string(self.root.join("box/app-list-entry.json")).unwrap()).unwrap()
        }
    }

    impl Drop for Bench {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn setup_file_carries_the_settings_and_the_files_they_name() {
        let bench = Bench::new("carry");
        let pack = bench.setup(Some("가나다"));
        let names: Vec<&str> = pack.files.iter().map(|i| i.name.as_str()).collect();
        // The missing background is left out; the sound's backslash is tidied.
        assert_eq!(names, ["settings.json", "assets/brand-logo.png", "assets/alert.mp3"]);
        assert!(bench.setup(None).files.is_empty());
    }

    #[test]
    fn install_puts_program_guide_settings_shortcuts_and_entry_in_place() {
        let bench = Bench::new("install");
        let done = install(&bench.setup(Some("첫 브랜드")), &bench.places, &bench.options()).unwrap();
        let dir = bench.dir();
        assert_eq!((done.dir.clone(), done.settings), (dir.clone(), Carried::Written));
        assert_eq!(std::fs::read(dir.join(EXE_NAME)).unwrap(), bench.program, "the program without the pack behind it");
        for (name, text) in GUIDES {
            assert_eq!(std::fs::read(dir.join(name)).unwrap(), text, "{name}");
        }
        assert_eq!(bench.brand(), "첫 브랜드");
        assert_eq!(std::fs::read(dir.join("assets/brand-logo.png")).unwrap(), [1, 2, 3]);
        assert!(dir.join("assets/alert.mp3").is_file() && dir.join(MARKER_NAME).is_file() && !dir.join(FRESH_NAME).exists());
        assert_eq!(std::fs::read(dir.join(LICENSES_NAME)).unwrap(), LICENSES, "the licences go with the program");
        let menu = bench.root.join("box/StartMenu").join(APP_NAME);
        assert_eq!(shell::link_target(&menu.join(LINK_APP)).unwrap(), dir.join(EXE_NAME));
        assert_eq!(shell::link_target(&menu.join(LINK_GUIDES[0])).unwrap(), dir.join(GUIDES[0].0));
        assert_eq!(shell::link_target(&menu.join(LINK_REMOVES[0])).unwrap(), dir.join(EXE_NAME));
        assert_eq!(shell::link_target(&bench.root.join("box/Desktop").join(LINK_APP)).unwrap(), dir.join(EXE_NAME));
        let entry = bench.entry();
        assert_eq!(entry["DisplayName"], APP_NAME);
        assert_eq!(entry["DisplayVersion"], VERSION);
        assert_eq!(entry["InstallLocation"], dir.display().to_string());
        assert_eq!(entry["EstimatedSize"], 292);
        let remove = entry["UninstallString"].as_str().unwrap();
        assert!(remove.starts_with(&format!("\"{}\" --uninstall --sandbox ", dir.join(EXE_NAME).display())), "{remove}");
        // The next setup finds the same folder again.
        assert_eq!(bench.places.default_dir(), dir);
        assert_eq!(look(&dir), Found { program: true, settings: true });
    }

    #[test]
    fn guides_have_the_same_sections_in_every_language() {
        // A heading starts at the left edge; everything under it is indented.
        let headings = |text: &[u8]| std::str::from_utf8(text).unwrap().lines().filter(|l| !l.is_empty() && !l.starts_with(' ')).count();
        let korean = headings(GUIDES[0].1);
        assert!(korean >= 12, "{korean}");
        for (name, text) in GUIDES {
            assert_eq!(headings(text), korean, "{name}");
            assert!(!text.starts_with(&[0xEF, 0xBB, 0xBF]) && !text.contains(&b'\r'), "{name}");
        }
    }

    #[test]
    fn install_follows_the_language_of_the_setup() {
        let bench = Bench::new("language");
        let (dir, menu) = (bench.dir(), bench.root.join("box/StartMenu").join(APP_NAME));
        let japanese = Options { language: Lang::Ja, ..bench.options() };
        assert_eq!(install(&bench.setup(None), &bench.places, &japanese).unwrap().settings, Carried::Nothing);
        // No settings came along, so the program is told which language to open in.
        let first: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap()).unwrap();
        assert_eq!((first["language"].as_str(), first["event"].as_str()), (Some("ja"), Some("新しい可能性をひらく時間")));
        assert_eq!(saved_language(&dir), Some(Lang::Ja));
        assert_eq!(shell::link_target(&menu.join("使い方.lnk")).unwrap(), dir.join("使い方.txt"));
        assert_eq!(shell::link_target(&menu.join("Event Studio のアンインストール.lnk")).unwrap(), dir.join(EXE_NAME));
        assert!(!menu.join(LINK_GUIDES[0]).exists() && !menu.join(LINK_REMOVES[0]).exists());
        // Installed again in English: the shortcuts change their names, the saved language stays.
        let english = Options { language: Lang::En, ..bench.options() };
        install(&bench.setup(None), &bench.places, &english).unwrap();
        let names: std::collections::BTreeSet<String> = std::fs::read_dir(&menu).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names.into_iter().collect::<Vec<_>>(), ["Event Studio.lnk", "Guide.lnk", "Uninstall Event Studio.lnk"]);
        assert_eq!(shell::link_target(&menu.join("Guide.lnk")).unwrap(), dir.join("Guide.txt"));
        assert_eq!(saved_language(&dir), Some(Lang::Ja));
        // Settings that come with the setup file keep their own language.
        std::fs::remove_file(dir.join("settings.json")).unwrap();
        install(&bench.setup(Some("브랜드")), &bench.places, &english).unwrap();
        assert_eq!((bench.brand(), saved_language(&dir)), ("브랜드".to_string(), None));
        // Removal finds the shortcuts and guides of every language.
        uninstall(&dir, &bench.places, true, Duration::ZERO).unwrap();
        assert!(!dir.exists() && !menu.exists());
    }

    #[test]
    fn installing_again_keeps_saved_settings_unless_asked() {
        let bench = Bench::new("again");
        install(&bench.setup(Some("처음")), &bench.places, &bench.options()).unwrap();
        std::fs::write(bench.dir().join("assets/brand-logo.png"), [5u8; 9]).unwrap();
        let second = bench.setup(Some("나중"));
        assert_eq!(install(&second, &bench.places, &bench.options()).unwrap().settings, Carried::KeptExisting);
        assert_eq!(bench.brand(), "처음");
        assert_eq!(std::fs::read(bench.dir().join("assets/brand-logo.png")).unwrap(), [5u8; 9]);
        let replace = Options { replace_settings: true, ..bench.options() };
        assert_eq!(install(&second, &bench.places, &replace).unwrap().settings, Carried::Replaced);
        assert_eq!(bench.brand(), "나중");
        assert_eq!(std::fs::read(bench.dir().join("assets/brand-logo.png")).unwrap(), [1, 2, 3]);
        let kept = bench.dir().join(PREVIOUS_DIR);
        assert!(std::fs::read_to_string(kept.join("settings.json")).unwrap().contains("처음"));
        assert_eq!(std::fs::read(kept.join("assets/brand-logo.png")).unwrap(), [5u8; 9]);
        // A setup file without settings never touches them.
        assert_eq!(install(&bench.setup(None), &bench.places, &replace).unwrap().settings, Carried::Nothing);
        assert_eq!(bench.brand(), "나중");
    }

    #[test]
    fn removal_keeps_settings_unless_asked_and_never_other_files() {
        let bench = Bench::new("remove");
        let pack = bench.setup(Some("브랜드"));
        install(&pack, &bench.places, &bench.options()).unwrap();
        let dir = bench.dir();
        std::fs::write(dir.join("내 메모.txt"), "mine").unwrap();
        let done = uninstall(&dir, &bench.places, false, Duration::ZERO).unwrap();
        assert!(done.data_kept && done.left.is_empty());
        assert!(!dir.join(EXE_NAME).exists() && !dir.join(GUIDES[0].0).exists());
        assert!(dir.join("settings.json").is_file() && dir.join("assets/brand-logo.png").is_file());
        assert!(!bench.root.join("box/StartMenu").join(APP_NAME).exists());
        assert!(!bench.root.join("box/Desktop").join(LINK_APP).exists());
        assert!(!bench.root.join("box/app-list-entry.json").exists());
        assert!(uninstall(&dir, &bench.places, false, Duration::ZERO).is_err(), "nothing installed there any more");
        // Installed again, the saved settings are back in use; removing with them leaves only the person's own file.
        assert_eq!(install(&pack, &bench.places, &bench.options()).unwrap().settings, Carried::KeptExisting);
        let done = uninstall(&dir, &bench.places, true, Duration::ZERO).unwrap();
        assert!(!done.data_kept && done.left.is_empty());
        let rest: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(rest, ["내 메모.txt"]);
        // With nothing of the person's in it, the folder goes too.
        std::fs::remove_file(dir.join("내 메모.txt")).unwrap();
        install(&pack, &bench.places, &Options { replace_settings: true, ..bench.options() }).unwrap();
        uninstall(&dir, &bench.places, true, Duration::ZERO).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn removal_leaves_another_installs_shortcut_and_entry_alone() {
        let bench = Bench::new("two");
        let pack = bench.setup(None);
        let first = bench.root.join("box").join("첫 폴더").join(APP_NAME);
        install(&pack, &bench.places, &Options { dir: first.clone(), ..bench.options() }).unwrap();
        // A second install elsewhere takes over the shortcuts and the entry.
        let second = bench.root.join("box").join("둘째 폴더").join(APP_NAME);
        install(&pack, &bench.places, &Options { dir: second.clone(), ..bench.options() }).unwrap();
        uninstall(&first, &bench.places, true, Duration::ZERO).unwrap();
        assert!(!first.exists());
        let desktop = bench.root.join("box/Desktop").join(LINK_APP);
        assert_eq!(shell::link_target(&desktop).unwrap(), second.join(EXE_NAME));
        assert_eq!(bench.entry()["InstallLocation"], second.display().to_string());
        uninstall(&second, &bench.places, true, Duration::ZERO).unwrap();
        assert!(!desktop.exists() && !bench.root.join("box/app-list-entry.json").exists());
    }

    #[test]
    fn a_copy_used_without_installing_is_never_removed() {
        let bench = Bench::new("portable");
        // The program, its guide and its settings in a folder of their own, as when it is used without installing.
        let dir = bench.root.join("portable");
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        for name in [EXE_NAME, GUIDES[0].0, "settings.json", "assets/brand-logo.png"] {
            std::fs::write(dir.join(name), "x").unwrap();
        }
        assert!(uninstall(&dir, &bench.places, true, Duration::ZERO).is_err());
        for name in [EXE_NAME, GUIDES[0].0, "settings.json", "assets/brand-logo.png"] {
            assert!(dir.join(name).is_file(), "{name}");
        }
    }

    #[test]
    fn a_running_program_is_not_replaced_or_removed() {
        let bench = Bench::new("running");
        let pack = bench.setup(None);
        install(&pack, &bench.places, &bench.options()).unwrap();
        let exe = bench.dir().join(EXE_NAME);
        // Held open the way Windows holds a running program's file: nobody else may write it.
        let held = std::fs::OpenOptions::new().read(true).share_mode(1).open(&exe).unwrap();
        assert_eq!(install(&pack, &bench.places, &bench.options()).unwrap_err(), RUNNING);
        let asked = Instant::now();
        assert_eq!(uninstall(&bench.dir(), &bench.places, false, Duration::from_millis(300)).unwrap_err(), RUNNING);
        assert!(asked.elapsed() >= Duration::from_millis(300) && exe.is_file() && bench.dir().join(GUIDES[0].0).is_file());
        drop(held);
        install(&pack, &bench.places, &bench.options()).unwrap();
    }

    #[test]
    fn install_folder_rules() {
        assert_eq!(usable_dir(Path::new("C:/Apps/Event Studio")).unwrap().to_string_lossy(), r"C:\Apps\Event Studio");
        for bad in [r"C:\", r"Event Studio", r"C:\Apps\..\Windows", &format!(r"C:\{}", "가".repeat(220))] {
            assert!(usable_dir(Path::new(bad)).is_err(), "{bad}");
        }
        assert_eq!(with_app_folder(Path::new(r"D:\Tools")), Path::new(r"D:\Tools\Event Studio"));
        assert_eq!(with_app_folder(Path::new(r"D:\Tools\event studio")), Path::new(r"D:\Tools\event studio"));
        assert!(same_path(Path::new(r"C:\A\b\"), Path::new("c:/a/B")));
        assert!(inside(Path::new(r"C:\A\b\x.exe"), Path::new(r"c:\a\B")) && !inside(Path::new(r"C:\A\bc\x.exe"), Path::new(r"C:\A\b")));
        assert!(is_retired("EventStudio-uninstall-123.exe") && !is_retired("notes.txt"));
    }
}
