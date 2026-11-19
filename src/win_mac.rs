//! The macOS side of win.rs: the floating timer on AppKit. All sizes are in points, measured
//! from the top-left corner of the primary screen (Cocoa itself counts up from its bottom).
//!
//! What Windows does with a window region and a no-activate tool window is done here in the
//! ways AppKit offers:
//!
//! - Above everything, full-screen slide shows included: the window gets the screen-saver
//!   level and joins every Space, also as an auxiliary of a full-screen one.
//! - Clicks pass through the empty parts: a window takes the mouse as a whole or not at all
//!   (`setIgnoresMouseEvents`), so the pointer is followed and the window takes the mouse only
//!   while the pointer is over a painted part. A window that ignores the mouse, or belongs to
//!   an app that is not in front, is not told where the pointer is: `follow_pointer` gives that
//!   to the program, which hands it to egui.
//! - The keyboard stays with the slide show: a click on a window always brings its app to the
//!   front on macOS, so once the mouse button is up the app that was in front before gets the
//!   front back (`return_focus`).
//!
//! Global hotkeys come from the `global-hotkey` crate (Carbon's RegisterEventHotKey).

use super::{HitShape, Key, PxRect, MOD_ALT_KEY, MOD_CTRL_KEY};
use crate::lang::t;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplicationActivationOptions, NSEvent, NSNormalWindowLevel, NSRunningApplication, NSScreen, NSScreenSaverWindowLevel, NSView, NSWindow,
    NSWindowCollectionBehavior, NSWorkspace,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Once};

/// What is kept between frames while the timer floats (all of it on the main thread).
#[derive(Default)]
struct Floating {
    /// The painted parts of the window; `None`: the whole window takes the mouse.
    shapes: Option<Vec<HitShape>>,
    /// The window lets the mouse through at the moment.
    ignoring: bool,
    /// The app that was in front before this one came to the front.
    other: Option<Retained<NSRunningApplication>>,
    /// The front was handed back since this app last came to it.
    handed_back: bool,
}

thread_local! {
    static FLOATING: RefCell<Floating> = RefCell::new(Floating::default());
}

/// The window of a handle (its content view, win.rs `handle`). Only on the main thread.
fn window(h: isize) -> Option<(Retained<NSWindow>, MainThreadMarker)> {
    let mtm = MainThreadMarker::new()?;
    // The view lives as long as its window, and the handle is only kept while the window is open.
    let view = unsafe { (h as *const NSView).as_ref() }?;
    Some((view.window()?, mtm))
}

/// The height of the primary screen: Cocoa's coordinates start at its bottom-left corner.
fn primary_height(mtm: MainThreadMarker) -> f64 {
    NSScreen::screens(mtm).firstObject().map_or(0.0, |screen| screen.frame().size.height)
}

fn from_cocoa(r: NSRect, top: f64) -> PxRect {
    PxRect {
        x: r.origin.x.round() as i32,
        y: (top - r.origin.y - r.size.height).round() as i32,
        w: r.size.width.round() as i32,
        h: r.size.height.round() as i32,
    }
}

fn to_cocoa(r: PxRect, top: f64) -> NSRect {
    NSRect::new(NSPoint::new(r.x as f64, top - r.y as f64 - r.h as f64), NSSize::new(r.w as f64, r.h as f64))
}

fn holds(r: PxRect, x: i32, y: i32) -> bool {
    x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h
}

pub fn window_rect(h: isize) -> Option<PxRect> {
    let (window, mtm) = window(h)?;
    Some(from_cocoa(window.frame(), primary_height(mtm)))
}

pub fn set_rect(h: isize, r: PxRect) {
    if let Some((window, mtm)) = window(h) {
        window.setFrame_display(to_cocoa(r, primary_height(mtm)), true);
    }
}

/// Puts the window back in front of everything without bringing the app to the front; called
/// once a second.
pub fn keep_on_top(h: isize) {
    if let Some((window, _)) = window(h) {
        if window.isVisible() && !window.isMiniaturized() {
            window.setLevel(NSScreenSaverWindowLevel);
            window.orderFrontRegardless();
        }
    }
}

