//! The setup file's format: the program itself with a pack of files added behind it.
//!
//! Self-extracting setup programs are commonly built this way: a small program followed by its
//! data, which it finds again in its own file by a signature and checks before use. Here the
//! signature sits at the very end, so a start only has to read the last few bytes to know
//! whether it is a setup file. Layout, numbers little-endian:
//!
//! ```text
//! [program: app_len bytes][file 1][file 2]...[index: JSON][index_len u64][app_len u64][index_check u64][MAGIC]
//! ```
//!
//! The index names each file with its length and check value, and holds the program's check
//! value; every part is verified when it is read, so a damaged download is refused instead of
//! installed.

use crate::lang::{t, tf};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// The last bytes of every setup file.
pub const MAGIC: &[u8; 16] = b"EVSTUDIO-SETUP\x01\n";
/// index_len + app_len + index_check + MAGIC.
const TAIL: u64 = 8 + 8 + 8 + 16;
/// The settings file plus the few images and the sound the program keeps in `assets/`.
pub const MAX_FILES: usize = 16;
/// An image may be 40 MB when it is chosen (studio.rs `load_image`); nothing bigger is carried.
pub const MAX_FILE_BYTES: u64 = 40 * 1024 * 1024;
const MAX_INDEX_BYTES: u64 = 64 * 1024;
const VERSION: u32 = 1;
const COPY_BLOCK: usize = 1 << 16;

/// The settings file's name inside a pack and beside the program.
pub const SETTINGS: &str = "settings.json";

/// FNV-1a, 64 bits: catches damage in transit, nothing more.
#[derive(Clone, Copy)]
pub struct Check(u64);

impl Check {
    pub fn new() -> Self {
        Check(0xcbf2_9ce4_8422_2325)
    }
    pub fn add(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    pub fn value(self) -> u64 {
        self.0
    }
    pub fn of(bytes: &[u8]) -> u64 {
        let mut c = Check::new();
        c.add(bytes);
        c.value()
    }
}

/// One carried file.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub name: String,
    pub len: u64,
    pub check: u64,
}

#[derive(Serialize, Deserialize)]
struct Index {
    version: u32,
    app_check: u64,
    files: Vec<Item>,
}

/// A program file and what it carries behind its own code.
#[derive(Clone, Debug)]
pub struct Pack {
    pub path: PathBuf,
    /// How many bytes at the start of the file are the program.
    pub app_len: u64,
    /// The program's check value; `None` for a plain program, which has nothing to compare with.
    pub app_check: Option<u64>,
    pub files: Vec<Item>,
}

fn damaged(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, tf!("설치 파일이 손상되었습니다 ({0}). 다시 받아 주세요.", what))
}

/// Only the names the program itself writes: the settings file, or one file directly in `assets/`.
pub fn safe_name(name: &str) -> bool {
    let plain = |s: &str| {
        !s.is_empty()
            && s.len() <= 64
            && !s.starts_with('.')
            && !s.ends_with('.')
            && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    };
    name == SETTINGS || name.strip_prefix("assets/").is_some_and(plain)
}

impl Pack {
    /// Reads what `path` carries. `Ok(None)`: an ordinary program without a pack.
    pub fn open(path: &Path) -> io::Result<Option<Pack>> {
        let mut f = File::open(path)?;
        let total = f.metadata()?.len();
        if total < TAIL {
            return Ok(None);
        }
        f.seek(SeekFrom::Start(total - TAIL))?;
        let mut tail = [0u8; TAIL as usize];
        f.read_exact(&mut tail)?;
        if &tail[24..] != MAGIC {
            return Ok(None);
        }
        let number = |at: usize| u64::from_le_bytes(tail[at..at + 8].try_into().unwrap());
        let (index_len, app_len, index_check) = (number(0), number(8), number(16));
        if index_len == 0 || index_len > MAX_INDEX_BYTES || app_len == 0 || app_len + index_len + TAIL > total {
            return Err(damaged(t("크기")));
        }
        f.seek(SeekFrom::Start(total - TAIL - index_len))?;
        let mut raw = vec![0u8; index_len as usize];
        f.read_exact(&mut raw)?;
        if Check::of(&raw) != index_check {
            return Err(damaged(t("목록")));
        }
        let index: Index = serde_json::from_slice(&raw).map_err(|_| damaged(t("목록")))?;
        let carried: u64 = index.files.iter().map(|i| i.len).sum();
        let names_ok = index.files.iter().all(|i| safe_name(&i.name) && i.len <= MAX_FILE_BYTES);
        if index.version != VERSION || index.files.len() > MAX_FILES || !names_ok || app_len + carried + index_len + TAIL != total {
            return Err(damaged(t("목록")));
        }
        Ok(Some(Pack { path: path.to_path_buf(), app_len, app_check: Some(index.app_check), files: index.files }))
    }

