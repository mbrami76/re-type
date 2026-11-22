// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Tray icon, balloon notices and the context menu (REQ-UX-001, REQ-UX-002).

use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIcon, CreatePopupMenu, DestroyMenu, GetCursorPos, SetForegroundWindow,
    TrackPopupMenu, HICON, MF_CHECKED, MF_SEPARATOR, MF_STRING, TPM_RIGHTBUTTON, WM_APP,
};

pub const WM_TRAY: u32 = WM_APP + 1;
pub const ID_SETTINGS: usize = 1001;
pub const ID_PAUSE: usize = 1002;
pub const ID_QUIT: usize = 1003;
pub const ID_TYPE_NOW: usize = 1004;
pub const ID_PICK: usize = 1005;
pub const ID_HELP: usize = 1006;
pub const ID_ELEVATE: usize = 1007;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn copy_into(dst: &mut [u16], s: &str) {
    let w: Vec<u16> = s.encode_utf16().take(dst.len() - 1).collect();
    dst[..w.len()].copy_from_slice(&w);
    dst[w.len()] = 0;
}

/// 32x32 icon drawn in code: a blue rounded key-cap with a white arrow, so no resource file
/// is needed.
/// The embedded application icon (resource 1) at the size the shell asks for the tray.
pub fn app_icon(big: bool) -> HICON {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, LoadImageW, IMAGE_ICON, SM_CXICON, SM_CXSMICON};
    // SAFETY: resource 1 is the icon linked in by build.rs; a null result is handled by the caller.
    unsafe {
        let n = GetSystemMetrics(if big { SM_CXICON } else { SM_CXSMICON });
        LoadImageW(GetModuleHandleW(std::ptr::null()), 1 as *const u16, IMAGE_ICON, n, n, 0) as HICON
    }
}

fn make_icon() -> HICON {
    const N: usize = 32;
    let mut bgra = vec![0u8; N * N * 4];
    let inside = |x: i32, y: i32| {
        let (cx, cy) = (x.clamp(5, N as i32 - 6), y.clamp(5, N as i32 - 6));
        (x - cx).pow(2) + (y - cy).pow(2) <= 25
    };
    let arrow = |x: i32, y: i32| {
        let shaft = (14..=17).contains(&y) && (8..=21).contains(&x);
        let head = (x - 22).abs() + (y - 15).abs() <= 6 && x >= 18;
        shaft || head
    };
    for y in 0..N as i32 {
        for x in 0..N as i32 {
            let i = (y as usize * N + x as usize) * 4;
            let px: [u8; 4] = if !inside(x, y) {
                [0, 0, 0, 0]
            } else if arrow(x, y) {
                [255, 255, 255, 255]
            } else {
                [160, 90, 30, 255] // BGRA: blue
            };
            bgra[i..i + 4].copy_from_slice(&px);
        }
    }
    let and_mask = [0u8; N * N / 8];
    // SAFETY: both buffers have the sizes CreateIcon reads for a 32x32, 1-plane, 32-bpp icon.
    unsafe {
        CreateIcon(GetModuleHandleW(std::ptr::null()), N as i32, N as i32, 1, 32, and_mask.as_ptr(), bgra.as_ptr())
    }
}

fn data(hwnd: HWND) -> NOTIFYICONDATAW {
    // SAFETY: NOTIFYICONDATAW is plain data; all-zero is a valid empty value.
    let mut d: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    d.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    d.hWnd = hwnd;
    d.uID = 1;
    d
}

pub fn add(hwnd: HWND, tip: &str) {
    let mut d = data(hwnd);
    d.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    d.uCallbackMessage = WM_TRAY;
    let icon = app_icon(false);
    d.hIcon = if icon.is_null() { make_icon() } else { icon };
    copy_into(&mut d.szTip, tip);
    // SAFETY: `d` is fully initialised.
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &d);
        // version 4 reports keyboard use of the icon (Enter, menu key) and where to open the menu
        let mut v = data(hwnd);
        v.Anonymous.uVersion = 4;
        USES_V4.store(Shell_NotifyIconW(NIM_SETVERSION, &v) != 0, std::sync::atomic::Ordering::SeqCst);
    }
}

static USES_V4: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the tray icon sends version 4 messages (see `add`).
pub fn uses_v4() -> bool {
    USES_V4.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn remove(hwnd: HWND) {
    // SAFETY: see `add`.
    unsafe { Shell_NotifyIconW(NIM_DELETE, &data(hwnd)) };
}

pub fn notify(hwnd: HWND, title: &str, text: &str) {
    let mut d = data(hwnd);
    d.uFlags = NIF_INFO;
    copy_into(&mut d.szInfoTitle, title);
    copy_into(&mut d.szInfo, text);
    // SAFETY: see `add`.
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &d) };
}

/// `at` is where the icon is (keyboard use); `None` opens the menu at the mouse pointer.
/// `elevate` adds the command that restarts Re:Type with administrator rights (REQ-UX-010); it is
/// offered only while Re:Type runs without them.
pub fn show_menu(hwnd: HWND, paused: bool, pick: bool, elevate: bool, at: Option<(i32, i32)>) {
    // SAFETY: the menu is created, shown and destroyed here; strings outlive the calls.
    unsafe {
        let menu = CreatePopupMenu();
        AppendMenuW(menu, MF_STRING, ID_TYPE_NOW, wide(&crate::i18n::tr("&Type clipboard now")).as_ptr());
        if pick {
            AppendMenuW(menu, MF_STRING, ID_PICK, wide(&crate::i18n::tr("P&ick target and type…")).as_ptr());
        }
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, ID_SETTINGS, wide(&crate::i18n::tr("&Settings…")).as_ptr());
        AppendMenuW(menu, MF_STRING, ID_HELP, wide(&crate::i18n::tr("&Help")).as_ptr());
        AppendMenuW(menu, MF_STRING | if paused { MF_CHECKED } else { 0 }, ID_PAUSE, wide(&crate::i18n::tr("&Pause hotkeys")).as_ptr());
        if elevate {
            AppendMenuW(menu, MF_STRING, ID_ELEVATE, wide(&crate::i18n::tr("Restart as &administrator…")).as_ptr());
        }
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, ID_QUIT, wide(&crate::i18n::tr("&Quit")).as_ptr());
        let mut pt = POINT { x: 0, y: 0 };
        match at {
            Some((x, y)) => pt = POINT { x, y },
            None => {
                GetCursorPos(&mut pt);
            }
        }
        SetForegroundWindow(hwnd); // required, otherwise the menu does not close on outside click
        TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, std::ptr::null());
        DestroyMenu(menu);
    }
}
