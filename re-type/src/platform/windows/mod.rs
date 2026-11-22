// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Windows application: message window, hotkeys, typing worker, tray (REQ-COMP-001).
//!
//! All UI and hotkey work happens on the main thread; each typing run executes on its own
//! short-lived thread so the tray stays responsive and Esc can abort it (REQ-SAFE-001).

mod clipboard;
mod foreground;
mod hotkeys;
mod input;
mod richfield;
mod richhelp;
mod settings;
mod sniper;
mod startup;
mod theme;
mod tray;

use crate::config::{self, Binding, Config};
use crate::hotkey::Hotkey;
use crate::log;
use crate::typing::{self, Outcome};
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use tray::{wide, ID_ELEVATE, ID_HELP, ID_PAUSE, ID_PICK, ID_QUIT, ID_SETTINGS, ID_TYPE_NOW, WM_TRAY};
use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub(crate) const WM_NOTICE: u32 = WM_APP + 2;
const ERROR_ALREADY_EXISTS: u32 = 183;

static BUSY: AtomicBool = AtomicBool::new(false);

struct App {
    hwnd: HWND,
    cfg: Config,
    cfg_path: std::path::PathBuf,
    paused: bool,
    /// Re:Type's own hotkeys are released by the settings window (Release button, Listen).
    released: bool,
    bindings: Vec<Binding>,
    registered: Vec<u32>,
    taskbar_created: u32,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.borrow_mut().as_mut().map(f))
}

pub(crate) fn message_box(owner: HWND, text: &str, flags: u32) {
    // SAFETY: strings are NUL-terminated and outlive the call.
    unsafe { MessageBoxW(owner, wide(text).as_ptr(), wide(crate::PRODUCT_NAME).as_ptr(), flags) };
}

/// A message box that returns the button the user pressed (IDYES = 6, IDNO = 7).
pub(crate) fn message_box_ask(owner: HWND, text: &str, flags: u32) -> i32 {
    // SAFETY: strings are NUL-terminated and outlive the call.
    unsafe { MessageBoxW(owner, wide(text).as_ptr(), wide(crate::PRODUCT_NAME).as_ptr(), flags) }
}

impl App {
    /// Registers the hotkeys of `self.cfg`. Returns one line per hotkey that could not be set.
    fn apply(&mut self) -> Vec<String> {
        for id in self.registered.drain(..) {
            hotkeys::unregister(self.hwnd, id);
        }
        self.bindings = Vec::new();
        let bindings = match self.cfg.bindings() {
            Ok(b) => b,
            Err(errors) => return errors,
        };
        let mut failures = Vec::new();
        for b in &bindings {
            if self.paused || self.released {
                break;
            }
            match hotkeys::register(self.hwnd, b.id, b.hotkey) {
                Ok(()) => self.registered.push(b.id),
                Err(e) => failures.push(format!("{} ({}): {e}", b.label, b.hotkey)),
            }
        }
        self.bindings = bindings;
        for f in &failures {
            log::warn(&format!("hotkey not registered: {f}"));
        }
        failures
    }
}

// ---- services used by the settings window -------------------------------------------------

pub(crate) fn current_config() -> Config {
    with_app(|a| a.cfg.clone()).unwrap_or_default()
}

/// Folder where users can put their own translation files.
pub(crate) fn lang_dir() -> std::path::PathBuf {
    with_app(|a| a.cfg_path.clone()).unwrap_or_else(config::config_path).with_file_name("lang")
}

/// The Windows display language, e.g. `it-IT`.
pub(crate) fn system_language() -> String {
    use windows_sys::Win32::Globalization::{GetUserDefaultUILanguage, LCIDToLocaleName};
    let mut buf = [0u16; 85];
    // SAFETY: the buffer is big enough for a locale name (LOCALE_NAME_MAX_LENGTH).
    let n = unsafe { LCIDToLocaleName(GetUserDefaultUILanguage() as u32, buf.as_mut_ptr(), buf.len() as i32, 0) };
    if n > 1 {
        String::from_utf16_lossy(&buf[..(n - 1) as usize])
    } else {
        String::new()
    }
}