    /// The whole of `path` as a program that carries nothing.
    pub fn plain(path: &Path) -> io::Result<Pack> {
        let app_len = std::fs::metadata(path)?.len();
        Ok(Pack { path: path.to_path_buf(), app_len, app_check: None, files: Vec::new() })
    }

    pub fn has(&self, name: &str) -> bool {
        self.files.iter().any(|i| i.name == name)
    }

    /// Writes the program part to `to` and checks it on the way.
    pub fn copy_app(&self, to: &Path) -> io::Result<()> {
        let mut from = File::open(&self.path)?;
        let mut out = File::create(to)?;
        let (mut left, mut check, mut block) = (self.app_len, Check::new(), vec![0u8; COPY_BLOCK]);
        while left > 0 {
            let want = left.min(COPY_BLOCK as u64) as usize;
            from.read_exact(&mut block[..want])?;
            check.add(&block[..want]);
            out.write_all(&block[..want])?;
            left -= want as u64;
        }
        out.flush()?;
        drop(out);
        if self.app_check.is_some_and(|c| c != check.value()) {
            let _ = std::fs::remove_file(to);
            return Err(damaged(t("프로그램")));
        }
        Ok(())
    }

    /// One carried file's contents, checked. `Ok(None)`: not carried.
    pub fn read(&self, name: &str) -> io::Result<Option<Vec<u8>>> {
        let mut at = self.app_len;
        for item in &self.files {
            if item.name == name {
                let mut f = File::open(&self.path)?;
                f.seek(SeekFrom::Start(at))?;
                let mut data = vec![0u8; item.len as usize];
                f.read_exact(&mut data)?;
                if Check::of(&data) != item.check {
                    return Err(damaged(name));
                }
                return Ok(Some(data));
            }
            at += item.len;
        }
        Ok(None)
    }
}

