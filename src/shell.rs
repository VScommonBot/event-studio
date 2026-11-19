//! What installing needs from Windows: where the Start menu and the desktop are, shortcut
//! files, and the entry in the list of installed apps.
//!
//! Setup programs do three things there: shortcuts through the shell's link object
//! (`CoCreateInstance`, then IShellLinkW's SetPath, SetWorkingDirectory, SetArguments and
//! SetDescription and IPersistFile's Save), the entry's values as REG_SZ text, and the whole
//! key deleted on removal. A setup that runs as an administrator writes to
//! HKEY_LOCAL_MACHINE; this program installs for the signed-in person only, so everything
//! goes to HKEY_CURRENT_USER.

use std::path::Path;

/// A shortcut file's contents.
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Link<'a> {
    pub target: &'a Path,
    pub arguments: &'a str,
    pub start_in: &'a Path,
    pub note: &'a str,
}

/// One value of the app-list entry.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    Number(u32),
}

/// Per-person folders.
#[derive(Clone, Copy, Debug)]
pub enum Folder {
    /// Start menu > Programs.
    StartMenu,
    Desktop,
    /// Programs installed for one person (`%LOCALAPPDATA%\Programs`).
    Programs,
}

#[cfg(windows)]
mod imp {
    use super::{Folder, Link, Value};
    use crate::lang::{t, tf};
    use std::ffi::{c_void, OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};
    use std::ptr::{null, null_mut};
    use windows_sys::core::{GUID, HRESULT, PCWSTR, PWSTR};
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;
    use windows_sys::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows_sys::Win32::System::Registry::{RegDeleteKeyW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD, REG_SZ, RRF_RT_REG_SZ};
    use windows_sys::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_UserProgramFiles, SHGetKnownFolderPath, KF_FLAG_DONT_VERIFY};

    fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
        s.as_ref().encode_wide().chain(Some(0)).collect()
    }

    fn text(buffer: &[u16]) -> OsString {
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        OsString::from_wide(&buffer[..len])
    }

    /// A path with each of its parts written out in full. Windows also knows a folder by a short
    /// name (`C:\Users\RUNNER~1` for `C:\Users\runneradmin`), and a shortcut stores the full one:
    /// two spellings of one folder are compared in this form. A path that is not there comes back
    /// as it was given.
    pub fn long_path(path: &Path) -> PathBuf {
        let given = wide(path);
        let mut full = vec![0u16; 1024];
        loop {
            let len = unsafe { GetLongPathNameW(given.as_ptr(), full.as_mut_ptr(), full.len() as u32) } as usize;
            if len == 0 {
                return path.to_path_buf();
            }
            if len < full.len() {
                return PathBuf::from(text(&full[..len]));
            }
            // Too long for the buffer: `len` is the room it needs.
            full.resize(len + 1, 0);
        }
    }

    pub fn folder(which: Folder) -> Option<PathBuf> {
        let id = match which {
            Folder::StartMenu => &FOLDERID_Programs,
            Folder::Desktop => &FOLDERID_Desktop,
            Folder::Programs => &FOLDERID_UserProgramFiles,
        };
        let mut found: PWSTR = null_mut();
        // DONT_VERIFY: the per-person Programs folder need not exist before the first install.
        let hr = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DONT_VERIFY as _, null_mut(), &mut found) };
        let path = (hr >= 0 && !found.is_null()).then(|| {
            let len = (0..).take_while(|&i| unsafe { *found.add(i) } != 0).count();
            PathBuf::from(OsString::from_wide(unsafe { std::slice::from_raw_parts(found, len) }))
        });
        unsafe { CoTaskMemFree(found as *const c_void) };
        path.filter(|p| p.is_absolute())
    }

    // The shell's link object and the two interfaces used on it.
    const fn shell_id(first: u32) -> GUID {
        GUID { data1: first, data2: 0, data3: 0, data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46] }
    }
    const CLSID_SHELL_LINK: GUID = shell_id(0x0002_1401);
    const IID_SHELL_LINK_W: GUID = shell_id(0x0002_14F9);
    const IID_PERSIST_FILE: GUID = shell_id(0x0000_010B);

    // Places in the method tables, in the order Windows' headers declare the methods (the first
    // three are IUnknown's): IShellLinkW 20 SetPath, 9 SetWorkingDirectory, 11 SetArguments,
    // 7 SetDescription, and IPersistFile 6 Save.
    const QUERY_INTERFACE: usize = 0;
    const RELEASE: usize = 2;
    const LINK_GET_PATH: usize = 3;
    const LINK_SET_DESCRIPTION: usize = 7;
    const LINK_SET_WORKING_DIRECTORY: usize = 9;
    const LINK_SET_ARGUMENTS: usize = 11;
    const LINK_SET_PATH: usize = 20;
    const FILE_LOAD: usize = 5;
    const FILE_SAVE: usize = 6;
    /// IShellLinkW::GetPath: the path as stored, without looking for the file.
    const SLGP_RAWPATH: u32 = 4;

    type Query = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT;
    type Release = unsafe extern "system" fn(*mut c_void) -> u32;
    type SetText = unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT;
    type GetPath = unsafe extern "system" fn(*mut c_void, PWSTR, i32, *mut c_void, u32) -> HRESULT;
    type Load = unsafe extern "system" fn(*mut c_void, PCWSTR, u32) -> HRESULT;
    type Save = unsafe extern "system" fn(*mut c_void, PCWSTR, i32) -> HRESULT;

    /// A COM object, released when dropped.
    struct Object(*mut c_void);

    impl Object {
        /// The function at `index` of the object's method table.
        unsafe fn method<F: Copy>(&self, index: usize) -> F {
            let table = *(self.0 as *const *const *const c_void);
            std::mem::transmute_copy(&*table.add(index))
        }
        unsafe fn query(&self, id: &GUID) -> Result<Object, String> {
            let mut out = null_mut();
            let hr = self.method::<Query>(QUERY_INTERFACE)(self.0, id, &mut out);
            check(hr, "QueryInterface")?;
            Ok(Object(out))
        }
        unsafe fn set(&self, index: usize, value: &[u16], what: &str) -> Result<(), String> {
            check(self.method::<SetText>(index)(self.0, value.as_ptr()), what)
        }
    }

    impl Drop for Object {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { self.method::<Release>(RELEASE)(self.0) };
            }
        }
    }

    fn check(hr: HRESULT, what: &str) -> Result<(), String> {
        if hr >= 0 {
            Ok(())
        } else {
            Err(tf!("{0} 실패 (0x{1})", what, format!("{:08x}", hr as u32)))
        }
    }

    unsafe fn shell_link() -> Result<Object, String> {
        let mut out = null_mut();
        let hr = CoCreateInstance(&CLSID_SHELL_LINK, null_mut(), CLSCTX_INPROC_SERVER, &IID_SHELL_LINK_W, &mut out);
        check(hr, t("바로 가기 준비"))?;
        Ok(Object(out))
    }

    /// Runs `work` on a thread of its own with COM set up, whatever the calling thread has.
    fn with_com<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
        std::thread::spawn(move || {
            check(unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as _) }, t("COM 준비"))?;
            let out = work();
            unsafe { CoUninitialize() };
            out
        })
        .join()
        .map_err(|_| t("바로 가기 작업이 중단되었습니다").to_string())?
    }

    pub fn write_link(at: &Path, link: &Link) -> Result<(), String> {
        let (at, target, arguments) = (wide(at), wide(link.target), wide(link.arguments));
        let (start_in, note) = (wide(link.start_in), wide(link.note));
        with_com(move || unsafe {
            let shell = shell_link()?;
            shell.set(LINK_SET_PATH, &target, t("바로 가기 대상"))?;
            shell.set(LINK_SET_WORKING_DIRECTORY, &start_in, t("바로 가기 시작 위치"))?;
            shell.set(LINK_SET_ARGUMENTS, &arguments, t("바로 가기 인수"))?;
            shell.set(LINK_SET_DESCRIPTION, &note, t("바로 가기 설명"))?;
            let file = shell.query(&IID_PERSIST_FILE)?;
            check(file.method::<Save>(FILE_SAVE)(file.0, at.as_ptr(), 1), t("바로 가기 저장"))
        })
    }

    /// Where a shortcut file points, as stored in it.
    pub fn link_target(at: &Path) -> Option<PathBuf> {
        let at = wide(at);
        with_com(move || unsafe {
            let shell = shell_link()?;
            let file = shell.query(&IID_PERSIST_FILE)?;
            check(file.method::<Load>(FILE_LOAD)(file.0, at.as_ptr(), 0), t("바로 가기 읽기"))?;
            let mut path = [0u16; 1024];
            let hr = shell.method::<GetPath>(LINK_GET_PATH)(shell.0, path.as_mut_ptr(), path.len() as i32, null_mut(), SLGP_RAWPATH);
            check(hr, t("바로 가기 대상 읽기"))?;
            Ok(PathBuf::from(text(&path)))
        })
        .ok()
        .filter(|p| !p.as_os_str().is_empty())
    }

    /// Sets values under a key of the signed-in person's registry, making the key if needed.
    pub fn entry_write(key: &str, values: &[(&str, Value)]) -> Result<(), String> {
        let key = wide(key);
        for (name, value) in values {
            let name = wide(name);
            let status = match value {
                Value::Text(t) => {
                    let data = wide(t);
                    let bytes = (data.len() * 2) as u32;
                    unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), REG_SZ, data.as_ptr() as *const c_void, bytes) }
                }
                Value::Number(n) => unsafe {
                    RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), REG_DWORD, n as *const u32 as *const c_void, 4)
                },
            };
            if status != ERROR_SUCCESS {
                return Err(tf!("설치된 앱 목록에 등록하지 못했습니다 (오류 {0})", status));
            }
        }
        Ok(())
    }

    pub fn entry_text(key: &str, name: &str) -> Option<String> {
        let (key, name) = (wide(key), wide(name));
        let mut data = [0u16; 1024];
        let mut bytes = (data.len() * 2) as u32;
        let status = unsafe {
            RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, null_mut(), data.as_mut_ptr() as *mut c_void, &mut bytes)
        };
        (status == ERROR_SUCCESS).then(|| text(&data).to_string_lossy().into_owned())
    }

    /// Deletes one key (it has no keys below it). A key that is not there is fine.
    pub fn entry_delete(key: &str) -> Result<(), String> {
        let key = wide(key);
        match unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, key.as_ptr()) } {
            ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
            status => Err(tf!("설치된 앱 목록에서 지우지 못했습니다 (오류 {0})", status)),
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{Folder, Link, Value};
    use crate::lang::t;
    use std::path::{Path, PathBuf};
    const ONLY_WINDOWS: &str = "설치는 Windows에서만 지원합니다";
    pub fn long_path(path: &Path) -> PathBuf {
        path.to_path_buf()
    }
    pub fn folder(_: Folder) -> Option<PathBuf> {
        None
    }
    pub fn write_link(_: &Path, _: &Link) -> Result<(), String> {
        Err(t(ONLY_WINDOWS).into())
    }
    pub fn link_target(_: &Path) -> Option<PathBuf> {
        None
    }
    pub fn entry_write(_: &str, _: &[(&str, Value)]) -> Result<(), String> {
        Err(t(ONLY_WINDOWS).into())
    }
    pub fn entry_text(_: &str, _: &str) -> Option<String> {
        None
    }
    pub fn entry_delete(_: &str) -> Result<(), String> {
        Err(t(ONLY_WINDOWS).into())
    }
}