/// Loads the translation for `setting` and logs what had to be ignored.
fn apply_language(cfg_path: &std::path::Path, setting: &str) {
    let dir = cfg_path.with_file_name("lang");
    let (code, notes) = crate::i18n::init(setting, &system_language(), &dir);
    for n in notes {
        log::warn(&format!("translation: {n}"));
    }
    log::info(&format!("language: {code}"));
}

pub(crate) fn config_path_display() -> String {
    with_app(|a| a.cfg_path.display().to_string()).unwrap_or_default()
}

/// Saves `cfg` and registers its hotkeys. Returns the hotkeys Windows refused.
pub(crate) fn commit_config(cfg: Config) -> Result<Vec<String>, String> {
    with_app(|a| {
        config::save(&a.cfg_path, &cfg).map_err(|e| format!("cannot save {}: {e}", a.cfg_path.display()))?;
        apply_language(&a.cfg_path, &cfg.language);
        a.cfg = cfg;
        log::info("configuration saved");
        Ok(a.apply())
    })
    .unwrap_or_else(|| Err("application not ready".into()))
}

/// `Ok` if `hk` is free, or is already held by this application.
pub(crate) fn probe_hotkey(hk: Hotkey) -> Result<(), String> {
    with_app(|a| {
        if a.bindings.iter().any(|b| b.hotkey == hk) && !a.paused && !a.released {
            return Ok(());
        }
        hotkeys::probe(a.hwnd, hk)
    })
    .unwrap_or(Ok(()))
}

/// Releases (`true`) or takes back (`false`) Re:Type's own hotkeys; returns the hotkeys Windows refused
/// when they are taken back.
pub(crate) fn set_hotkeys_released(on: bool) -> Vec<String> {
    with_app(|a| {
        if a.released == on {
            return Vec::new();
        }
        a.released = on;
        log::info(if on { "hotkeys released by the settings window" } else { "hotkeys taken back" });
        a.apply()
    })
    .unwrap_or_default()
}

pub(crate) fn hotkeys_released() -> bool {
    with_app(|a| a.released).unwrap_or(false)
}

pub(crate) fn first_free(candidates: &[Hotkey]) -> Option<Hotkey> {
    with_app(|a| {
        let held: Vec<Hotkey> = a.bindings.iter().map(|b| b.hotkey).collect();
        candidates.iter().copied().find(|c| !held.contains(c) && hotkeys::probe(a.hwnd, *c).is_ok())
    })
    .flatten()
}

// ---- typing -------------------------------------------------------------------------------

const T_NOT_ELEVATED: &str = "Re:Type could not get administrator rights (the request was declined), so it runs without them. Windows will ask again the next time Re:Type starts.";

const T_ELEVATE_DECLINED: &str = "Re:Type could not get administrator rights (the request was declined). It keeps running without them.";

/// Tray command: restart Re:Type with administrator rights for this run only (REQ-UX-010). Nothing
/// is saved. The hotkeys are released first because the elevated copy registers the same ones;
/// they are taken back if Windows or the user refuses the request.
fn elevate_now(hwnd: HWND) {
    if foreground::self_elevated() {
        return;
    }
    let held_by_settings = hotkeys_released();
    set_hotkeys_released(true);
    if restart_elevated() {
        log::info("restarting with administrator rights (tray command)");
        // SAFETY: destroying our own window; the elevated copy takes over.
        unsafe { DestroyWindow(hwnd) };
        return;
    }
    log::warn(T_ELEVATE_DECLINED);
    if !held_by_settings {
        set_hotkeys_released(false);
    }
    tray::notify(hwnd, "Re:Type", &crate::i18n::tr(T_ELEVATE_DECLINED));
}

fn outcome_notice(o: &Outcome, limit: usize) -> Option<String> {
    match o {
        Outcome::Completed { .. } => None,
        Outcome::Aborted { .. } => Some(crate::i18n::tr("Typing aborted (Esc).")),
        Outcome::NoText => Some(crate::i18n::tr("The clipboard holds no text.")),
        Outcome::Image => Some(crate::i18n::tr("The clipboard holds an image, which cannot be typed. Nothing was typed.")),
        Outcome::NotText => Some(crate::i18n::tr("The clipboard holds content that is not text. Nothing was typed.")),
        Outcome::FilesIgnored => Some(crate::i18n::tr("The clipboard holds copied files, and typing them is switched off in Settings.")),
        Outcome::TooLong { .. } => Some(crate::i18n::trf("The clipboard text is longer than the limit of {0} characters.", &[&limit.to_string()])),
        Outcome::ClipboardBusy => Some(crate::i18n::tr("The clipboard is in use or not answering. Try again.")),
        Outcome::Failed(e) => Some(e.clone()),
    }
}

