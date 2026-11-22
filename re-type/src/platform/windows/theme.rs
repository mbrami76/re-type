// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Colour scheme of the settings window and the tray menu: light, dark, and the Windows
//! high-contrast colours (REQ-UX-007, REQ-ACC-004).
//!
//! The window is made of standard Win32 controls. Light mode and high contrast use the system
//! colours, so they follow whatever the user chose in Windows. Dark mode uses fixed colours and
//! the dark variants of the control themes that Windows ships (`DarkMode_Explorer`,
//! `DarkMode_CFD`); the tab strip is painted here because Windows has no dark tab theme.
//! The few undocumented uxtheme functions needed for dark menus are looked up at run time and
//! only used on builds that have them: if anything is missing the result is a light menu, never
//! an error.

use crate::config::Theme;
use std::cell::RefCell;
use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ};
use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows_sys::Win32::UI::Controls::{GetComboBoxInfo, SetWindowTheme, COMBOBOXINFO};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetFocus, IsWindowEnabled};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
const DWMWA_USE_IMMERSIVE_DARK_MODE_OLD: u32 = 19;
const FIRST_BUILD_WITH_DARK_MENUS: u32 = 18362;

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct Palette {
    pub dark: bool,
    pub bg: COLORREF,
    pub text: COLORREF,
    pub disabled: COLORREF,
    pub edit_bg: COLORREF,
    pub edit_text: COLORREF,
    pub border: COLORREF,
    pub tab_off: COLORREF,
    pub accent: COLORREF,
}

const fn rgb(r: u32, g: u32, b: u32) -> COLORREF {
    r | (g << 8) | (b << 16)
}

fn sys(index: SYS_COLOR_INDEX) -> COLORREF {
    // SAFETY: plain query.
    unsafe { GetSysColor(index) }
}

fn mix(a: COLORREF, b: COLORREF, percent_b: u32) -> COLORREF {
    let ch = |shift: u32| {
        let (x, y) = ((a >> shift) & 0xFF, (b >> shift) & 0xFF);
        (x * (100 - percent_b) + y * percent_b) / 100
    };
    rgb(ch(0), ch(8), ch(16))
}

impl Palette {
    pub fn system() -> Palette {
        let bg = sys(COLOR_BTNFACE);
        let hc = high_contrast();
        Palette {
            dark: false,
            bg,
            text: sys(COLOR_BTNTEXT),
            disabled: sys(COLOR_GRAYTEXT),
            edit_bg: sys(COLOR_WINDOW),
            edit_text: sys(COLOR_WINDOWTEXT),
            border: if hc { sys(COLOR_BTNTEXT) } else { sys(COLOR_BTNSHADOW) },
            tab_off: if hc { bg } else { mix(bg, rgb(0, 0, 0), 6) },
            accent: if hc { sys(COLOR_HIGHLIGHT) } else { sys(COLOR_HOTLIGHT) },
        }
    }

    pub fn dark() -> Palette {
        Palette {
            dark: true,
            bg: rgb(32, 32, 32),
            text: rgb(255, 255, 255),
            disabled: rgb(136, 136, 136),
            edit_bg: rgb(45, 45, 45),
            edit_text: rgb(255, 255, 255),
            border: rgb(100, 100, 100),
            tab_off: rgb(44, 44, 44),
            accent: rgb(76, 194, 255),
        }
    }
}

struct Active {
    palette: Palette,
    bg: HBRUSH,
    edit: HBRUSH,
}

thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
}

// ---- system settings ----------------------------------------------------------------------

/// Whether Windows high-contrast mode is on. It always wins over the dark choice.
pub fn high_contrast() -> bool {
    // SAFETY: HIGHCONTRASTW is plain data; the call fills it.
    unsafe {
        let mut hc: HIGHCONTRASTW = std::mem::zeroed();
        hc.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
        SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, &mut hc as *mut _ as *mut _, 0) != 0
            && hc.dwFlags & HCF_HIGHCONTRASTON != 0
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// "Choose your default app mode" in the Windows settings.
fn system_prefers_dark() -> bool {
    // SAFETY: reads one DWORD into a local.
    unsafe {
        let mut value: u32 = 1;
        let mut size = 4u32;
        let rc = RegGetValueW(
            HKEY_CURRENT_USER,
            wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize").as_ptr(),
            wide("AppsUseLightTheme").as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut value as *mut u32 as *mut _,
            &mut size,
        );
        rc == 0 && value == 0
    }
}

fn windows_build() -> u32 {
    // SAFETY: reads a short string into a local buffer of the size passed.
    unsafe {
        let mut buf = [0u16; 16];
        let mut size = std::mem::size_of_val(&buf) as u32;
        let rc = RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion").as_ptr(),
            wide("CurrentBuildNumber").as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr() as *mut _,
            &mut size,
        );
        if rc != 0 {
            return 0;
        }
        let n = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..n]).trim().parse().unwrap_or(0)
    }
}