pub use imp::*;

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn personal_folders_are_found() {
        for which in [Folder::StartMenu, Folder::Desktop, Folder::Programs] {
            let path = folder(which).unwrap_or_else(|| panic!("{which:?}"));
            assert!(path.is_absolute(), "{which:?}: {path:?}");
        }
        assert!(folder(Folder::StartMenu).unwrap().is_dir());
        assert!(folder(Folder::Desktop).unwrap().is_dir());
    }

    #[test]
    fn shortcut_file_points_where_it_was_told() {
        // (The temp folder may be given by its short name; a shortcut stores the full one.)
        let dir = long_path(&std::env::temp_dir()).join(format!("EventStudio-link-test-{} 한글 폴더", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(long_path(&dir), dir, "a full path stays as it is");
        assert_eq!(long_path(&dir.join("없는 폴더")), dir.join("없는 폴더"), "and so does one that is not there");
        let target = dir.join("대상 프로그램.exe");
        std::fs::write(&target, b"x").unwrap();
        let at = dir.join("바로 가기.lnk");
        write_link(&at, &Link { target: &target, arguments: "--uninstall", start_in: &dir, note: "설명" }).unwrap();
        assert!(std::fs::metadata(&at).unwrap().len() > 100);
        assert_eq!(link_target(&at).unwrap(), target);
        assert!(link_target(&dir.join("없는 파일.lnk")).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Removes the scratch key even when an assertion fails.
    struct Scratch(String);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = entry_delete(&self.0);
        }
    }

    #[test]
    fn app_list_entry_is_written_read_and_deleted() {
        // A key of its own, made and removed here; the real entry is never touched by a test.
        let key = Scratch(format!(r"Software\EventStudio-SelfTest-{}", std::process::id()));
        assert!(entry_text(&key.0, "DisplayName").is_none());
        let values = [("DisplayName", Value::Text("Event Studio 한글".into())), ("NoModify", Value::Number(1))];
        entry_write(&key.0, &values).unwrap();
        assert_eq!(entry_text(&key.0, "DisplayName").as_deref(), Some("Event Studio 한글"));
        assert!(entry_text(&key.0, "NoModify").is_none(), "a number is not text");
        entry_delete(&key.0).unwrap();
        assert!(entry_text(&key.0, "DisplayName").is_none());
        entry_delete(&key.0).unwrap();
    }
}