fn start_typing(hwnd: HWND, b: &Binding, cfg: &Config) {
    start_run(hwnd, b.segments.clone(), b.hotkey.mods, b.label.clone(), cfg, None, false);
}

/// "Type clipboard now" from the tray menu: types the clipboard into the window the user was
/// working in (REQ-UX-006).
fn type_now(hwnd: HWND) {
    let name = crate::PRODUCT_NAME;
    let Some(target) = foreground::target() else {
        tray::notify(hwnd, name, &crate::i18n::tr("Click into the window you want to type in, then choose the command again."));
        return;
    };
    let Some(cfg) = with_app(|a| a.cfg.clone()) else { return };
    match crate::pattern::parse("{CLIPBOARD}") {
        Ok(segs) => start_run(hwnd, segs, 0, "tray: type clipboard now".into(), &cfg, Some((target, foreground::control(), None)), false),
        Err(_) => log::error("internal error: the plain clipboard pattern does not parse"),
    }
}

/// "Pick target and type": the user clicks the control to type into (REQ-UX-008).
fn pick_begin(hwnd: HWND) {
    if BUSY.load(Ordering::SeqCst) || sniper::active() || !with_app(|a| a.cfg.pick_enabled).unwrap_or(true) {
        return;
    }
    if sniper::start(hwnd) {
        log::info("pick target: started");
        tray::notify(
            hwnd,
            crate::PRODUCT_NAME,
            &crate::i18n::tr("Click the field you want to type in. Esc or a right click cancels."),
        );
    } else {
        tray::notify(hwnd, crate::PRODUCT_NAME, &crate::i18n::tr("Pick target could not start."));
    }
}

fn pick_finish(hwnd: HWND, pt: windows_sys::Win32::Foundation::POINT) {
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    let name = crate::PRODUCT_NAME;
    let hit = sniper::control_at(pt);
    sniper::stop();
    let Some(ctl) = hit else {
        tray::notify(hwnd, name, &crate::i18n::tr("Nothing was picked."));
        return;
    };
    // SAFETY: plain window queries on a handle that was just returned by the system.
    let (root, own, class) = unsafe {
        let root = GetAncestor(ctl, GA_ROOT);
        let mut pid = 0u32;
        GetWindowThreadProcessId(root, &mut pid);
        (root, pid == GetCurrentProcessId(), foreground::class_of(ctl))
    };
    let refusal = if root.is_null() || own {
        Some(crate::i18n::tr("Pick a field in another program."))
    } else if foreground::is_shell(root) {
        Some(crate::i18n::tr("The taskbar and menus cannot receive typing."))
    } else if matches!(class.as_str(), "Button" | "Static") {
        // Space and Enter would press a button
        Some(crate::i18n::tr("That is not a text field, so nothing was typed."))
    } else {
        None
    };
    if let Some(text) = refusal {
        log::info(&format!("pick target: refused ({class})"));
        tray::notify(hwnd, name, &text);
        return;
    }
    // windows that draw their own fields cannot be given the focus on one field directly
    let draws_own = ["Chrome_", "Mozilla", "Intermediate D3D", "HwndWrapper[", "Windows.UI.", "Qt", "SunAwt"]
        .iter()
        .any(|p| class.starts_with(p));
    log::info(&format!("pick target: {class}{}", if draws_own { " (click)" } else { "" }));
    let ctl_for_focus = if draws_own { 0 } else { ctl as isize };
    let Some(cfg) = with_app(|a| a.cfg.clone()) else { return };
    match crate::pattern::parse("{CLIPBOARD}") {
        Ok(segs) => start_run(hwnd, segs, 0, "tray: pick target and type".into(), &cfg, Some((root as isize, ctl_for_focus, Some((pt.x, pt.y)))), true),
        Err(_) => log::error("internal error: the plain clipboard pattern does not parse"),
    }
}