/// Whether the window should be dark for this setting right now.
pub fn wants_dark(theme: Theme) -> bool {
    if high_contrast() {
        return false;
    }
    match theme {
        Theme::Dark => true,
        Theme::Light => false,
        Theme::System => system_prefers_dark(),
    }
}

// ---- dark menus ---------------------------------------------------------------------------

type FnSetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
type FnVoid = unsafe extern "system" fn();

/// Makes the tray menu follow the colour choice. Quietly does nothing on Windows builds that
/// cannot do it.
pub fn init_menus(theme: Theme) {
    if windows_build() < FIRST_BUILD_WITH_DARK_MENUS {
        return;
    }
    // SAFETY: the ordinals below are the long-standing uxtheme exports used by dark-mode aware
    // programs (SetPreferredAppMode 135, FlushMenuThemes 136, RefreshImmersiveColorPolicyState 104);
    // each is only called when it was found, with the signature it has had since build 18362.
    unsafe {
        let ux = LoadLibraryW(wide("uxtheme.dll").as_ptr());
        if ux.is_null() {
            return;
        }
        let mode = if high_contrast() {
            3 // force light: the system colours apply
        } else {
            match theme {
                Theme::System => 1, // allow dark
                Theme::Dark => 2,   // force dark
                Theme::Light => 3,  // force light
            }
        };
        if let Some(f) = GetProcAddress(ux, 135usize as *const u8) {
            let f: FnSetPreferredAppMode = std::mem::transmute(f);
            f(mode);
        }
        if let Some(f) = GetProcAddress(ux, 104usize as *const u8) {
            let f: FnVoid = std::mem::transmute(f);
            f();
        }
        if let Some(f) = GetProcAddress(ux, 136usize as *const u8) {
            let f: FnVoid = std::mem::transmute(f);
            f();
        }
    }
}

// ---- active palette and brushes -----------------------------------------------------------

/// Makes the palette for `theme` the active one (creating its brushes) and returns it.
pub fn activate(theme: Theme) -> Palette {
    let palette = if wants_dark(theme) { Palette::dark() } else { Palette::system() };
    ACTIVE.with(|a| {
        let mut a = a.borrow_mut();
        if let Some(old) = a.take() {
            // SAFETY: brushes created by `activate`; no window keeps them (WM_CTLCOLOR hands out
            // the current ones on every call).
            unsafe {
                DeleteObject(old.bg as HGDIOBJ);
                DeleteObject(old.edit as HGDIOBJ);
            }
        }
        // SAFETY: plain GDI object creation.
        let (bg, edit) = unsafe { (CreateSolidBrush(palette.bg), CreateSolidBrush(palette.edit_bg)) };
        *a = Some(Active { palette, bg, edit });
    });
    palette
}

pub fn palette() -> Palette {
    ACTIVE.with(|a| a.borrow().as_ref().map(|a| a.palette)).unwrap_or_else(Palette::system)
}

pub fn release() {
    ACTIVE.with(|a| {
        if let Some(old) = a.borrow_mut().take() {
            // SAFETY: see `activate`.
            unsafe {
                DeleteObject(old.bg as HGDIOBJ);
                DeleteObject(old.edit as HGDIOBJ);
            }
        }
    });
}

/// Fills `rc` with the window colour.
pub unsafe fn fill_bg(hdc: HDC, rc: &RECT) {
    let brush = ACTIVE.with(|a| a.borrow().as_ref().map(|a| a.bg));
    match brush {
        Some(b) => FillRect(hdc, rc, b),
        None => FillRect(hdc, rc, GetSysColorBrush(COLOR_BTNFACE)),
    };
}

