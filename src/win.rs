//! The floating timer's window, as each system needs it: Win32 below, AppKit in win_mac.rs.
//! All sizes are in the system's own window units (`units_per_point`): physical pixels on
//! Windows, points on macOS.
//!
//! On Windows:
//!
//! The floating timer is a chrome-less, transparent window that is put back in front of
//! everything once a second without ever taking the keyboard. Only its painted parts take
//! the mouse; the rest lets clicks through to the window behind, the way the empty pixels of
//! a per-pixel-alpha layered window do. While it floats it is a tool window
//! (WS_EX_TOOLWINDOW), so it has no task-bar button, and a no-activate window, so clicking
//! it does not take focus away from the slides. Moving and resizing follow the pointer's
//! screen position, as in VS Event Timer 1.1.0, this program's Electron predecessor.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PxRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl PxRect {
    pub fn to_array(self) -> [i32; 4] {
        [self.x, self.y, self.w, self.h]
    }
    pub fn from_array(a: [i32; 4]) -> Self {
        Self { x: a[0], y: a[1], w: a[2], h: a[3] }
    }
}

/// The work-area top-right corner, `margin` inside (placement.js `topRight`).
pub fn top_right(area: PxRect, w: i32, h: i32, margin: i32) -> PxRect {
    PxRect {
        x: area.x.max(area.x + area.w - w - margin),
        y: area.y + margin.min((area.h - h).max(0)),
        w,
        h,
    }
}

/// Hotkey ids sent by the global hotkey threads.
pub const HOT_TOGGLE: u8 = 1;
pub const HOT_RESET: u8 = 2;
pub const HOT_QNA: u8 = 3;

/// RegisterHotKey modifier flags (Alt 1, Ctrl 2, Shift 4, Win 8).
pub const MOD_ALT_KEY: u32 = 1;
pub const MOD_CTRL_KEY: u32 = 2;

/// One global hotkey: the id sent back, its modifier flags and its virtual key.
pub type Key = (u8, u32, u32);

/// A part of the window that takes the mouse, in window pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitShape {
    Rect(PxRect),
    Ellipse(PxRect),
}

impl HitShape {
    /// Whether a point of the window (from its top-left corner) is on the shape.
    #[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
    pub fn contains(&self, (x, y): (i32, i32)) -> bool {
        match *self {
            HitShape::Rect(r) => x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h,
            HitShape::Ellipse(r) => {
                let (rx, ry) = (r.w as f64 / 2.0, r.h as f64 / 2.0);
                let (dx, dy) = (x as f64 + 0.5 - (r.x as f64 + rx), y as f64 + 0.5 - (r.y as f64 + ry));
                rx > 0.0 && ry > 0.0 && (dx / rx).powi(2) + (dy / ry).powi(2) <= 1.0
            }
        }
    }
}

/// How many window units (`PxRect`) one egui point is. Windows places and sizes windows in
/// physical pixels; macOS in points, which differ from egui's only by the zoom.
pub fn units_per_point(ctx: &eframe::egui::Context) -> f32 {
    if cfg!(target_os = "macos") {
        ctx.zoom_factor()
    } else {
        ctx.pixels_per_point()
    }
}

/// The short hotkeys, registered only while the floating timer is up: Ctrl+1 start / pause,
/// Ctrl+2 reset, Ctrl+3 switch to Q&A.
pub const DIGIT_KEYS: [Key; 3] =
    [(HOT_TOGGLE, MOD_CTRL_KEY, b'1' as u32), (HOT_RESET, MOD_CTRL_KEY, b'2' as u32), (HOT_QNA, MOD_CTRL_KEY, b'3' as u32)];

/// VS Event Timer 1.1.0 main.js `applyHotkeys`: Ctrl+Alt+Space start/pause, Ctrl+Alt+R reset;
/// Ctrl+Alt+Q for Q&A is new.
pub const STUDIO_KEYS: [Key; 3] = [
    (HOT_TOGGLE, MOD_CTRL_KEY | MOD_ALT_KEY, 0x20),
    (HOT_RESET, MOD_CTRL_KEY | MOD_ALT_KEY, b'R' as u32),
    (HOT_QNA, MOD_CTRL_KEY | MOD_ALT_KEY, b'Q' as u32),
];