/// Writes a setup file: the program part of `program` (which may itself be a setup file) and
/// `files` behind it. `out` only appears once it is complete.
pub fn write_setup(program: &Path, files: &[(String, Vec<u8>)], out: &Path) -> io::Result<()> {
    let refuse = |why: &str| io::Error::new(io::ErrorKind::InvalidInput, why.to_string());
    if files.len() > MAX_FILES {
        return Err(refuse(t("담을 파일이 너무 많습니다.")));
    }
    for (name, data) in files {
        if !safe_name(name) || files.iter().filter(|(n, _)| n == name).count() != 1 {
            return Err(refuse(t("담을 수 없는 파일 이름입니다.")));
        }
        if data.len() as u64 > MAX_FILE_BYTES {
            return Err(refuse(t("담을 파일이 너무 큽니다 (40MB 이하).")));
        }
    }
    let source = match Pack::open(program)? {
        Some(p) => p,
        None => Pack::plain(program)?,
    };
    let part = out.with_extension("part");
    let result = (|| {
        source.copy_app(&part)?;
        // copy_app has just read the program; reading it again from the copy gives its check value.
        let mut check = Check::new();
        let mut f = File::open(&part)?;
        let mut block = vec![0u8; COPY_BLOCK];
        loop {
            let n = f.read(&mut block)?;
            if n == 0 {
                break;
            }
            check.add(&block[..n]);
        }
        drop(f);
        let index = Index {
            version: VERSION,
            app_check: check.value(),
            files: files.iter().map(|(name, data)| Item { name: name.clone(), len: data.len() as u64, check: Check::of(data) }).collect(),
        };
        let raw = serde_json::to_vec(&index).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let mut f = std::fs::OpenOptions::new().append(true).open(&part)?;
        for (_, data) in files {
            f.write_all(data)?;
        }
        f.write_all(&raw)?;
        f.write_all(&(raw.len() as u64).to_le_bytes())?;
        f.write_all(&source.app_len.to_le_bytes())?;
        f.write_all(&Check::of(&raw).to_le_bytes())?;
        f.write_all(MAGIC)?;
        f.flush()?;
        drop(f);
        std::fs::rename(&part, out)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("EventStudio-pack-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn program(dir: &Path) -> (PathBuf, Vec<u8>) {
        let bytes: Vec<u8> = (0..200_000u32).map(|i| (i * 31 % 251) as u8).collect();
        let path = dir.join("program.exe");
        std::fs::write(&path, &bytes).unwrap();
        (path, bytes)
    }

    fn files() -> Vec<(String, Vec<u8>)> {
        vec![(SETTINGS.to_string(), br#"{"brand":"B"}"#.to_vec()), ("assets/brand-logo.png".to_string(), vec![7u8; 5000])]
    }

    #[test]
    fn setup_file_carries_program_and_files() {
        let dir = scratch("roundtrip");
        let (exe, bytes) = program(&dir);
        assert!(Pack::open(&exe).unwrap().is_none(), "a plain program carries nothing");
        let out = dir.join("한글 설치 파일.exe");
        write_setup(&exe, &files(), &out).unwrap();
        assert!(!out.with_extension("part").exists());
        let pack = Pack::open(&out).unwrap().expect("a setup file");
        assert_eq!(pack.app_len, bytes.len() as u64);
        assert_eq!(pack.files.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), [SETTINGS, "assets/brand-logo.png"]);
        assert_eq!(pack.read(SETTINGS).unwrap().unwrap(), br#"{"brand":"B"}"#);
        assert_eq!(pack.read("assets/brand-logo.png").unwrap().unwrap(), vec![7u8; 5000]);
        assert!(pack.read("assets/other.png").unwrap().is_none());
        let copy = dir.join("copy.exe");
        pack.copy_app(&copy).unwrap();
        assert_eq!(std::fs::read(&copy).unwrap(), bytes, "the installed program is the packed one, byte for byte");
        // A setup file made from a setup file carries the new files only.
        let again = dir.join("again.exe");
        write_setup(&out, &[], &again).unwrap();
        let pack = Pack::open(&again).unwrap().unwrap();
        assert_eq!((pack.app_len, pack.files.len()), (bytes.len() as u64, 0));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn damaged_setup_file_is_refused() {
        let dir = scratch("damage");
        let (exe, bytes) = program(&dir);
        let out = dir.join("setup.exe");
        write_setup(&exe, &files(), &out).unwrap();
        let good = std::fs::read(&out).unwrap();
        let flip = |at: usize| {
            let mut bad = good.clone();
            bad[at] ^= 0x40;
            let path = dir.join("bad.exe");
            std::fs::write(&path, bad).unwrap();
            path
        };
        // In the program: found when it is copied out.
        let bad = flip(1000);
        let pack = Pack::open(&bad).unwrap().unwrap();
        let copy = dir.join("copy.exe");
        assert!(pack.copy_app(&copy).is_err());
        assert!(!copy.exists(), "a damaged program is not left behind");
        // In a carried file: found when it is read.
        let pack = Pack::open(&flip(bytes.len() + 3)).unwrap().unwrap();
        assert!(pack.read(SETTINGS).is_err());
        assert!(pack.read("assets/brand-logo.png").is_ok());
        // In the index or the numbers behind it: found at once.
        assert!(Pack::open(&flip(good.len() - TAIL as usize - 5)).is_err());
        assert!(Pack::open(&flip(good.len() - TAIL as usize + 9)).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_the_programs_own_file_names_are_carried() {
        for good in [SETTINGS, "assets/background.png", "assets/alert.mp3", "assets/partner-logo.png"] {
            assert!(safe_name(good), "{good}");
        }
        for bad in [
            "",
            "settings.json.bak",
            "assets/",
            "assets/../settings.json",
            "assets/a/b.png",
            "assets\\a.png",
            "../x",
            "C:/x",
            "assets/.hidden",
            "assets/한글.png",
            "EventStudio.exe",
        ] {
            assert!(!safe_name(bad), "{bad}");
        }
        let dir = scratch("names");
        let (exe, _) = program(&dir);
        let out = dir.join("setup.exe");
        assert!(write_setup(&exe, &[("../evil.exe".to_string(), vec![1])], &out).is_err());
        assert!(write_setup(&exe, &[(SETTINGS.to_string(), vec![1]), (SETTINGS.to_string(), vec![2])], &out).is_err());
        assert!(!out.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