fn start_run(hwnd: HWND, segs: Vec<crate::pattern::Segment>, mods: u32, label: String, cfg: &Config, target: Option<(isize, isize, Option<(i32, i32)>)>, strict: bool) {
    if BUSY.swap(true, Ordering::SeqCst) {
        return; // a run is already in progress
    }
    let (hwnd_raw, opts) = (hwnd as isize, cfg.run_options());
    let spawned = std::thread::Builder::new().name("typing".into()).spawn(move || {
        // From the tray the taskbar has the focus: hand it back to the user's window, and type
        // nothing if that did not work.
        // Windows drops keystrokes sent to a window that runs as administrator, and says nothing:
        // refuse up front, so that no run is reported as completed when nothing was typed.
        let window = match target {
            Some((w, _, _)) => w,
            None => (unsafe { GetForegroundWindow() }) as isize,
        };
        let outcome = if foreground::blocked_by_elevation(window) {
            Outcome::Failed(
                crate::i18n::tr("The window runs as administrator, and Windows does not let Re:Type type into it. Nothing was typed. Start Re:Type as administrator to type there."),
            )
        } else {
            match target {
            Some((w, c, click)) => {
                // Some windows draw their own fields (web pages): the focus cannot be given to
                // one field directly, so the picked point is clicked (control 0), as it is when
                // the control refuses the focus.
                let ready = foreground::bring_to_front(w, c, strict);
                let by_click = matches!((&ready, click), (Ok(()), Some(_)) if c == 0)
                    || matches!((&ready, click), (Err(foreground::Refused::Control), Some(_)));
                match ready {
                    Err(foreground::Refused::Window) => {
                        Outcome::Failed(crate::i18n::tr("Could not return to the window you were working in. Nothing was typed."))
                    }
                    _ if by_click => match click {
                        Some((x, y)) if foreground::click_at(w, x, y) => {
                            log::info("pick target: focus by click");
                            typing::run(&input::WinBackend, mods, &segs, &opts)
                        }
                        _ => Outcome::Failed(crate::i18n::tr("The picked field did not take the keyboard focus, not even by a click. Nothing was typed.")),
                    },
                    Err(foreground::Refused::Control) => {
                        Outcome::Failed(crate::i18n::tr("The picked control did not take the keyboard focus. Nothing was typed."))
                    }
                    Ok(()) => typing::run(&input::WinBackend, mods, &segs, &opts),
                }
            }
            None => typing::run(&input::WinBackend, mods, &segs, &opts),
            }
        };
        BUSY.store(false, Ordering::SeqCst);
        // log events and counts only, never text (REQ-SEC-001)
        log::info(&format!("{label}: {outcome:?}"));
        if let Some(text) = outcome_notice(&outcome, opts.max_chars) {
            let boxed = Box::into_raw(Box::new(text));
            // SAFETY: the main window frees the box in WM_NOTICE; if posting fails we free it here.
            unsafe {
                if PostMessageW(hwnd_raw as HWND, WM_NOTICE, 0, boxed as LPARAM) == 0 {
                    drop(Box::from_raw(boxed));
                }
            }
        }
    });
    if let Err(e) = spawned {
        BUSY.store(false, Ordering::SeqCst);
        log::error(&format!("cannot start typing thread: {e}"));
    }
}

// ---- main window --------------------------------------------------------------------------

unsafe extern "system" fn main_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match catch_unwind(AssertUnwindSafe(|| handle(hwnd, msg, wp, lp))) {
        Ok(Some(r)) => r,
        Ok(None) => DefWindowProcW(hwnd, msg, wp, lp),
        Err(_) => {
            log::error("internal error in the main window");
            DefWindowProcW(hwnd, msg, wp, lp)
        }
    }
}