#[cfg(windows)]
mod imp {
    use super::{HitShape, Key, PxRect};
    use crate::lang::t;
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
    use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
    use windows_sys::Win32::Graphics::Gdi::{
        CombineRgn, CreateEllipticRgn, CreateRectRgn, DeleteObject, GetMonitorInfoW, MonitorFromPoint, SetWindowRgn,
        MONITORINFO, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL, MONITOR_DEFAULTTOPRIMARY, RGN_OR,
    };
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT, VK_LBUTTON,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetMessageW, GetWindowLongPtrW, GetWindowRect, IsIconic, IsWindowVisible, PeekMessageW,
        PostThreadMessageW, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOPMOST, MSG, PM_NOREMOVE,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_SHOWNA, WM_HOTKEY,
        WM_QUIT, WM_USER, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    fn hwnd(h: isize) -> HWND {
        h as HWND
    }

    pub fn window_rect(h: isize) -> Option<PxRect> {
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        (unsafe { GetWindowRect(hwnd(h), &mut r) } != 0)
            .then(|| PxRect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top })
    }

    pub fn set_rect(h: isize, r: PxRect) {
        unsafe { SetWindowPos(hwnd(h), null_mut(), r.x, r.y, r.w, r.h, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER) };
    }

    /// Puts the window back in front of everything; called once a second. Only a shown, not
    /// minimised window, and with `SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE` (0x13), so it
    /// never takes the keyboard.
    pub fn keep_on_top(h: isize) {
        unsafe {
            if IsWindowVisible(hwnd(h)) != 0 && IsIconic(hwnd(h)) == 0 {
                SetWindowPos(hwnd(h), HWND_TOPMOST, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE);
            }
        }
    }

    /// Only `shapes` take the mouse (and show); the rest of the window passes it to the window
    /// behind, as the empty pixels of a per-pixel-alpha layered window do. `None`
    /// gives the whole window back. (A window region, because WS_EX_LAYERED | WS_EX_TRANSPARENT
    /// is all or nothing for the whole window.)
    pub fn set_hit_region(h: isize, shapes: Option<&[HitShape]>) {
        unsafe {
            let Some(shapes) = shapes else {
                SetWindowRgn(hwnd(h), null_mut(), 1);
                return;
            };
            let region = CreateRectRgn(0, 0, 0, 0);
            for s in shapes {
                let part = match *s {
                    HitShape::Rect(r) => CreateRectRgn(r.x, r.y, r.x + r.w, r.y + r.h),
                    HitShape::Ellipse(r) => CreateEllipticRgn(r.x, r.y, r.x + r.w, r.y + r.h),
                };
                CombineRgn(region, region, part, RGN_OR);
                DeleteObject(part);
            }
            // The system owns the region from here on.
            SetWindowRgn(hwnd(h), region, 1);
        }
    }

    /// The floating timer's window styles, or the editor's back again.
    ///
    /// While it floats the window is a tool window (WS_EX_TOOLWINDOW 0x80, and without
    /// WS_EX_APPWINDOW 0x40000), so the timer has no task-bar button and isn't in Alt+Tab. On
    /// top of that it is no-activate, so clicks on it don't take the keyboard from the slide
    /// show.
    pub fn set_floating(h: isize, on: bool) {
        unsafe {
            let ex = GetWindowLongPtrW(hwnd(h), GWL_EXSTYLE);
            let (tool, app, quiet) = (WS_EX_TOOLWINDOW as isize, WS_EX_APPWINDOW as isize, WS_EX_NOACTIVATE as isize);
            let want = if on { (ex | tool | quiet) & !app } else { (ex | app) & !(tool | quiet) };
            if want == ex {
                return;
            }
            // The task bar only takes note of these two styles while the window is hidden.
            let reshow = (ex ^ want) & (tool | app) != 0 && IsWindowVisible(hwnd(h)) != 0;
            if reshow {
                ShowWindow(hwnd(h), SW_HIDE);
            }
            SetWindowLongPtrW(hwnd(h), GWL_EXSTYLE, want);
            if reshow {
                ShowWindow(hwnd(h), SW_SHOWNA);
            }
        }
    }

    /// The language Windows itself is shown in, as a Windows language id.
    pub fn ui_language() -> Option<u16> {
        Some(unsafe { GetUserDefaultUILanguage() })
    }

    /// (macOS names its language by a code instead.)
    pub fn locale() -> Option<String> {
        None
    }

    /// The window region does all of this on Windows: nothing to follow by hand.
    pub const FOLLOWS_POINTER: bool = false;
    pub fn follow_pointer(_: isize) -> Option<(i32, i32)> {
        None
    }
    /// A no-activate window never takes the keyboard on Windows: nothing to hand back.
    pub fn return_focus() {}

    pub fn cursor() -> (i32, i32) {
        let mut p = POINT { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut p) };
        (p.x, p.y)
    }

    /// The physical left button, so a drag ends even if the release happened outside.
    pub fn left_button_down() -> bool {
        (unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } as u16) & 0x8000 != 0
    }

    fn monitor_area(x: i32, y: i32, flags: u32) -> Option<PxRect> {
        unsafe {
            let m = MonitorFromPoint(POINT { x, y }, flags);
            if m.is_null() {
                return None;
            }
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(m, &mut info) == 0 {
                return None;
            }
            let r = info.rcWork;
            Some(PxRect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top })
        }
    }

    /// Work area (screen minus task bar) of the monitor nearest to a point.
    pub fn work_area(x: i32, y: i32) -> Option<PxRect> {
        monitor_area(x, y, MONITOR_DEFAULTTONEAREST).or_else(|| monitor_area(0, 0, MONITOR_DEFAULTTOPRIMARY))
    }

    /// Whether a point lies on any monitor (placement.js `isVisible`).
    pub fn on_screen(x: i32, y: i32) -> bool {
        monitor_area(x, y, MONITOR_DEFAULTTONULL).is_some()
    }

    /// Global hotkeys on their own thread (RegisterHotKey needs a message loop).
    pub struct Hotkeys {
        thread: u32,
        join: Option<std::thread::JoinHandle<()>>,
    }

    /// Registers `keys` system-wide. MOD_NOREPEAT is added, so holding Ctrl+1 does not toggle
    /// the timer over and over. `Err` when another program already owns one of them.
    pub fn start_hotkeys(keys: &'static [Key], on_key: impl Fn(u8) + Send + 'static) -> Result<Hotkeys, String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let join = std::thread::spawn(move || {
            let mut msg: MSG = unsafe { std::mem::zeroed() };
            // Make sure this thread has a message queue before anyone posts to it.
            unsafe { PeekMessageW(&mut msg, null_mut(), WM_USER, WM_USER, PM_NOREMOVE) };
            let mut failed = Vec::new();
            for &(id, mods, vk) in keys {
                if unsafe { RegisterHotKey(null_mut(), id as i32, mods | MOD_NOREPEAT, vk) } == 0 {
                    failed.push(id);
                }
            }
            let _ = tx.send((unsafe { GetCurrentThreadId() }, failed.clone()));
            while unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) } > 0 {
                if msg.message == WM_HOTKEY {
                    on_key(msg.wParam as u8);
                }
            }
            for &(id, _, _) in keys {
                if !failed.contains(&id) {
                    unsafe { UnregisterHotKey(null_mut(), id as i32) };
                }
            }
        });
        let (thread, failed) = rx.recv().map_err(|e| e.to_string())?;
        let keys = Hotkeys { thread, join: Some(join) };
        if failed.is_empty() {
            Ok(keys)
        } else {
            drop(keys);
            Err(t("전역 단축키 등록 실패 (다른 프로그램이 사용 중)").into())
        }
    }

    impl Drop for Hotkeys {
        fn drop(&mut self) {
            unsafe { PostThreadMessageW(self.thread, WM_QUIT, 0, 0) };
            if let Some(j) = self.join.take() {
                let _ = j.join();
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[path = "win_mac.rs"]
mod imp;

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    use super::{HitShape, Key, PxRect};
    use crate::lang::t;
    pub fn ui_language() -> Option<u16> {
        None
    }
    /// The language of the session, as its environment names it (`ko_KR.UTF-8`).
    pub fn locale() -> Option<String> {
        ["LC_ALL", "LC_MESSAGES", "LANG"].into_iter().find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
    }
    pub const FOLLOWS_POINTER: bool = false;
    pub fn follow_pointer(_: isize) -> Option<(i32, i32)> {
        None
    }
    pub fn return_focus() {}
    pub fn window_rect(_: isize) -> Option<PxRect> {
        None
    }
    pub fn set_rect(_: isize, _: PxRect) {}
    pub fn keep_on_top(_: isize) {}
    pub fn set_hit_region(_: isize, _: Option<&[HitShape]>) {}
    pub fn set_floating(_: isize, _: bool) {}
    pub fn cursor() -> (i32, i32) {
        (0, 0)
    }
    pub fn left_button_down() -> bool {
        false
    }
    pub fn work_area(_: i32, _: i32) -> Option<PxRect> {
        None
    }
    pub fn on_screen(_: i32, _: i32) -> bool {
        false
    }
    pub struct Hotkeys;
    pub fn start_hotkeys(_: &'static [Key], _: impl Fn(u8) + Send + 'static) -> Result<Hotkeys, String> {
        Err(t("전역 단축키는 Windows에서만 지원합니다").into())
    }
}

pub use imp::*;

/// The native window behind eframe's root window: its HWND on Windows, its content view
/// (NSView) on macOS.
pub fn handle(frame: &eframe::Frame) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match frame.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        RawWindowHandle::AppKit(h) => Some(h.ns_view.as_ptr() as isize),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn top_right_keeps_margin_and_stays_inside() {
        let area = PxRect { x: 0, y: 0, w: 1920, h: 1040 };
        assert_eq!(top_right(area, 360, 200, 24), PxRect { x: 1536, y: 24, w: 360, h: 200 });
        // Wider than the screen: pinned to the left edge, like placement.js.
        assert_eq!(top_right(area, 2500, 200, 24).x, 0);
        // Taller than the screen: no top margin.
        assert_eq!(top_right(PxRect { x: 100, y: 50, w: 800, h: 150 }, 360, 200, 24).y, 50);
    }
    #[test]
    fn hit_shapes_know_their_points() {
        let rect = HitShape::Rect(PxRect { x: 10, y: 20, w: 30, h: 40 });
        assert!(rect.contains((10, 20)) && rect.contains((39, 59)) && !rect.contains((40, 59)) && !rect.contains((9, 20)));
        let disc = HitShape::Ellipse(PxRect { x: 0, y: 0, w: 100, h: 100 });
        assert!(disc.contains((50, 50)) && disc.contains((50, 0)) && disc.contains((99, 50)));
        assert!(!disc.contains((3, 3)) && !disc.contains((96, 96)) && !disc.contains((120, 50)));
        assert!(!HitShape::Ellipse(PxRect { x: 0, y: 0, w: 0, h: 10 }).contains((0, 5)));
    }
    #[test]
    fn short_hotkeys_are_ctrl_digits() {
        // Ctrl+1 start / pause, Ctrl+2 reset, Ctrl+3 Q&A; Ctrl is MOD_CONTROL (2).
        assert_eq!(DIGIT_KEYS, [(HOT_TOGGLE, 2, 0x31), (HOT_RESET, 2, 0x32), (HOT_QNA, 2, 0x33)]);
        assert!(STUDIO_KEYS.iter().all(|k| k.1 == 3));
    }
}
