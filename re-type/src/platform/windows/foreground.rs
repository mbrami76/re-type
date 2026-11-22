// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Remembers the window the user was working in, and the control that had the keyboard focus
//! in it, so that "Type clipboard now" from the tray menu can hand the focus back before
//! typing (REQ-UX-006).
//!
//! Clicking the tray icon makes the taskbar the foreground window, so by the time the command
//! runs the window the user wanted is no longer in front. Only the handles of that window and
//! of its focused control are kept; nothing about their content. Windows of the shell (the
//! taskbar, the tray overflow, menus) never replace the remembered ones.

use std::sync::atomic::{AtomicIsize, Ordering};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetAncestor, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
    IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow, ShowWindow, EVENT_OBJECT_FOCUS, EVENT_SYSTEM_FOREGROUND,
    GA_ROOT, GUITHREADINFO, SW_RESTORE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

/// The top-level window the user was last working in.
static TARGET: AtomicIsize = AtomicIsize::new(0);
/// The control of that window that had the keyboard focus (0 when unknown).
static CONTROL: AtomicIsize = AtomicIsize::new(0);

/// `OBJID_CLIENT` as the signed value WinEvent callbacks receive.
const OBJID_CLIENT: i32 = -4;

/// Windows of the shell that take the focus when the tray is used. Typing into them is never wanted.
const SHELL_CLASSES: [&str; 6] = [
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "NotifyIconOverflowWindow",
    "TopLevelWindowForOverflowXamlIsland",
    "XamlExplorerHostIslandWindow",
    "#32768",
];

pub fn class_of(h: HWND) -> String {
    let mut buf = [0u16; 64];
    // SAFETY: `buf` is a valid buffer of the length passed.
    let n = unsafe { GetClassNameW(h, buf.as_mut_ptr(), buf.len() as i32) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

pub fn is_shell(h: HWND) -> bool {
    let c = class_of(h);
    SHELL_CLASSES.iter().any(|s| *s == c)
}

/// The control with the keyboard focus in the thread that owns `window`, if it is inside `window`.
unsafe fn focus_of(window: HWND) -> HWND {
    let thread = GetWindowThreadProcessId(window, std::ptr::null_mut());
    let mut info: GUITHREADINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    if thread != 0
        && GetGUIThreadInfo(thread, &mut info) != 0
        && !info.hwndFocus.is_null()
        && GetAncestor(info.hwndFocus, GA_ROOT) == window
    {
        info.hwndFocus
    } else {
        std::ptr::null_mut()
    }
}

unsafe extern "system" fn on_foreground(_hook: HWINEVENTHOOK, _event: u32, hwnd: HWND, _obj: i32, _child: i32, _thread: u32, _time: u32) {
    if !hwnd.is_null() && IsWindowVisible(hwnd) != 0 && !is_shell(hwnd) {
        TARGET.store(hwnd as isize, Ordering::SeqCst);
        CONTROL.store(focus_of(hwnd) as isize, Ordering::SeqCst);
    }
}

unsafe extern "system" fn on_focus(_hook: HWINEVENTHOOK, _event: u32, hwnd: HWND, obj: i32, _child: i32, _thread: u32, _time: u32) {
    if hwnd.is_null() || obj != OBJID_CLIENT {
        return;
    }
    let root = GetAncestor(hwnd, GA_ROOT);
    if !root.is_null() && IsWindowVisible(root) != 0 && !is_shell(root) {
        TARGET.store(root as isize, Ordering::SeqCst);
        CONTROL.store(hwnd as isize, Ordering::SeqCst);
    }
}

pub struct Hooks(HWINEVENTHOOK, HWINEVENTHOOK);

/// Starts following the foreground window and its focused control. Events of this process are
/// skipped, so the settings window and the tray menu never replace the remembered window.
pub fn install() -> Hooks {
    // SAFETY: the callbacks only touch atomics and query windows; the hooks are removed with `remove`.
    unsafe {
        let h = GetForegroundWindow();
        if !h.is_null() && !is_shell(h) {
            TARGET.store(h as isize, Ordering::SeqCst);
            CONTROL.store(focus_of(h) as isize, Ordering::SeqCst);
        }
        let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
        let fg = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, std::ptr::null_mut(), Some(on_foreground), 0, 0, flags);
        let focus = SetWinEventHook(EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS, std::ptr::null_mut(), Some(on_focus), 0, 0, flags);
        Hooks(fg, focus)
    }
}

pub fn remove(hooks: Hooks) {
    for h in [hooks.0, hooks.1] {
        if !h.is_null() {
            // SAFETY: `h` came from `install`.
            unsafe { UnhookWinEvent(h) };
        }
    }
}

/// The control that had the keyboard focus in that window (0 when unknown).
pub fn control() -> isize {
    CONTROL.load(Ordering::SeqCst)
}

/// Why the typing target could not be made ready.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// The window did not come to the front.
    Window,
    /// The window is in front, but the chosen control did not take the keyboard focus.
    Control,
}