fn handle(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
    match msg {
        sniper::WM_SNIPER => {
            if sniper::active() {
                match wp {
                    sniper::EV_MOVE => sniper::on_move(sniper::unpack(lp)),
                    sniper::EV_PICK => pick_finish(hwnd, sniper::unpack(lp)),
                    _ => {
                        sniper::stop();
                        log::info("pick target: cancelled");
                    }
                }
            }
            Some(0)
        }
        WM_TIMER if wp == sniper::TIMER_ID => {
            sniper::stop();
            log::info("pick target: timed out");
            tray::notify(hwnd, crate::PRODUCT_NAME, &crate::i18n::tr("Pick target timed out."));
            Some(0)
        }
        WM_HOTKEY if wp as i32 == sniper::ESC_ID => {
            sniper::stop();
            log::info("pick target: cancelled");
            Some(0)
        }
        WM_HOTKEY if wp as u32 == config::PICK_ID => {
            pick_begin(hwnd);
            Some(0)
        }
        WM_HOTKEY => {
            let id = wp as u32;
            let job = with_app(|a| a.bindings.iter().find(|b| b.id == id).cloned().map(|b| (b, a.cfg.clone())));
            if let Some(Some((b, cfg))) = job {
                start_typing(hwnd, &b, &cfg);
            }
            Some(0)
        }
        WM_TRAY => {
            const NIN_KEYSELECT: u32 = 0x401;
            let event = (lp & 0xFFFF) as u32;
            let paused = with_app(|a| a.paused).unwrap_or(false);
            let show = |at: Option<(i32, i32)>| {
                // the menu follows the colour setting of the moment
                if let Some(t) = with_app(|a| a.cfg.theme) {
                    theme::init_menus(t);
                }
                let pick = with_app(|a| a.cfg.pick_enabled).unwrap_or(true);
                tray::show_menu(hwnd, paused, pick, !foreground::self_elevated(), at);
            };
            if tray::uses_v4() {
                // version 4: the menu key and Shift+F10 arrive as WM_CONTEXTMENU, Enter as
                // NIN_KEYSELECT; wParam holds the position of the icon (REQ-ACC-005)
                let (x, y) = ((wp & 0xFFFF) as i16 as i32, ((wp >> 16) & 0xFFFF) as i16 as i32);
                match event {
                    WM_LBUTTONDBLCLK | NIN_KEYSELECT => settings::open(hwnd),
                    WM_CONTEXTMENU => show(Some((x, y))),
                    _ => {}
                }
            } else {
                match event {
                    WM_LBUTTONDBLCLK => settings::open(hwnd),
                    WM_RBUTTONUP | WM_CONTEXTMENU => show(None),
                    _ => {}
                }
            }
            Some(0)
        }
        WM_COMMAND => {
            match wp & 0xFFFF {
                ID_SETTINGS => settings::open(hwnd),
                ID_HELP => settings::open_help(hwnd),
                ID_TYPE_NOW => type_now(hwnd),
                ID_PICK => pick_begin(hwnd),
                ID_ELEVATE => elevate_now(hwnd),
                ID_PAUSE => {
                    let failures = with_app(|a| {
                        a.paused = !a.paused;
                        log::info(if a.paused { "hotkeys paused" } else { "hotkeys resumed" });
                        a.apply()
                    });
                    if let Some(f) = failures.filter(|f| !f.is_empty()) {
                        tray::notify(hwnd, "Re:Type", &crate::i18n::trf("Not registered: {0}", &[&f.join("; ")]));
                    }
                }
                ID_QUIT => {
                    // SAFETY: destroying our own window.
                    unsafe { DestroyWindow(hwnd) };
                }
                _ => {}
            }
            Some(0)
        }
        WM_NOTICE => {
            // SAFETY: pointer produced by Box::into_raw in `start_typing`, consumed exactly once.
            let text = unsafe { *Box::from_raw(lp as *mut String) };
            tray::notify(hwnd, crate::PRODUCT_NAME, &text);
            Some(0)
        }
        WM_DESTROY => {
            with_app(|a| {
                for id in a.registered.drain(..) {
                    hotkeys::unregister(hwnd, id);
                }
            });
            tray::remove(hwnd);
            // SAFETY: ends the message loop.
            unsafe { PostQuitMessage(0) };
            Some(0)
        }
        m if with_app(|a| a.taskbar_created != 0 && a.taskbar_created == m).unwrap_or(false) => {
            // Explorer restarted: the tray icon is gone, add it again.
            tray::add(hwnd, &product_title());
            Some(0)
        }
        _ => None,
    }
}

/// The single-instance mutex (kept for the life of the process, or released for a restart).
static MUTEX: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Takes the single-instance mutex. False if another copy of Re:Type holds it.
fn take_mutex() -> bool {
    // SAFETY: the handle is stored and closed only by `release_mutex`.
    unsafe {
        let h = CreateMutexW(std::ptr::null(), 0, wide("Local\\re-type.single-instance").as_ptr());
        // access denied: a copy running as administrator created it and this one is not elevated
        if h.is_null() || GetLastError() == ERROR_ALREADY_EXISTS {
            return false;
        }
        MUTEX.store(h as isize, Ordering::SeqCst);
        true
    }
}

