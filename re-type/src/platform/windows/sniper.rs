// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! "Pick target and type": the user points at the control that should receive the typing and
//! clicks it (REQ-UX-008).
//!
//! While picking, a low-level mouse hook swallows the clicks (so nothing under the pointer is
//! pressed), a thin frame follows the control under the pointer, Esc or a right click cancels,
//! and a timer ends the mode after 30 seconds. The hook only looks at mouse messages and is
//! removed on every way out. Only window handles are kept, and only for the one run.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, CreateSolidBrush, DeleteObject, SetWindowRgn, RGN_DIFF};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, VK_ESCAPE};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Message from the hook to the main window: wParam is the event, lParam the screen position.
pub const WM_SNIPER: u32 = WM_APP + 3;
pub const EV_MOVE: usize = 1;
pub const EV_PICK: usize = 2;
pub const EV_CANCEL: usize = 3;
/// Id of the Esc hotkey and of the timeout timer on the main window.
pub const ESC_ID: i32 = 0x7F01;
pub const TIMER_ID: usize = 0x7F02;
const TIMEOUT_MS: u32 = 30_000;
const FRAME_CLASS: &str = "ReTypePickFrame";
const BORDER: i32 = 3;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static HOOK: AtomicIsize = AtomicIsize::new(0);
static MAIN: AtomicIsize = AtomicIsize::new(0);
static FRAME: AtomicIsize = AtomicIsize::new(0);
static LAST: AtomicIsize = AtomicIsize::new(0);

pub fn active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn pack(x: i32, y: i32) -> LPARAM {
    (((y as i16 as u16 as u32) << 16) | (x as i16 as u16 as u32)) as LPARAM
}

pub fn unpack(lp: LPARAM) -> POINT {
    POINT { x: (lp & 0xFFFF) as u16 as i16 as i32, y: ((lp >> 16) & 0xFFFF) as u16 as i16 as i32 }
}

unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && ACTIVE.load(Ordering::SeqCst) {
        let info = &*(lp as *const MSLLHOOKSTRUCT);
        let main = MAIN.load(Ordering::SeqCst) as HWND;
        let post = |ev: usize| {
            PostMessageW(main, WM_SNIPER, ev, pack(info.pt.x, info.pt.y));
        };
        match wp as u32 {
            WM_MOUSEMOVE => post(EV_MOVE),
            WM_LBUTTONDOWN | WM_RBUTTONDOWN => return 1,
            WM_LBUTTONUP => {
                post(EV_PICK);
                return 1;
            }
            WM_RBUTTONUP => {
                post(EV_CANCEL);
                return 1;
            }
            _ => {}
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wp, lp)
}

unsafe extern "system" fn frame_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, wp, lp)
}

fn make_frame() -> HWND {
    // SAFETY: standard class registration and window creation on the main thread.
    unsafe {
        let hinst = GetModuleHandleW(std::ptr::null());
        let class = wide(FRAME_CLASS);
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(frame_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_CROSS),
            hbrBackground: CreateSolidBrush(0x0020_B0FF), // amber, as BGR
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        RegisterClassExW(&wc);
        let h = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
            class.as_ptr(),
            wide("Re:Type target").as_ptr(),
            WS_POPUP,
            0,
            0,
            10,
            10,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        );
        if !h.is_null() {
            SetLayeredWindowAttributes(h, 0, 230, LWA_ALPHA);
        }
        h
    }
}

/// Starts picking. Returns false if it is already running or the hook could not be installed.
pub fn start(main: HWND) -> bool {
    if ACTIVE.swap(true, Ordering::SeqCst) {
        return false;
    }
    // SAFETY: hook, hotkey and timer are all released by `stop`.
    unsafe {
        MAIN.store(main as isize, Ordering::SeqCst);
        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), GetModuleHandleW(std::ptr::null()), 0);
        if hook.is_null() {
            ACTIVE.store(false, Ordering::SeqCst);
            return false;
        }
        HOOK.store(hook as isize, Ordering::SeqCst);
        RegisterHotKey(main, ESC_ID, 0, u32::from(VK_ESCAPE));
        SetTimer(main, TIMER_ID, TIMEOUT_MS, None);
        FRAME.store(make_frame() as isize, Ordering::SeqCst);
    }
    true
}

/// Ends picking and removes everything `start` installed. Safe to call twice.
pub fn stop() {
    // SAFETY: releases only what `start` created.
    unsafe {
        let hook = HOOK.swap(0, Ordering::SeqCst) as HHOOK;
        if !hook.is_null() {
            UnhookWindowsHookEx(hook);
        }
        let main = MAIN.load(Ordering::SeqCst) as HWND;
        if !main.is_null() && ACTIVE.load(Ordering::SeqCst) {
            UnregisterHotKey(main, ESC_ID);
            KillTimer(main, TIMER_ID);
        }
        let frame = FRAME.swap(0, Ordering::SeqCst) as HWND;
        if !frame.is_null() {
            DestroyWindow(frame);
        }
        LAST.store(0, Ordering::SeqCst);
    }
    ACTIVE.store(false, Ordering::SeqCst);
}

/// The deepest window under a screen point. The frame is click-through and layered, so the
/// system skips it.
pub fn control_at(pt: POINT) -> Option<HWND> {
    // SAFETY: plain query.
    let h = unsafe { WindowFromPoint(pt) };
    (!h.is_null()).then_some(h)
}

/// Moves the frame to the control under the pointer.
pub fn on_move(pt: POINT) {
    let frame = FRAME.load(Ordering::SeqCst) as HWND;
    if frame.is_null() {
        return;
    }
    let Some(h) = control_at(pt) else { return };
    if h as isize == LAST.swap(h as isize, Ordering::SeqCst) {
        return;
    }
    // SAFETY: shapes and moves our own frame window.
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        if GetWindowRect(h, &mut r) == 0 {
            return;
        }
        let (w, hgt) = ((r.right - r.left).max(2 * BORDER + 2), (r.bottom - r.top).max(2 * BORDER + 2));
        let outer = CreateRectRgn(0, 0, w, hgt);
        let inner = CreateRectRgn(BORDER, BORDER, w - BORDER, hgt - BORDER);
        CombineRgn(outer, outer, inner, RGN_DIFF);
        DeleteObject(inner as _);
        SetWindowPos(frame, HWND_TOPMOST, r.left, r.top, w, hgt, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        SetWindowRgn(frame, outer, 1); // the window owns `outer` from here
    }
}