/// The floating timer's window, or the editor's back again. While it floats the window is
/// above full-screen slide shows on every Space, casts no shadow (a shadow would outline the
/// see-through window) and is left out of the window cycle (Cmd+`).
pub fn set_floating(h: isize, on: bool) {
    let Some((window, _)) = window(h) else { return };
    if on {
        let everywhere = NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary;
        window.setLevel(NSScreenSaverWindowLevel);
        window.setCollectionBehavior(everywhere);
        window.setHasShadow(false);
    } else {
        window.setLevel(NSNormalWindowLevel);
        window.setCollectionBehavior(NSWindowCollectionBehavior::empty());
        window.setHasShadow(true);
        window.setIgnoresMouseEvents(false);
        FLOATING.with_borrow_mut(|f| *f = Floating::default());
    }
}

/// Which parts of the window take the mouse; `None` gives the whole window back.
/// `follow_pointer` acts on it frame by frame.
pub fn set_hit_region(h: isize, shapes: Option<&[HitShape]>) {
    let whole = shapes.is_none();
    let was_ignoring = FLOATING.with_borrow_mut(|f| {
        f.shapes = shapes.map(<[HitShape]>::to_vec);
        let was = f.ignoring;
        f.ignoring = was && !whole;
        was
    });
    if whole && was_ignoring {
        if let Some((window, _)) = window(h) {
            window.setIgnoresMouseEvents(false);
        }
    }
}

/// The pointer is followed by hand on macOS (see the top of this file).
pub const FOLLOWS_POINTER: bool = true;

/// Lets the mouse through the window unless the pointer is over one of its painted parts, and
/// says where in the window the pointer is while it is over one (from the top-left corner).
pub fn follow_pointer(h: isize) -> Option<(i32, i32)> {
    let (window, mtm) = window(h)?;
    let rect = from_cocoa(window.frame(), primary_height(mtm));
    let (x, y) = cursor();
    let at = (x - rect.x, y - rect.y);
    let (over, ignore, changed) = FLOATING.with_borrow_mut(|f| {
        let over = holds(rect, x, y) && f.shapes.as_ref().is_none_or(|shapes| shapes.iter().any(|shape| shape.contains(at)));
        // With no shapes set the whole window takes the mouse, wherever the pointer is.
        let ignore = f.shapes.is_some() && !over;
        (over, ignore, std::mem::replace(&mut f.ignoring, ignore) != ignore)
    });
    if changed {
        window.setIgnoresMouseEvents(ignore);
    }
    over.then_some(at)
}

/// Gives the front back to the app that had it before a click on the floating timer brought
/// this one forward. Called once a frame while the timer floats and no drag is going on; the
/// mouse button has to be up, and the front is handed back once each time this app gets it.
pub fn return_focus() {
    let me = NSRunningApplication::currentApplication().processIdentifier();
    let Some(front) = NSWorkspace::sharedWorkspace().frontmostApplication() else { return };
    let in_front = front.processIdentifier() == me;
    let hand_to = FLOATING.with_borrow_mut(|f| {
        if !in_front {
            f.other = Some(front);
            f.handed_back = false;
            None
        } else if !f.handed_back && NSEvent::pressedMouseButtons() == 0 {
            f.handed_back = true;
            f.other.clone()
        } else {
            None
        }
    });
    if let Some(other) = hand_to {
        other.activateWithOptions(NSApplicationActivationOptions::empty());
    }
}

/// macOS names its language by a code (`ko-KR`), the first of the person's list.
pub fn locale() -> Option<String> {
    let listed = std::process::Command::new("/usr/bin/defaults").args(["read", "-g", "AppleLanguages"]).output().ok()?;
    crate::lang::first_listed(&String::from_utf8_lossy(&listed.stdout))
}

/// (A Windows language id; macOS has `locale` instead.)
pub fn ui_language() -> Option<u16> {
    None
}

pub fn cursor() -> (i32, i32) {
    let Some(mtm) = MainThreadMarker::new() else { return (0, 0) };
    let at = NSEvent::mouseLocation();
    (at.x.round() as i32, (primary_height(mtm) - at.y).round() as i32)
}

/// The physical left button, so a drag ends even if the release happened outside.
pub fn left_button_down() -> bool {
    NSEvent::pressedMouseButtons() & 1 != 0
}