fn release_mutex() {
    let h = MUTEX.swap(0, Ordering::SeqCst);
    if h != 0 {
        // SAFETY: the handle came from CreateMutexW and is closed once.
        unsafe { windows_sys::Win32::Foundation::CloseHandle(h as _) };
    }
}

/// Starts a copy of this program with administrator rights (UAC consent prompt). True if Windows
/// started it; the caller must then end this copy. The mutex is released for the new copy and
/// taken again if the request fails or is declined.
pub(crate) fn restart_elevated() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    release_mutex();
    // SAFETY: plain call with NUL-terminated strings.
    let rc = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            wide("runas").as_ptr(),
            wide(&exe.display().to_string()).as_ptr(),
            wide("--elevated").as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if rc > 32 {
        true
    } else {
        take_mutex();
        false
    }
}

/// Product name, with a mark when running as administrator.
pub(crate) fn product_title() -> String {
    if foreground::self_elevated() {
        format!("{} (administrator)", crate::PRODUCT_NAME)
    } else {
        crate::PRODUCT_NAME.to_string()
    }
}

/// Starts the application and runs until the user quits.
pub fn run() {
    let dir = config::config_dir();
    log::init(dir.join("re-type.log"));
    log::info(&format!("{} starting{}", crate::PRODUCT_NAME, if foreground::self_elevated() { " (administrator)" } else { "" }));
    // SAFETY: process-wide setting made once at start-up; failure (older Windows) is harmless.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    // one instance only (REQ-UX-003)
    if !take_mutex() {
        message_box(std::ptr::null_mut(), &crate::i18n::tr("Re:Type is already running. Look for its icon in the system tray."), MB_OK | MB_ICONINFORMATION);
        return;
    }

    let cfg_path = config::config_path();
    let (cfg, warnings) = config::load(&cfg_path);
    let mut warnings = warnings;
    apply_language(&cfg_path, &cfg.language);
    for w in &warnings {
        log::warn(w);
    }

    // Run as administrator (REQ-UX-010): ask Windows for the rights and restart; the copy that
    // starts elevated takes over. If the user declines, carry on without them.
    if cfg.elevated && !foreground::self_elevated() && !std::env::args().any(|a| a == "--no-elevate") {
        if restart_elevated() {
            log::info("restarting with administrator rights");
            return;
        }
        log::warn(T_NOT_ELEVATED);
        warnings.push(crate::i18n::tr(T_NOT_ELEVATED));
    }
    startup::repair(cfg.start_at_logon);

    // SAFETY: standard window-class registration and creation on this thread.
    let hwnd = unsafe {
        let hinst = GetModuleHandleW(std::ptr::null());
        let class = wide("ReTypeMain");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(main_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: tray::app_icon(true),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
            hIconSm: tray::app_icon(false),
        };
        RegisterClassExW(&wc);
        // a message-only window: invisible, receives hotkey and tray messages
        CreateWindowExW(0, class.as_ptr(), class.as_ptr(), 0, 0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(), hinst, std::ptr::null())
    };
    if hwnd.is_null() {
        log::error("cannot create the main window");
        message_box(std::ptr::null_mut(), "Cannot start: the main window could not be created.", MB_OK | MB_ICONERROR);
        return;
    }

    // SAFETY: plain registration of a broadcast message name.
    let taskbar_created = unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) };
    let cfg_theme = cfg.theme;
    APP.with(|a| {
        *a.borrow_mut() =
            Some(App { hwnd, cfg, cfg_path, paused: false, released: false, bindings: vec![], registered: vec![], taskbar_created })
    });
    theme::init_menus(cfg_theme);
    tray::add(hwnd, &product_title());
    let fg_hook = foreground::install();

    let mut problems = warnings;
    problems.extend(with_app(|a| a.apply()).unwrap_or_default());
    if !problems.is_empty() {
        tray::notify(hwnd, &crate::i18n::tr("Re:Type needs attention"), &problems.join("\n"));
    }

    // SAFETY: standard message loop; IsDialogMessageW gives the settings window Tab navigation.
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if settings::is_dialog_message(&msg) {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    sniper::stop();
    foreground::remove(fg_hook);
    log::info("exiting");
}