/// The window the user was last working in, if it still exists.
pub fn target() -> Option<isize> {
    let t = TARGET.load(Ordering::SeqCst);
    // SAFETY: plain query.
    (t != 0 && unsafe { IsWindow(t as HWND) } != 0).then_some(t)
}

/// Brings `target` to the front and puts the keyboard focus on `control` (0: leave the window's
/// own focus). With `strict` the control must really hold the focus afterwards. The caller
/// must check the result before typing: never type into whatever else holds the focus.
pub fn bring_to_front(target: isize, control: isize, strict: bool) -> Result<(), Refused> {
    // SAFETY: plain window calls on handles that are checked with IsWindow first.
    unsafe {
        let h = target as HWND;
        if IsWindow(h) == 0 {
            return Err(Refused::Window);
        }
        if IsIconic(h) != 0 {
            ShowWindow(h, SW_RESTORE);
        }
        // Windows only lets the process that has the focus change it. Sharing the input queue of
        // the current foreground window's thread for a moment is the documented way around that.
        let current = GetForegroundWindow();
        let mine = GetCurrentThreadId();
        let theirs = if current.is_null() { 0 } else { GetWindowThreadProcessId(current, std::ptr::null_mut()) };
        let attached = theirs != 0 && theirs != mine && AttachThreadInput(mine, theirs, 1) != 0;
        BringWindowToTop(h);
        SetForegroundWindow(h);
        if attached {
            AttachThreadInput(mine, theirs, 0);
        }
        let mut in_front = false;
        for _ in 0..20 {
            if GetForegroundWindow() == h {
                in_front = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        if !in_front {
            return Err(Refused::Window);
        }
        restore_control(h, control as HWND);
        if strict && control != 0 && focus_of(h) as isize != control {
            return Err(Refused::Control);
        }
        Ok(())
    }
}

/// Puts the keyboard focus back on the remembered control of `window`, if it still exists and
/// belongs to it. Failing to do so is not an error: the window keeps its own focus then.
unsafe fn restore_control(window: HWND, c: HWND) {
    if c.is_null() || IsWindow(c) == 0 || GetAncestor(c, GA_ROOT) != window || IsWindowVisible(c) == 0 {
        return;
    }
    if focus_of(window) == c {
        return; // already there
    }
    // the control belongs to the thread of the window, so share its input queue for the call
    let mine = GetCurrentThreadId();
    let theirs = GetWindowThreadProcessId(window, std::ptr::null_mut());
    let attached = theirs != 0 && theirs != mine && AttachThreadInput(mine, theirs, 1) != 0;
    SetFocus(c);
    if attached {
        AttachThreadInput(mine, theirs, 0);
    }
}

/// Clicks the left mouse button at the screen point (`x`, `y`) and puts the pointer back. Used
/// only as the fallback of "Pick target and type" for controls that refuse the keyboard focus
/// directly (web pages, programs that draw their own fields). Returns whether `window` is the
/// foreground window afterwards.
pub fn click_at(window: isize, x: i32, y: i32) -> bool {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};
    // SAFETY: plain input calls with fully initialised structures.
    unsafe {
        let mut old = POINT { x: 0, y: 0 };
        GetCursorPos(&mut old);
        SetCursorPos(x, y);
        let ev = |flags| INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 { mi: MOUSEINPUT { dx: 0, dy: 0, mouseData: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        };
        let events = [ev(MOUSEEVENTF_LEFTDOWN), ev(MOUSEEVENTF_LEFTUP)];
        let sent = SendInput(2, events.as_ptr(), std::mem::size_of::<INPUT>() as i32);
        std::thread::sleep(std::time::Duration::from_millis(200));
        SetCursorPos(old.x, old.y);
        sent == 2 && GetAncestor(GetForegroundWindow(), GA_ROOT) == window as HWND
    }
}

/// True when the window belongs to a process that runs elevated (as administrator) while Re:Type
/// does not. Windows then silently drops the keystrokes we send (UIPI), and `SendInput` still
/// reports success, so this has to be checked beforehand. An unknown answer counts as "no".
pub fn blocked_by_elevation(window: isize) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
    // SAFETY: plain queries; the process handle is closed before returning.
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(window as HWND, &mut pid);
        if pid == 0 {
            return false;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return false;
        }
        let theirs = token_elevated(process);
        CloseHandle(process);
        matches!((theirs, token_elevated(GetCurrentProcess())), (Some(true), Some(false)))
    }
}

/// Whether the process behind `process` runs elevated (as administrator); `None` if unknown.
fn token_elevated(process: windows_sys::Win32::Foundation::HANDLE) -> Option<bool> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::OpenProcessToken;
    // SAFETY: queries the token of a process handle that the caller owns.
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let mut info = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut info as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        (ok != 0).then_some(info.TokenIsElevated != 0)
    }
}

/// True when this program runs elevated (as administrator).
pub fn self_elevated() -> bool {
    // SAFETY: the pseudo handle of the current process needs no closing.
    unsafe { token_elevated(windows_sys::Win32::System::Threading::GetCurrentProcess()) }.unwrap_or(false)
}