/// Each screen's whole area and its work area (without the menu bar and the Dock).
fn screens() -> Vec<(PxRect, PxRect)> {
    let Some(mtm) = MainThreadMarker::new() else { return Vec::new() };
    let top = primary_height(mtm);
    NSScreen::screens(mtm).to_vec().iter().map(|screen| (from_cocoa(screen.frame(), top), from_cocoa(screen.visibleFrame(), top))).collect()
}

/// Work area of the screen a point is on, else of the primary screen.
pub fn work_area(x: i32, y: i32) -> Option<PxRect> {
    let all = screens();
    all.iter().find(|(whole, _)| holds(*whole, x, y)).or(all.first()).map(|(_, work)| *work)
}

/// Whether a point lies on any screen.
pub fn on_screen(x: i32, y: i32) -> bool {
    screens().iter().any(|(whole, _)| holds(*whole, x, y))
}

/// Whom to tell about a pressed hotkey.
type Tell = Arc<Mutex<Box<dyn Fn(u8) + Send>>>;
/// What a pressed hotkey stands for and whom to tell, by the id `global-hotkey` gives the key.
type Listener = (u8, Tell);
static LISTENERS: Mutex<Option<HashMap<u32, Listener>>> = Mutex::new(None);
static DISPATCH: Once = Once::new();

/// The crate takes one handler for every hotkey of the program, once: it looks the key up.
fn dispatch(event: GlobalHotKeyEvent) {
    if event.state != HotKeyState::Pressed {
        return;
    }
    let found = LISTENERS.lock().ok().and_then(|all| all.as_ref().and_then(|all| all.get(&event.id).cloned()));
    if let Some((id, tell)) = found {
        if let Ok(tell) = tell.lock() {
            tell(id);
        }
    }
}

fn hotkey((_, modifiers, key): Key) -> Option<HotKey> {
    let code = match key {
        0x20 => Code::Space,
        0x31 => Code::Digit1,
        0x32 => Code::Digit2,
        0x33 => Code::Digit3,
        0x51 => Code::KeyQ,
        0x52 => Code::KeyR,
        _ => return None,
    };
    let mut held = Modifiers::empty();
    if modifiers & MOD_CTRL_KEY != 0 {
        held |= Modifiers::CONTROL;
    }
    if modifiers & MOD_ALT_KEY != 0 {
        held |= Modifiers::ALT;
    }
    Some(HotKey::new(Some(held), code))
}

/// Global hotkeys; they are given up when this is dropped.
pub struct Hotkeys {
    manager: GlobalHotKeyManager,
    keys: Vec<HotKey>,
}

/// Registers `keys` system-wide (Ctrl is the Control key, Alt the Option key). `Err` when one
/// of them cannot be had; then none is kept.
pub fn start_hotkeys(keys: &'static [Key], on_key: impl Fn(u8) + Send + 'static) -> Result<Hotkeys, String> {
    let refused = || t("전역 단축키 등록 실패 (다른 프로그램이 사용 중)").to_string();
    DISPATCH.call_once(|| GlobalHotKeyEvent::set_event_handler(Some(dispatch)));
    let manager = GlobalHotKeyManager::new().map_err(|_| refused())?;
    let tell: Tell = Arc::new(Mutex::new(Box::new(on_key)));
    let mut started = Hotkeys { manager, keys: Vec::new() };
    for &key in keys {
        // (A key that is refused drops `started`, which gives up the ones before it.)
        let combination = hotkey(key).ok_or_else(refused)?;
        started.manager.register(combination).map_err(|_| refused())?;
        started.keys.push(combination);
        if let Ok(mut all) = LISTENERS.lock() {
            all.get_or_insert_with(HashMap::new).insert(combination.id(), (key.0, tell.clone()));
        }
    }
    Ok(started)
}

impl Drop for Hotkeys {
    fn drop(&mut self) {
        let _ = self.manager.unregister_all(&self.keys);
        if let Ok(mut all) = LISTENERS.lock() {
            if let Some(all) = all.as_mut() {
                for key in &self.keys {
                    all.remove(&key.id());
                }
            }
        }
    }
}