/// Answer to a WM_CTLCOLOR* message: sets the text and background colours on `hdc` and returns
/// the brush. `as_field` is for read-only edit controls that are meant to look like a field.
pub unsafe fn control_colors(msg: u32, hdc: HDC, control: HWND, as_field: bool) -> LRESULT {
    ACTIVE.with(|a| {
        let a = a.borrow();
        let Some(a) = a.as_ref() else { return GetSysColorBrush(COLOR_BTNFACE) as LRESULT };
        let p = &a.palette;
        let field = as_field || msg == WM_CTLCOLOREDIT || msg == WM_CTLCOLORLISTBOX;
        let enabled = IsWindowEnabled(control) != 0;
        SetBkMode(hdc, OPAQUE as i32);
        if field {
            SetBkColor(hdc, p.edit_bg);
            SetTextColor(hdc, if enabled { p.edit_text } else { p.disabled });
            a.edit as LRESULT
        } else {
            SetBkColor(hdc, p.bg);
            SetTextColor(hdc, if enabled { p.text } else { p.disabled });
            a.bg as LRESULT
        }
    })
}

// ---- applying the scheme to windows -------------------------------------------------------

/// Title bar and non-client parts of a top-level window.
pub fn apply_window(hwnd: HWND, dark: bool) {
    // SAFETY: the attribute value is a BOOL; unsupported attributes just fail.
    unsafe {
        let v: i32 = i32::from(dark);
        for attr in [DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_USE_IMMERSIVE_DARK_MODE_OLD] {
            DwmSetWindowAttribute(hwnd, attr, &v as *const i32 as *const _, 4);
        }
        // uxtheme 133: AllowDarkModeForWindow
        if windows_build() >= FIRST_BUILD_WITH_DARK_MENUS {
            let ux = LoadLibraryW(wide("uxtheme.dll").as_ptr());
            if !ux.is_null() {
                if let Some(f) = GetProcAddress(ux, 133usize as *const u8) {
                    let f: unsafe extern "system" fn(HWND, i32) -> i32 = std::mem::transmute(f);
                    f(hwnd, i32::from(dark));
                }
            }
        }
    }
}

fn class_of(h: HWND) -> String {
    let mut buf = [0u16; 64];
    // SAFETY: valid buffer and length.
    let n = unsafe { GetClassNameW(h, buf.as_mut_ptr(), buf.len() as i32) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

/// Gives one control the dark or the normal Windows theme.
pub fn theme_control(h: HWND, dark: bool) {
    let class = class_of(h);
    if class.eq_ignore_ascii_case("button") {
        // SAFETY: plain style query.
        let style = unsafe { GetWindowLongW(h, GWL_STYLE) } as u32;
        if style & 0xF == BS_AUTOCHECKBOX as u32 {
            // Windows paints check box text in the dark theme in black; paint it here instead
            // (the control stays a real check box for the keyboard and for screen readers)
            // SAFETY: valid window of this thread.
            unsafe { SetWindowSubclass(h, Some(check_proc), CHECK_SUBCLASS_ID, 0) };
        }
    }
    let name = match class.to_ascii_lowercase().as_str() {
        "button" => "DarkMode_Explorer",
        "edit" | "combobox" => "DarkMode_CFD",
        "listbox" => "DarkMode_Explorer",
        _ => return,
    };
    // SAFETY: valid window and NUL-terminated strings; a null theme name restores the default.
    unsafe {
        if dark {
            SetWindowTheme(h, wide(name).as_ptr(), std::ptr::null());
        } else {
            SetWindowTheme(h, std::ptr::null(), std::ptr::null());
        }
        if class.eq_ignore_ascii_case("combobox") {
            let mut info: COMBOBOXINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<COMBOBOXINFO>() as u32;
            if GetComboBoxInfo(h, &mut info) != 0 && !info.hwndList.is_null() {
                if dark {
                    SetWindowTheme(info.hwndList, wide("DarkMode_Explorer").as_ptr(), std::ptr::null());
                } else {
                    SetWindowTheme(info.hwndList, std::ptr::null(), std::ptr::null());
                }
            }
        }
    }
}

unsafe extern "system" fn theme_child(h: HWND, dark: LPARAM) -> i32 {
    theme_control(h, dark != 0);
    1
}

/// Themes `root` and everything inside it.
pub fn apply_tree(root: HWND, dark: bool) {
    // SAFETY: enumerates the children of our own window.
    unsafe {
        theme_control(root, dark);
        EnumChildWindows(root, Some(theme_child), LPARAM::from(dark));
    }
}

// ---- check boxes in the dark scheme -------------------------------------------------------

const CHECK_SUBCLASS_ID: usize = 0x5265_5443;

unsafe extern "system" fn check_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
    let dark = palette().dark;
    match msg {
        WM_ERASEBKGND if dark => {
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            fill_bg(wp as HDC, &rc);
            1
        }
        WM_PAINT if dark => {
            paint_check(hwnd);
            0
        }
        WM_SETFOCUS | WM_KILLFOCUS | WM_ENABLE | BM_SETCHECK if dark => {
            let r = DefSubclassProc(hwnd, msg, wp, lp);
            InvalidateRect(hwnd, std::ptr::null(), 0);
            r
        }
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, Some(check_proc), CHECK_SUBCLASS_ID);
            DefSubclassProc(hwnd, msg, wp, lp)
        }
        _ => DefSubclassProc(hwnd, msg, wp, lp),
    }
}

unsafe fn paint_check(hwnd: HWND) {
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let p = palette();
    let dpi = GetDpiForWindow(hwnd).max(96) as i32;
    let dp = |v: i32| (v * dpi + 48) / 96;
    let mut rc: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rc);
    fill_bg(hdc, &rc);
    let font = SendMessageW(hwnd, WM_GETFONT, 0, 0) as HGDIOBJ;
    let old_font = if font.is_null() { std::ptr::null_mut() } else { SelectObject(hdc, font) };
    let enabled = IsWindowEnabled(hwnd) != 0;
    let ink = if enabled { p.text } else { p.disabled };
    // the box, centred on the first line of text
    let mut tm: TEXTMETRICW = std::mem::zeroed();
    GetTextMetricsW(hdc, &mut tm);
    let side = dp(13);
    let top = rc.top + dp(1) + (tm.tmHeight - side) / 2;
    let bx = RECT { left: rc.left + dp(1), top, right: rc.left + dp(1) + side, bottom: top + side };
    let fill = CreateSolidBrush(p.edit_bg);
    let frame = CreateSolidBrush(ink);
    FillRect(hdc, &bx, fill);
    FrameRect(hdc, &bx, frame);
    DeleteObject(fill as HGDIOBJ);
    DeleteObject(frame as HGDIOBJ);
    if SendMessageW(hwnd, BM_GETCHECK, 0, 0) == 1 {
        let pen = CreatePen(PS_SOLID, dp(2).max(1), ink);
        let old = SelectObject(hdc, pen as HGDIOBJ);
        MoveToEx(hdc, bx.left + dp(3), bx.top + dp(7), std::ptr::null_mut());
        LineTo(hdc, bx.left + dp(5), bx.top + dp(9));
        LineTo(hdc, bx.left + dp(10), bx.top + dp(4));
        SelectObject(hdc, old);
        DeleteObject(pen as HGDIOBJ);
    }
    // the text, with the access-key underline
    let mut buf = [0u16; 256];
    let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32).max(0);
    let mut tr = RECT { left: rc.left + dp(20), top: rc.top + dp(1), right: rc.right, bottom: rc.bottom };
    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(hdc, ink);
    DrawTextW(hdc, buf.as_ptr(), n, &mut tr, DT_LEFT | DT_WORDBREAK | DT_EDITCONTROL);
    // keyboard focus
    if GetFocus() == hwnd {
        let mut calc = tr;
        DrawTextW(hdc, buf.as_ptr(), n, &mut calc, DT_LEFT | DT_WORDBREAK | DT_EDITCONTROL | DT_CALCRECT);
        let fr = RECT { left: tr.left - dp(2), top: tr.top - dp(1), right: calc.right + dp(2), bottom: calc.bottom + dp(1) };
        SetTextColor(hdc, p.text);
        SetBkColor(hdc, p.bg);
        DrawFocusRect(hdc, &fr);
    }
    if !old_font.is_null() {
        SelectObject(hdc, old_font);
    }
    EndPaint(hwnd, &ps);
}
