// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Settings window (REQ-FUNC-002, REQ-FUNC-003, REQ-FUNC-004, REQ-FUNC-005, REQ-FUNC-006,
//! REQ-FUNC-007, REQ-FUNC-008).
//!
//! Standard Win32 controls on tab pages. Nothing has a fixed position: every text is measured
//! with the window's font and DPI, wrapped to the available width, and the controls are laid out
//! again whenever the window is resized or moved to a monitor with another scale. The window has
//! a minimum size computed from its content, so no text is ever cut off or covered.

use super::richfield::{self, Kind};
use super::richhelp;
use super::theme;
use super::tray::wide;
use crate::actions::{NewlineMode, TabMode};
use crate::config::{self, Config, Theme};
use crate::hotkey::{self, Hotkey};
use crate::safechars::SafeMode;
use crate::typing::FilesMode;
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{InitCommonControlsEx, ICC_TAB_CLASSES, INITCOMMONCONTROLSEX};
use windows_sys::Win32::UI::Accessibility::NotifyWinEvent;
use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow, SystemParametersInfoForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// Control constants not exported by the crate features in use.
const EM_SETMARGINS: u32 = 0xD3;
const OBJID_CLIENT_ID: i32 = -4;
const WM_CHANGEUISTATE: u32 = 0x127;
const WM_THEMECHANGED: u32 = 0x31A;
const VK_TAB_KEY: u16 = 0x09;
const VK_PRIOR_KEY: u16 = 0x21;
const VK_NEXT_KEY: u16 = 0x22;
const EVENT_OBJECT_VALUECHANGE_: u32 = 0x800E;
const EVENT_OBJECT_NAMECHANGE_: u32 = 0x800C;
const EVENT_OBJECT_LIVEREGIONCHANGED_: u32 = 0x8019;
const EM_SETSEL: u32 = 0xB1;
const LB_ADDSTRING_: u32 = 0x0180;

const CLASS: &str = "ReTypeSettings";
const PAGE_CLASS: &str = "ReTypeSettingsPage";

// control ids: fields
const ID_HOTKEY: i32 = 2001;
const ID_CHECK: i32 = 2002;
const ID_FIND: i32 = 2003;
const ID_STATUS: i32 = 2004;
const ID_HOTKEY_NOTE: i32 = 2005;
const ID_PICK_ON: i32 = 2006;
const ID_PICK_HK: i32 = 2007;
const ID_PICK_CHECK: i32 = 2008;
const ID_PICK_FIND: i32 = 2009;
const ID_PICK_STATUS: i32 = 2019;
const ID_PICK_NOTE: i32 = 2024;
const ID_INITIAL: i32 = 2010;
const ID_CHARDELAY: i32 = 2011;
const ID_RELEASE: i32 = 2012;
const ID_MAXCHARS: i32 = 2013;
const ID_NEWLINE: i32 = 2014;
const ID_TAB: i32 = 2015;
const ID_TABSPACES: i32 = 2016;
const ID_STRIP: i32 = 2017;
const ID_ESC_NOTE: i32 = 2018;
const ID_SAFE: i32 = 2040;
const ID_SAFE_NOTE: i32 = 2041;
const ID_FILES: i32 = 2042;
const ID_FILES_NOTE: i32 = 2043;
const ID_CLEAR: i32 = 2045;
const ID_THEME: i32 = 2046;
const ID_THEME_NOTE: i32 = 2047;
const ID_LANG: i32 = 2058;
const ID_LANG_NOTE: i32 = 2059;
const ID_LANG_WARN: i32 = 2060;
const ID_LANG_ISSUES: i32 = 2061;
const ID_LISTEN: i32 = 2062;
const ID_PICK_LISTEN: i32 = 2063;
const ID_TLISTEN: i32 = 2064;
const ID_FREE_KEYS: i32 = 2065;
const ID_AUTOSTART: i32 = 2050;
const ID_AUTOSTART_NOTE: i32 = 2051;
const ID_ELEV: i32 = 2052;
const ID_ELEV_NOTE: i32 = 2053;
const ID_STATE_NOTE: i32 = 2054;
const ID_HELPTEXT: i32 = 2055;
const ID_ABOUT_TEXT: i32 = 2070;
const ID_ABOUT_WEB: i32 = 2071;
const ID_ABOUT_SRC: i32 = 2072;
const ID_SAVE: i32 = 2020;
const ID_CLOSE: i32 = 2021;
const ID_FOOTER: i32 = 2022;
const ID_FILE: i32 = 2026;
const ID_TAB_CONTROL: i32 = 2023;
const ID_LIST: i32 = 2030;
const ID_ADD: i32 = 2031;
const ID_REMOVE: i32 = 2032;
const ID_TNAME: i32 = 2033;
const ID_TENABLED: i32 = 2034;
const ID_THOTKEY: i32 = 2035;
const ID_TCHECK: i32 = 2036;
const ID_TFIND: i32 = 2037;
const ID_TSTATUS: i32 = 2038;
const ID_TPATTERN: i32 = 2039;
const ID_THELP: i32 = 2044;
// Texts with access keys (the letter after & is the Alt shortcut). Within one page the letters
// differ from each other and from the footer buttons (S, C).
const T_HOTKEY: &str = "&Hotkey";
const T_PICK_ON: &str = "&Enable \"Pick target and type\" (tray command and optional shortcut)";
const T_PICK_HK: &str = "Shortcut ke&y (optional)";
const T_CHECK: &str = "Chec&k hotkey";
const T_FIND: &str = "&Find free hotkey";
const T_INITIAL: &str = "&Delay before typing (ms)";
const T_CHARDELAY: &str = "Delay &between keystrokes (ms)";
const T_RELEASE: &str = "Max wait for key &release (ms)";
const T_MAXCHARS: &str = "&Longest clipboard text (characters)";
const T_NEWLINE: &str = "&Newline is typed as";
const T_TAB: &str = "Tab is typed &as";
const T_TABSPACES: &str = "Spaces per &tab";
const T_STRIP: &str = "Dro&p trailing line breaks of the clipboard text";
const T_SAFE: &str = "Spec&ial characters";
const T_CLEAR: &str = "Clear the clipboard &after it was typed successfully";
const T_FILES: &str = "Copied &files are typed as";
const T_THEME: &str = "&Colour scheme";
const T_LANG: &str = "&Language";
const T_LISTEN: &str = "&Listen";
const T_RELEASE_KEYS: &str = "&Release Re:Type shortcuts";
const T_RESTORE_KEYS: &str = "&Restore Re:Type shortcuts";
const T_LANG_ISSUES: &str = "Report a &problem or translation error";
const T_LANG_WARN: &str = "This translation has not been checked by a native speaker and may contain mistakes. Please report errors, or help to improve it, on the project's issue page.";
const T_AUTOSTART: &str = "&Start Re:Type when I sign in to Windows";
const T_ELEV: &str = "Always run Re:Type as &administrator";
const T_ABOUT_WEB: &str = "&Website";
const T_ABOUT_SRC: &str = "Source &code";
const T_ABOUTTEXT: &str = "About text (use the arrow keys, Page Up and Page Down to scroll)";
const T_HELPTEXT: &str = "Help text (use the arrow keys, Page Up and Page Down to scroll)";
const T_LIST: &str = "T&emplates";
const T_ADD: &str = "&Add";
const T_REMOVE: &str = "&Remove";
const T_TNAME: &str = "&Name";
const T_TENABLED: &str = "Ena&bled";
const T_TPATTERN: &str = "&Pattern (typed as written; {tokens} are keys)";
const T_THELP: &str = "Pattern tokens";
const T_SAVE: &str = "&Save and apply";
const T_CLOSE: &str = "&Close";
const CHECK_HINT: &str = "Hotkey check: not run yet. Choose \"Check hotkey\" to test the combination.";

/// Removes the access-key markers: "&Save && apply" becomes "Save & apply".
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            if chars.peek() == Some(&'&') {
                out.push('&');
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

// control ids: labels (id = field id + 1000)
const LBL: i32 = 1000;

const HELP: &str = "{CLIPBOARD}  the clipboard text (every template needs it)\r\n\
{TAB} {SHIFT+TAB} {ENTER}   move between fields, confirm\r\n\
{UP} {DOWN} {LEFT} {RIGHT} {HOME} {END}   move inside a field\r\n\
{BACKSPACE} {DEL}   delete a character\r\n\
{TAB 3}   repeat a key 3 times (1 to 5)\r\n\
{DELAY 200}   wait 200 ms (up to 1000)\r\n\
{{ and }}   literal braces\r\n\
Any other text is typed as written. No shortcuts, function keys or Esc.\r\n\
Example: ID-{CLIPBOARD}{TAB}";

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Hotkey,
    Typing,
    Clipboard,
    Appearance,
    Startup,
    Templates,
    Help,
    About,
}

const PAGES: &[(Page, &str)] =
    &[
    (Page::Hotkey, "Hotkeys"),
    (Page::Typing, "Typing"),
    (Page::Clipboard, "Clipboard"),
    (Page::Appearance, "Appearance"),
    (Page::Startup, "Startup"),
    (Page::Templates, "Templates"),
    (Page::Help, "Help"),
    (Page::About, "About"),
];
/// Index of the Help page; About is the last page.
const HELP_PAGE: usize = PAGES.len() - 2;

struct State {
    hwnd: HWND,
    tab: HWND,
    pages: Vec<HWND>,
    font: HFONT,
    dpi: u32,
    cfg: Config,
    /// Smallest client size that shows everything, in pixels at the current DPI.
    min_client: (i32, i32),
    sel: Option<usize>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static LANGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static LAST_TAB: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().and_then(|mut s| s.as_mut().map(f)))
}

// ---- control access -----------------------------------------------------------------------

/// Finds a control by id on the main window or on any tab page.
fn item(_main: HWND, id: i32) -> HWND {
    // SAFETY: plain lookups.
    unsafe {
        let main = with_state(|s| (s.hwnd, s.pages.clone()));
        let Some((hwnd, pages)) = main else { return std::ptr::null_mut() };
        let h = GetDlgItem(hwnd, id);
        if !h.is_null() {
            return h;
        }
        for p in pages {
            let h = GetDlgItem(p, id);
            if !h.is_null() {
                return h;
            }
        }
        std::ptr::null_mut()
    }
}

fn set_text(hwnd: HWND, id: i32, text: &str) {
    // SAFETY: NUL-terminated string outlives the call.
    unsafe { SetWindowTextW(item(hwnd, id), wide(text).as_ptr()) };
}

/// Sets a status text and tells assistive technology that it changed, so that a screen reader
/// can speak it without the focus moving (REQ-ACC-003).
fn set_status(hwnd: HWND, id: i32, text: &str) {
    set_text(hwnd, id, text);
    let h = item(hwnd, id);
    if !h.is_null() {
        // SAFETY: plain notifications about our own window.
        unsafe {
            NotifyWinEvent(EVENT_OBJECT_VALUECHANGE_, h, OBJID_CLIENT_ID, 0);
            NotifyWinEvent(EVENT_OBJECT_NAMECHANGE_, h, OBJID_CLIENT_ID, 0);
            NotifyWinEvent(EVENT_OBJECT_LIVEREGIONCHANGED_, h, OBJID_CLIENT_ID, 0);
            // NVDA does not speak a change of a control that has no focus (tested), so while a
            // screen reader is running the focus goes to the result, which is then read out.
            // Shift+Tab goes back to the button.
            let mut active: i32 = 0;
            if SystemParametersInfoW(0x46 /* SPI_GETSCREENREADER */, 0, &mut active as *mut i32 as *mut _, 0) != 0 && active != 0 {
                windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(h);
            }
        }
    }
}

fn get_text(hwnd: HWND, id: i32) -> String {
    let t = text_of(item(hwnd, id));
    if RICH_FIELDS.iter().any(|(i, _)| *i == id) {
        // a rich-text control separates lines by a lone CR
        return t.replace("\r\n", "\r").replace('\r', "\r\n");
    }
    t
}

fn text_of(h: HWND) -> String {
    // SAFETY: the buffer has the length passed (+1 for the terminator).
    unsafe {
        let n = GetWindowTextLengthW(h).max(0) as usize;
        let mut buf = vec![0u16; n + 1];
        let got = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&buf[..got])
    }
}

fn set_check(hwnd: HWND, id: i32, on: bool) {
    // SAFETY: plain message.
    unsafe { SendMessageW(item(hwnd, id), BM_SETCHECK, usize::from(on), 0) };
}

fn get_check(hwnd: HWND, id: i32) -> bool {
    // SAFETY: plain message.
    unsafe { SendMessageW(item(hwnd, id), BM_GETCHECK, 0, 0) == 1 }
}

fn enable(hwnd: HWND, id: i32, on: bool) {
    // SAFETY: plain call.
    unsafe { EnableWindow(item(hwnd, id), i32::from(on)) };
}

fn combo_select(hwnd: HWND, id: i32, index: usize) {
    // SAFETY: plain message.
    unsafe { SendMessageW(item(hwnd, id), CB_SETCURSEL, index, 0) };
}

fn combo_index(hwnd: HWND, id: i32) -> usize {
    // SAFETY: plain message.
    unsafe { SendMessageW(item(hwnd, id), CB_GETCURSEL, 0, 0).max(0) as usize }
}

// ---- measuring and layout -----------------------------------------------------------------

struct Metrics {
    hdc: HDC,
    hwnd: HWND,
    old_font: HGDIOBJ,
    dpi: i32,
}

impl Metrics {
    fn new(hwnd: HWND, font: HFONT, dpi: u32) -> Metrics {
        // SAFETY: the DC is released in Drop; the font stays valid for the metrics' lifetime.
        unsafe {
            let hdc = GetDC(hwnd);
            let old_font = SelectObject(hdc, font as HGDIOBJ);
            Metrics { hdc, hwnd, old_font, dpi: dpi as i32 }
        }
    }
    /// Device-independent units to pixels.
    fn dp(&self, v: i32) -> i32 {
        (v * self.dpi + 48) / 96
    }
    fn text_width(&self, text: &str) -> i32 {
        let w = wide(text);
        let mut size = SIZE { cx: 0, cy: 0 };
        // SAFETY: valid DC and string.
        unsafe { GetTextExtentPoint32W(self.hdc, w.as_ptr(), (w.len() - 1) as i32, &mut size) };
        size.cx
    }
    /// Height of `text` wrapped to `width` pixels.
    fn text_height(&self, text: &str, width: i32) -> i32 {
        let w = wide(text);
        let mut r = RECT { left: 0, top: 0, right: width.max(10), bottom: 0 };
        // SAFETY: valid DC and string.
        unsafe { DrawTextW(self.hdc, w.as_ptr(), -1, &mut r, DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX | DT_EDITCONTROL) };
        r.bottom - r.top
    }
    fn line_height(&self) -> i32 {
        self.text_height("Ag", 10_000)
    }
}

impl Drop for Metrics {
    fn drop(&mut self) {
        // SAFETY: restores and releases what `new` selected and acquired.
        unsafe {
            SelectObject(self.hdc, self.old_font);
            ReleaseDC(self.hwnd, self.hdc);
        }
    }
}

/// Vertical flow layout inside one page.
struct Flow<'a> {
    m: &'a Metrics,
    page: HWND,
    x: i32,
    y: i32,
    w: i32,
    label_w: i32,
    stacked: bool,
    dry: bool,
}

impl<'a> Flow<'a> {
    fn new(m: &'a Metrics, page: HWND, width: i32, label_texts: &[&str], dry: bool) -> Flow<'a> {
        let pad = m.dp(12);
        let w = (width - 2 * pad).max(m.dp(200));
        let widest = label_texts.iter().map(|t| m.text_width(&plain(&crate::i18n::tr(t)))).max().unwrap_or(0) + m.dp(12);
        let stacked = w < m.dp(430) || widest > w * 45 / 100;
        Flow { m, page, x: pad, y: pad, w, label_w: widest, stacked, dry }
    }

    fn put(&self, id: i32, x: i32, y: i32, w: i32, h: i32) {
        if self.dry {
            return;
        }
        // SAFETY: moves a child control of the page.
        unsafe {
            let h_ctl = GetDlgItem(self.page, id);
            if !h_ctl.is_null() {
                // a drop-down list box is part of the combo box's height: without room for it the
                // list opens empty. Windows trims the visible part to one line by itself.
                let mut class = [0u16; 16];
                let n = GetClassNameW(h_ctl, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
                let is_combo = String::from_utf16_lossy(&class[..n]).eq_ignore_ascii_case("ComboBox");
                let h = if is_combo { h + self.m.dp(200) } else { h };
                SetWindowPos(h_ctl, std::ptr::null_mut(), x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
            }
        }
    }

    fn text_of_id(&self, id: i32) -> String {
        // SAFETY: lookup on the page.
        unsafe { text_of(GetDlgItem(self.page, id)) }
    }

    fn gap(&mut self, dp: i32) {
        self.y += self.m.dp(dp);
    }

    fn field_h(&self) -> i32 {
        self.m.line_height() + self.m.dp(10)
    }

    /// Label and one field. The field is at most `max_dp` wide.
    fn row(&mut self, field: i32, max_dp: i32, height: Option<i32>) {
        let h = height.unwrap_or_else(|| self.field_h());
        let label_h = self.m.text_height(&plain(&self.text_of_id(field + LBL)), self.w);
        if self.stacked {
            self.put(field + LBL, self.x, self.y, self.w, label_h);
            self.y += label_h + self.m.dp(3);
            self.put(field, self.x, self.y, self.w.min(self.m.dp(max_dp)), h);
        } else {
            let fw = (self.w - self.label_w).min(self.m.dp(max_dp)).max(self.m.dp(60));
            self.put(field + LBL, self.x, self.y + (h - self.m.line_height()) / 2, self.label_w, label_h);
            self.put(field, self.x + self.label_w, self.y, fw, h);
        }
        self.y += h + self.m.dp(8);
    }

    /// X coordinate where fields start (so buttons and notes can line up with them).
    fn field_x(&self) -> i32 {
        if self.stacked {
            self.x
        } else {
            self.x + self.label_w
        }
    }

    /// A row of buttons, wrapped to the next line when they do not fit.
    fn buttons(&mut self, ids: &[i32]) {
        let bh = self.m.line_height() + self.m.dp(12);
        let gap = self.m.dp(8);
        let mut x = self.field_x();
        let right = self.x + self.w;
        let mut y = self.y;
        for &id in ids {
            let bw = (self.m.text_width(&plain(&self.text_of_id(id))) + self.m.dp(28)).max(self.m.dp(75));
            if x + bw > right && x > self.field_x() {
                x = self.field_x();
                y += bh + gap;
            }
            self.put(id, x, y, bw, bh);
            x += bw + gap;
        }
        self.y = y + bh + self.m.dp(8);
    }

    /// Wrapped text across the field column.
    fn note(&mut self, id: i32, from_field_column: bool) {
        let x = if from_field_column { self.field_x() } else { self.x };
        let w = self.x + self.w - x;
        // read-only edit controls: room for a last line that wraps differently than measured
        let h = self.m.text_height(&self.text_of_id(id), w).max(self.m.line_height()) + self.m.dp(5);
        self.put(id, x, self.y, w, h);
        self.y += h + self.m.dp(8);
    }

    fn check(&mut self, id: i32) {
        let box_w = self.m.dp(24);
        let h = self.m.text_height(&plain(&self.text_of_id(id)), self.w - box_w).max(self.m.dp(20));
        self.put(id, self.x, self.y, self.w, h + self.m.dp(2));
        self.y += h + self.m.dp(10);
    }

    fn height(&self) -> i32 {
        self.y + self.m.dp(4)
    }
}

/// Lays out one page for a page width of `width` pixels and returns the height it needs.
fn layout_page(m: &Metrics, page: Page, hwnd: HWND, width: i32, height: i32, dry: bool) -> i32 {
    match page {
        Page::Hotkey => {
            let mut f = Flow::new(m, hwnd, width, &[T_HOTKEY, T_PICK_HK], dry);
            f.row(ID_HOTKEY, 300, None);
            f.buttons(&[ID_CHECK, ID_FIND, ID_LISTEN, ID_FREE_KEYS]);
            f.note(ID_STATUS, true);
            f.gap(6);
            f.note(ID_HOTKEY_NOTE, false);
            f.gap(10);
            f.check(ID_PICK_ON);
            f.row(ID_PICK_HK, 300, None);
            f.buttons(&[ID_PICK_CHECK, ID_PICK_FIND, ID_PICK_LISTEN]);
            f.note(ID_PICK_STATUS, true);
            f.gap(6);
            f.note(ID_PICK_NOTE, false);
            f.height()
        }
        Page::Typing => {
            let labels = [T_INITIAL, T_CHARDELAY, T_RELEASE, T_MAXCHARS, T_NEWLINE, T_TAB, T_TABSPACES];
            let mut f = Flow::new(m, hwnd, width, &labels, dry);
            f.row(ID_INITIAL, 110, None);
            f.row(ID_CHARDELAY, 110, None);
            f.row(ID_RELEASE, 110, None);
            f.row(ID_MAXCHARS, 110, None);
            f.row(ID_NEWLINE, 200, Some(m.dp(300)).map(|_| f.field_h()));
            f.row(ID_TAB, 200, None);
            f.row(ID_TABSPACES, 110, None);
            f.check(ID_STRIP);
            f.note(ID_ESC_NOTE, false);
            f.height()
        }
        Page::Clipboard => {
            let mut f = Flow::new(m, hwnd, width, &[T_SAFE, T_FILES], dry);
            f.row(ID_SAFE, 340, None);
            f.note(ID_SAFE_NOTE, true);
            f.gap(6);
            f.row(ID_FILES, 340, None);
            f.note(ID_FILES_NOTE, true);
            f.gap(6);
            f.check(ID_CLEAR);
            f.height()
        }
        Page::Appearance => {
            let mut f = Flow::new(m, hwnd, width, &[T_THEME, T_LANG], dry);
            f.row(ID_THEME, 260, None);
            f.note(ID_THEME_NOTE, true);
            f.row(ID_LANG, 260, None);
            f.note(ID_LANG_NOTE, true);
            f.height()
        }
        Page::Startup => {
            let mut f = Flow::new(m, hwnd, width, &[], dry);
            f.check(ID_AUTOSTART);
            f.note(ID_AUTOSTART_NOTE, false);
            f.gap(6);
            f.check(ID_ELEV);
            f.note(ID_ELEV_NOTE, false);
            f.note(ID_STATE_NOTE, false);
            f.height()
        }
        Page::About => {
            let pad = m.dp(12);
            let mut f = Flow::new(m, hwnd, width, &[], dry);
            f.buttons(&[ID_ABOUT_WEB, ID_ABOUT_SRC, ID_LANG_ISSUES]);
            if language_unchecked() {
                f.note(ID_LANG_WARN, false);
            }
            let label_h = m.text_height(&crate::i18n::tr(T_ABOUTTEXT), (width - 2 * pad).max(m.dp(200))).max(m.line_height());
            let min_h = f.y + label_h + m.line_height() * 8 + pad;
            if !dry {
                let w = (width - 2 * pad).max(m.dp(200));
                f.put(ID_ABOUT_TEXT + LBL, pad, f.y, w, label_h);
                let top = f.y + label_h + m.dp(3);
                f.put(ID_ABOUT_TEXT, pad, top, w, (height - top - pad).max(m.line_height() * 6));
            }
            min_h
        }
        Page::Help => {
            let pad = m.dp(12);
            let label_h = m.text_height(&crate::i18n::tr(T_HELPTEXT), (width - 2 * pad).max(m.dp(200))).max(m.line_height());
            let min_h = m.line_height() * 8 + 2 * pad + label_h;
            if !dry {
                // SAFETY: moves child controls of the page.
                unsafe {
                    let w = (width - 2 * pad).max(m.dp(200));
                    SetWindowPos(GetDlgItem(hwnd, ID_HELPTEXT + LBL), std::ptr::null_mut(), pad, pad, w, label_h, SWP_NOZORDER | SWP_NOACTIVATE);
                    SetWindowPos(
                        GetDlgItem(hwnd, ID_HELPTEXT),
                        std::ptr::null_mut(),
                        pad,
                        pad + label_h + m.dp(3),
                        w,
                        (height - 2 * pad - label_h - m.dp(3)).max(m.line_height() * 6),
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            min_h
        }
        Page::Templates => {
            let pad = m.dp(12);
            let gap = m.dp(12);
            let usable = (width - 2 * pad).max(m.dp(300));
            let list_w = (usable * 30 / 100).clamp(m.dp(140), m.dp(240));
            let btn_h = m.line_height() + m.dp(12);
            let list_label_h = m.line_height() + m.dp(3);
            let list_h = (height - 2 * pad - btn_h - m.dp(8) - list_label_h).max(m.dp(120));
            let put = |id: i32, x: i32, y: i32, w: i32, h: i32| {
                if !dry {
                    // SAFETY: moves a child control of the page.
                    unsafe {
                        let c = GetDlgItem(hwnd, id);
                        if !c.is_null() {
                            SetWindowPos(c, std::ptr::null_mut(), x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
                        }
                    }
                }
            };
            put(ID_LIST + LBL, pad, pad, list_w, m.line_height());
            put(ID_LIST, pad, pad + list_label_h, list_w, list_h);
            put(ID_ADD, pad, pad + list_label_h + list_h + m.dp(8), (list_w - m.dp(8)) / 2, btn_h);
            put(ID_REMOVE, pad + (list_w + m.dp(8)) / 2, pad + list_label_h + list_h + m.dp(8), (list_w - m.dp(8)) / 2, btn_h);
            // detail column
            let dx = pad + list_w + gap;
            let dw = (width - dx - pad).max(m.dp(200));
            let mut f = Flow::new(m, hwnd, width, &[], dry);
            f.x = dx;
            f.w = dw;
            f.stacked = true;
            f.row(ID_TNAME, 400, None);
            f.check(ID_TENABLED);
            f.row(ID_THOTKEY, 300, None);
            f.buttons(&[ID_TCHECK, ID_TFIND, ID_TLISTEN]);
            f.note(ID_TSTATUS, false);
            let pattern_h = m.line_height() * 4 + m.dp(14);
            f.row(ID_TPATTERN, 600, Some(pattern_h));
            // the help text fills what is left, with a minimum, and scrolls if it must
            let help_label_h = m.text_height(&plain(&f.text_of_id(ID_THELP + LBL)), dw);
            f.put(ID_THELP + LBL, f.x, f.y, dw, help_label_h);
            f.y += help_label_h + m.dp(3);
            let help_min = m.line_height() * 6 + m.dp(10);
            let help_h = (height - pad - f.y).max(help_min);
            f.put(ID_THELP, f.x, f.y, dw, help_h);
            f.y += help_h;
            (f.y + pad).max(pad + list_label_h + list_h + btn_h + m.dp(8) + pad)
        }
    }
}

// ---- creating the window ------------------------------------------------------------------

fn create(parent: HWND, class: &str, text: &str, style: u32, ex: u32, id: i32) -> HWND {
    // SAFETY: strings outlive the call; `parent` is one of our windows.
    unsafe {
        CreateWindowExW(
            ex,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            10,
            10,
            parent,
            id as usize as _,
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    }
}

/// A label. Its access key moves the focus to the control created right after it.
fn label(p: HWND, id: i32, text: &str) {
    create(p, "STATIC", &crate::i18n::tr(text), 0, 0, id + LBL);
}
/// Explanatory or status text. A read-only edit control, not a static one, so that it can be
/// reached with the Tab key and read by screen readers like any other control (REQ-ACC-002).
fn note(p: HWND, id: i32, text: &str) {
    let h = create(p, "EDIT", text, WS_TABSTOP | (ES_MULTILINE | ES_READONLY) as u32, 0, id);
    // SAFETY: plain message; no margins so the text wraps like the measured text.
    unsafe { SendMessageW(h, EM_SETMARGINS, 3, 0) };
}
/// A shortcut or pattern field that shows what is recognised (see `richfield`).
fn rich(p: HWND, id: i32, multiline: bool) {
    let style = if multiline {
        WS_TABSTOP | WS_VSCROLL | (ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32
    } else {
        WS_TABSTOP | ES_AUTOHSCROLL as u32
    };
    let h = create(p, richfield::CLASS, "", style, WS_EX_CLIENTEDGE, id);
    if let Some((_, kind)) = RICH_FIELDS.iter().find(|(i, _)| *i == id) {
        richfield::setup(h, *kind);
    }
}

/// The fields that are judged while the user types.
const RICH_FIELDS: [(i32, Kind); 4] =
    [(ID_HOTKEY, Kind::Shortcut), (ID_PICK_HK, Kind::Shortcut), (ID_THOTKEY, Kind::Shortcut), (ID_TPATTERN, Kind::Pattern)];

fn edit(p: HWND, id: i32, number: bool) {
    let style = WS_TABSTOP | ES_AUTOHSCROLL as u32 | if number { ES_NUMBER as u32 } else { 0 };
    create(p, "EDIT", "", style, WS_EX_CLIENTEDGE, id);
}
fn button(p: HWND, id: i32, text: &str) {
    create(p, "BUTTON", &crate::i18n::tr(text), WS_TABSTOP | BS_PUSHBUTTON as u32, 0, id);
}
fn check(p: HWND, id: i32, text: &str) {
    create(p, "BUTTON", &crate::i18n::tr(text), WS_TABSTOP | BS_AUTOCHECKBOX as u32 | BS_MULTILINE as u32, 0, id);
}
fn combo(p: HWND, id: i32, options: &[String]) {
    let c = create(p, "COMBOBOX", "", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32, 0, id);
    for o in options {
        // SAFETY: plain message with a NUL-terminated string.
        unsafe { SendMessageW(c, CB_ADDSTRING, 0, wide(o).as_ptr() as LPARAM) };
    }
}

fn build_page(page: Page, p: HWND) {
    match page {
        Page::Hotkey => {
            label(p, ID_HOTKEY, T_HOTKEY);
            rich(p, ID_HOTKEY, false);
            button(p, ID_CHECK, T_CHECK);
            button(p, ID_FIND, T_FIND);
            button(p, ID_LISTEN, T_LISTEN);
            button(p, ID_FREE_KEYS, T_RELEASE_KEYS);
            note(p, ID_STATUS, &crate::i18n::tr(CHECK_HINT));
            note(
                p,
                ID_HOTKEY_NOTE,
                &crate::i18n::tr("Type a combination such as ctrl+alt+shift+v. \"Check\" asks Windows whether the combination is free. \
Windows can only report hotkeys that other programs registered globally; a shortcut handled inside a program \
(for example Ctrl+Shift+V in a browser) cannot be detected."),
            );
            check(p, ID_PICK_ON, T_PICK_ON);
            label(p, ID_PICK_HK, T_PICK_HK);
            rich(p, ID_PICK_HK, false);
            button(p, ID_PICK_CHECK, T_CHECK);
            button(p, ID_PICK_FIND, T_FIND);
            button(p, ID_PICK_LISTEN, T_LISTEN);
            note(p, ID_PICK_STATUS, &crate::i18n::tr(CHECK_HINT));
            note(
                p,
                ID_PICK_NOTE,
                &crate::i18n::tr("\"Pick target and type\" lets you click the field that should receive the typing. Switched off, its tray command is hidden and it can \
not be started. The shortcut is optional: leave it empty to use the tray command only. A shortcut must differ from every other hotkey. \
Warning: the click on the picked field is a real click, so pick a text field, never a button."),
            );
        }
        Page::Typing => {
            for (id, text) in [
                (ID_INITIAL, T_INITIAL),
                (ID_CHARDELAY, T_CHARDELAY),
                (ID_RELEASE, T_RELEASE),
                (ID_MAXCHARS, T_MAXCHARS),
                (ID_TABSPACES, T_TABSPACES),
            ] {
                label(p, id, text);
                edit(p, id, true);
            }
            label(p, ID_NEWLINE, T_NEWLINE);
            combo(p, ID_NEWLINE, &[crate::i18n::tr("Enter"), crate::i18n::tr("Shift+Enter"), crate::i18n::tr("Space"), crate::i18n::tr("(skipped)")]);
            label(p, ID_TAB, T_TAB);
            combo(p, ID_TAB, &[crate::i18n::tr("Tab key"), crate::i18n::tr("Spaces"), crate::i18n::tr("(skipped)")]);
            check(p, ID_STRIP, T_STRIP);
            note(
                p,
                ID_ESC_NOTE,
                &crate::i18n::tr("Press Esc while typing to abort. In web browsers the Tab key moves the focus out of a text box: choose \"Spaces\" if your text contains tabs."),
            );
        }
        Page::Clipboard => {
            label(p, ID_SAFE, T_SAFE);
            combo(
                p,
                ID_SAFE,
                &[crate::i18n::tr("Type as they are"), crate::i18n::tr("Plain punctuation and spaces"), crate::i18n::tr("ASCII only (no accents or symbols)")],
            );
            note(
                p,
                ID_SAFE_NOTE,
                &crate::i18n::tr("\"Plain punctuation and spaces\" turns curly quotes, long dashes, the ellipsis and non-breaking spaces into ASCII and removes invisible characters. \
\"ASCII only\" also removes accents (\u{e9} becomes e), writes EUR for the euro sign and uses ? for characters with no equivalent. Tabs and line breaks are set on the Typing page."),
            );
            check(p, ID_CLEAR, T_CLEAR);
            label(p, ID_FILES, T_FILES);
            combo(p, ID_FILES, &[crate::i18n::tr("Full paths, one per line"), crate::i18n::tr("File names, one per line"), crate::i18n::tr("Nothing")]);
            note(
                p,
                ID_FILES_NOTE,
                &crate::i18n::tr("Applies when the clipboard holds files copied in Explorer instead of text. Images and other content that is not text are never typed; Re:Type shows a notice instead."),
            );
        }
        Page::Appearance => {
            label(p, ID_THEME, T_THEME);
            combo(p, ID_THEME, &[crate::i18n::tr("Follow Windows (light or dark)"), crate::i18n::tr("Light"), crate::i18n::tr("Dark")]);
            note(
                p,
                ID_THEME_NOTE,
                &crate::i18n::tr("Applies to this window and to the tray menu. When Windows high contrast is on, its colours are always used. \
Message boxes always use the Windows colours."),
            );
            label(p, ID_LANG, T_LANG);
            let av = crate::i18n::available(&super::lang_dir());
            let mut items = vec![crate::i18n::tr("Automatic (follow Windows)")];
            items.extend(av.iter().map(|(_, n)| n.clone()));
            combo(p, ID_LANG, &items);
            LANGS.with(|l| *l.borrow_mut() = av.into_iter().map(|(c, _)| c).collect());
            note(
                p,
                ID_LANG_NOTE,
                &crate::i18n::tr("The tray menu changes at once; this window changes the next time you open it. To add a language, put a file such as nl.json in the lang folder next to settings.json (see the user guide). Help and About are shown in English."),
            );
        }
        Page::Startup => {
            check(p, ID_AUTOSTART, T_AUTOSTART);
            note(
                p,
                ID_AUTOSTART_NOTE,
                &crate::i18n::tr("Starts Re:Type when you sign in, for your Windows account only; no administrator rights are needed. \
Switching this off removes the entry. If you switched it off in Task Manager (Startup apps), Windows keeps it off."),
            );
            check(p, ID_ELEV, T_ELEV);
            note(
                p,
                ID_ELEV_NOTE,
                &crate::i18n::tr("WARNING: administrator programs can change the whole system; switch this on only if you need it. Windows does not let a normal \
program type into windows that run as administrator, so without it Re:Type refuses to type there. With it, Re:Type asks Windows for the rights each \
time it starts (the UAC consent prompt) and restarts itself; if you decline it keeps running normally. Re:Type never starts elevated silently, so \
together with start at sign-in the prompt appears at every sign-in. The consent prompt and the sign-in screen can never be typed into."),
            );
            note(p, ID_STATE_NOTE, "");
        }
        Page::About => {
            button(p, ID_ABOUT_WEB, T_ABOUT_WEB);
            button(p, ID_ABOUT_SRC, T_ABOUT_SRC);
            button(p, ID_LANG_ISSUES, T_LANG_ISSUES);
            if language_unchecked() {
                note(p, ID_LANG_WARN, &crate::i18n::tr(T_LANG_WARN));
            }
            label(p, ID_ABOUT_TEXT, T_ABOUTTEXT);
            let h = create(
                p,
                "EDIT",
                &crate::about::text(),
                WS_TABSTOP | WS_VSCROLL | (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32,
                WS_EX_CLIENTEDGE,
                ID_ABOUT_TEXT,
            );
            // SAFETY: plain message.
            unsafe { SendMessageW(h, EM_SETSEL, 0, 0) };
        }
        Page::Help => {
            label(p, ID_HELPTEXT, T_HELPTEXT);
            let h = create(
                p,
                richfield::CLASS,
                "",
                WS_TABSTOP | WS_VSCROLL | (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32,
                WS_EX_CLIENTEDGE,
                ID_HELPTEXT,
            );
            richhelp::fill(h);
        }
        Page::Templates => {
            label(p, ID_LIST, T_LIST);
            create(p, "LISTBOX", "", WS_TABSTOP | WS_VSCROLL | LBS_NOTIFY as u32, WS_EX_CLIENTEDGE, ID_LIST);
            button(p, ID_ADD, T_ADD);
            button(p, ID_REMOVE, T_REMOVE);
            label(p, ID_TNAME, T_TNAME);
            edit(p, ID_TNAME, false);
            check(p, ID_TENABLED, T_TENABLED);
            label(p, ID_THOTKEY, T_HOTKEY);
            rich(p, ID_THOTKEY, false);
            button(p, ID_TCHECK, T_CHECK);
            button(p, ID_TFIND, T_FIND);
            button(p, ID_TLISTEN, T_LISTEN);
            note(p, ID_TSTATUS, CHECK_HINT);
            label(p, ID_TPATTERN, T_TPATTERN);
            rich(p, ID_TPATTERN, true);
            label(p, ID_THELP, T_THELP);
            let help = create(
                p,
                "EDIT",
                HELP,
                WS_TABSTOP | WS_VSCROLL | (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32,
                WS_EX_CLIENTEDGE,
                ID_THELP,
            );
            // SAFETY: plain message.
            unsafe { SendMessageW(help, EM_SETSEL, 0, 0) };
        }
    }
}

fn make_font(dpi: u32) -> HFONT {
    // SAFETY: NONCLIENTMETRICSW is plain data; the call fills it for the given DPI.
    unsafe {
        let mut ncm: NONCLIENTMETRICSW = std::mem::zeroed();
        ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        if SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS, ncm.cbSize, &mut ncm as *mut _ as *mut _, 0, dpi) != 0 {
            CreateFontIndirectW(&ncm.lfMessageFont)
        } else {
            GetStockObject(DEFAULT_GUI_FONT) as HFONT
        }
    }
}

unsafe extern "system" fn enum_set_font(h: HWND, font: LPARAM) -> i32 {
    SendMessageW(h, WM_SETFONT, font as usize, 1);
    1
}

fn apply_font(st: &State) {
    // SAFETY: applies the font to every child of the window and of each page.
    unsafe {
        for w in std::iter::once(st.hwnd).chain(st.pages.iter().copied()).chain(std::iter::once(st.tab)) {
            EnumChildWindows(w, Some(enum_set_font), st.font as LPARAM);
        }
        SendMessageW(st.tab, WM_SETFONT, st.font as usize, 1);
    }
}

// ---- config <-> controls ------------------------------------------------------------------

fn fill(hwnd: HWND, cfg: &Config) {
    set_text(hwnd, ID_HOTKEY, &hotkey::to_local(&cfg.hotkey));
    set_text(hwnd, ID_INITIAL, &cfg.initial_delay_ms.to_string());
    set_text(hwnd, ID_CHARDELAY, &cfg.char_delay_ms.to_string());
    set_text(hwnd, ID_RELEASE, &cfg.release_timeout_ms.to_string());
    set_text(hwnd, ID_MAXCHARS, &cfg.max_chars.to_string());
    set_text(hwnd, ID_TABSPACES, &cfg.text.tab_spaces.to_string());
    set_check(hwnd, ID_STRIP, cfg.text.strip_trailing_newline);
    combo_select(hwnd, ID_NEWLINE, NewlineMode::ALL.iter().position(|m| *m == cfg.text.newline).unwrap_or(0));
    combo_select(hwnd, ID_TAB, TabMode::ALL.iter().position(|m| *m == cfg.text.tab).unwrap_or(0));
    combo_select(hwnd, ID_SAFE, SafeMode::ALL.iter().position(|m| *m == cfg.text.safe).unwrap_or(0));
    set_check(hwnd, ID_CLEAR, cfg.clear_clipboard);
    let lang_index = LANGS.with(|l| l.borrow().iter().position(|c| *c == cfg.language)).map(|i| i + 1).unwrap_or(0);
    combo_select(hwnd, ID_LANG, lang_index);
    combo_select(hwnd, ID_THEME, Theme::ALL.iter().position(|m| *m == cfg.theme).unwrap_or(0));
    combo_select(hwnd, ID_FILES, FilesMode::ALL.iter().position(|m| *m == cfg.files).unwrap_or(0));
    set_check(hwnd, ID_PICK_ON, cfg.pick_enabled);
    set_text(hwnd, ID_PICK_HK, &hotkey::to_local(&cfg.pick_hotkey));
    pick_controls(hwnd, cfg.pick_enabled);
    set_check(hwnd, ID_AUTOSTART, cfg.start_at_logon);
    set_check(hwnd, ID_ELEV, cfg.elevated);
    refresh_state(hwnd);
}

/// Languages that a native speaker has checked; every other one gets a warning note on the About page.
const VALIDATED_LANGUAGES: [&str; 2] = ["en", "it"];

/// True when the interface language in use has not been checked by a native speaker.
fn language_unchecked() -> bool {
    !VALIDATED_LANGUAGES.contains(&crate::i18n::active_code().as_str())
}

thread_local! {
    /// The shortcut field that is waiting for a key combination (0: none).
    static LISTENING: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
    /// True when the Listen button released Re:Type's hotkeys by itself.
    static LISTEN_RELEASED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn status_of(field: i32) -> i32 {
    match field {
        ID_PICK_HK => ID_PICK_STATUS,
        ID_THOTKEY => ID_TSTATUS,
        _ => ID_STATUS,
    }
}

fn set_release_button(hwnd: HWND, released: bool) {
    set_text(hwnd, ID_FREE_KEYS, &crate::i18n::tr(if released { T_RESTORE_KEYS } else { T_RELEASE_KEYS }));
}

/// "Release Re:Type shortcuts" / "Restore Re:Type shortcuts": Re:Type's own global hotkeys are
/// unregistered so that nothing of Re:Type reacts (or holds a combination) while the user works here.
fn release_toggle(hwnd: HWND) {
    listen_stop(hwnd, false);
    let on = !super::hotkeys_released();
    let failed = super::set_hotkeys_released(on);
    set_release_button(hwnd, on);
    let text = if on {
        crate::i18n::tr("Re:Type's own shortcuts are released until you restore them, save, or close this window.")
    } else if failed.is_empty() {
        crate::i18n::tr("Re:Type's own shortcuts are active again.")
    } else {
        crate::i18n::trf("Re:Type's own shortcuts are active again, but some are taken: {0}", &[&failed.join("; ")])
    };
    set_status(hwnd, ID_STATUS, &text);
    relayout(hwnd);
}

/// Starts or stops listening for a combination for `field`.
fn listen_toggle(hwnd: HWND, field: i32) {
    if LISTENING.with(|l| l.get()) == field {
        listen_stop(hwnd, true);
        return;
    }
    listen_stop(hwnd, false);
    if !super::hotkeys_released() {
        super::set_hotkeys_released(true);
        LISTEN_RELEASED.with(|l| l.set(true));
    }
    LISTENING.with(|l| l.set(field));
    set_status(hwnd, status_of(field), &crate::i18n::tr("Listening: press modifier keys and one key. Esc cancels."));
    relayout(hwnd);
}

/// Ends listening; Re:Type's hotkeys come back if listening had released them.
fn listen_stop(hwnd: HWND, say: bool) {
    let field = LISTENING.with(|l| l.replace(0));
    if LISTEN_RELEASED.with(|l| l.replace(false)) {
        super::set_hotkeys_released(false);
    }
    if field != 0 && say {
        set_status(hwnd, status_of(field), &crate::i18n::tr("Listening stopped."));
        relayout(hwnd);
    }
}

/// Handles a key press while a field is listening. Returns true when the message was used.
fn listen_key(hwnd: HWND, msg: &MSG) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
    let field = LISTENING.with(|l| l.get());
    if field == 0 {
        return false;
    }
    match msg.message {
        WM_KEYUP | WM_SYSKEYUP | WM_CHAR | WM_SYSCHAR | WM_DEADCHAR | WM_SYSDEADCHAR => return true,
        WM_KEYDOWN | WM_SYSKEYDOWN => {}
        _ => return false,
    }
    let vk = msg.wParam as u32;
    if matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5) {
        return true; // a modifier alone: keep waiting for the key
    }
    if vk == hotkey::VK_ESCAPE {
        listen_stop(hwnd, true);
        return true;
    }
    let mut mods = 0;
    // SAFETY: plain key-state queries.
    unsafe {
        if GetKeyState(i32::from(VK_CONTROL)) < 0 {
            mods |= hotkey::MOD_CONTROL;
        }
        if GetKeyState(i32::from(VK_MENU)) < 0 {
            mods |= hotkey::MOD_ALT;
        }
        if GetKeyState(i32::from(VK_SHIFT)) < 0 {
            mods |= hotkey::MOD_SHIFT;
        }
    }
    let status = status_of(field);
    if hotkey::key_to_vk(&hotkey::vk_to_name(vk)).is_err() {
        set_status(hwnd, status, &crate::i18n::tr("That key cannot be used in a shortcut."));
    } else if mods == 0 {
        set_status(hwnd, status, &crate::i18n::tr("Press at least one modifier (Ctrl, Alt or Shift) together with the key."));
    } else {
        let hk = hotkey::Hotkey { mods, vk };
        set_text(hwnd, field, &hotkey::to_local(&hk.to_string()));
        listen_stop(hwnd, false);
        set_status(hwnd, status, &crate::i18n::trf("Captured: {0}", &[&hotkey::to_local(&hk.to_string())]));
    }
    relayout(hwnd);
    true
}

/// The shortcut controls follow the check box.
fn pick_controls(hwnd: HWND, on: bool) {
    for id in [ID_PICK_HK, ID_PICK_CHECK, ID_PICK_FIND, ID_PICK_LISTEN] {
        enable(hwnd, id, on);
    }
}

/// The line on the Startup page that says what is true right now.
fn refresh_state(hwnd: HWND) {
    let mut t = if super::foreground::self_elevated() {
        crate::i18n::tr("Right now Re:Type is running with administrator rights.")
    } else {
        crate::i18n::tr("Right now Re:Type is running without administrator rights.")
    };
    if super::startup::disabled_by_windows() {
        t.push(' ');
        t.push_str(&crate::i18n::tr("Windows has the start-at-sign-in entry switched off in Task Manager (Startup apps); saving with the first option on turns it back on."));
    }
    set_text(hwnd, ID_STATE_NOTE, &t);
}

fn number(hwnd: HWND, id: i32, name: &str, range: (u32, u32)) -> Result<u32, String> {
    let v: u32 = get_text(hwnd, id).trim().parse().map_err(|_| format!("{name} must be a whole number"))?;
    if v < range.0 || v > range.1 {
        return Err(format!("{name} must be between {} and {}", range.0, range.1));
    }
    Ok(v)
}

/// Reads the general settings from the controls into `cfg`.
fn read_general(hwnd: HWND, cfg: &mut Config) -> Result<(), String> {
    cfg.hotkey = hotkey::to_english(&get_text(hwnd, ID_HOTKEY)).to_lowercase();
    cfg.initial_delay_ms = number(hwnd, ID_INITIAL, "Delay before typing", config::INITIAL_DELAY_RANGE)?;
    cfg.char_delay_ms = number(hwnd, ID_CHARDELAY, "Delay between keystrokes", config::CHAR_DELAY_RANGE)?;
    cfg.release_timeout_ms = number(hwnd, ID_RELEASE, "Max wait for key release", config::RELEASE_TIMEOUT_RANGE)?;
    cfg.max_chars = number(hwnd, ID_MAXCHARS, "Longest clipboard text", config::MAX_CHARS_RANGE)?;
    cfg.text.tab_spaces = number(hwnd, ID_TABSPACES, "Spaces per tab", config::TAB_SPACES_RANGE)?;
    cfg.text.strip_trailing_newline = get_check(hwnd, ID_STRIP);
    cfg.text.newline = NewlineMode::ALL[combo_index(hwnd, ID_NEWLINE).min(NewlineMode::ALL.len() - 1)];
    cfg.text.tab = TabMode::ALL[combo_index(hwnd, ID_TAB).min(TabMode::ALL.len() - 1)];
    cfg.text.safe = SafeMode::ALL[combo_index(hwnd, ID_SAFE).min(SafeMode::ALL.len() - 1)];
    cfg.clear_clipboard = get_check(hwnd, ID_CLEAR);
    cfg.theme = Theme::ALL[combo_index(hwnd, ID_THEME).min(Theme::ALL.len() - 1)];
    cfg.language = match combo_index(hwnd, ID_LANG) {
        0 => "auto".to_string(),
        i => LANGS.with(|l| l.borrow().get(i - 1).cloned()).unwrap_or_else(|| "auto".into()),
    };
    cfg.files = FilesMode::ALL[combo_index(hwnd, ID_FILES).min(FilesMode::ALL.len() - 1)];
    cfg.pick_enabled = get_check(hwnd, ID_PICK_ON);
    cfg.pick_hotkey = hotkey::to_english(&get_text(hwnd, ID_PICK_HK)).to_lowercase();
    cfg.start_at_logon = get_check(hwnd, ID_AUTOSTART);
    cfg.elevated = get_check(hwnd, ID_ELEV);
    Ok(())
}

mod templates {
    use super::*;
    use crate::config::Template;

    pub fn refresh_list(hwnd: HWND, cfg: &Config, sel: Option<usize>) {
        // SAFETY: plain messages with NUL-terminated strings.
        unsafe {
            let list = item(hwnd, ID_LIST);
            SendMessageW(list, LB_RESETCONTENT, 0, 0);
            for t in &cfg.templates {
                let line = format!("{} {}", if t.enabled { "[x]" } else { "[ ]" }, t.name);
                SendMessageW(list, LB_ADDSTRING, 0, wide(&line).as_ptr() as LPARAM);
            }
            SendMessageW(list, LB_SETCURSEL, sel.unwrap_or(usize::MAX), 0);
        }
        load_detail(hwnd, cfg, sel);
    }

    pub fn load_detail(hwnd: HWND, cfg: &Config, sel: Option<usize>) {
        let t = sel.and_then(|i| cfg.templates.get(i)).cloned().unwrap_or(Template {
            name: String::new(),
            enabled: false,
            hotkey: String::new(),
            pattern: String::new(),
        });
        set_text(hwnd, ID_TNAME, &t.name);
        set_check(hwnd, ID_TENABLED, t.enabled);
        set_text(hwnd, ID_THOTKEY, &hotkey::to_local(&t.hotkey));
        set_text(hwnd, ID_TPATTERN, &t.pattern);
        set_text(hwnd, ID_TSTATUS, CHECK_HINT);
        for id in [ID_TNAME, ID_TENABLED, ID_THOTKEY, ID_TCHECK, ID_TFIND, ID_TPATTERN, ID_REMOVE] {
            enable(hwnd, id, sel.is_some());
        }
    }

    /// Copies the detail controls back into the selected template.
    pub fn commit_detail(hwnd: HWND, cfg: &mut Config, sel: Option<usize>) {
        if let Some(t) = sel.and_then(|i| cfg.templates.get_mut(i)) {
            let name = get_text(hwnd, ID_TNAME);
            t.name = if name.trim().is_empty() { "Template".into() } else { name.trim().to_string() };
            t.enabled = get_check(hwnd, ID_TENABLED);
            t.hotkey = hotkey::to_english(&get_text(hwnd, ID_THOTKEY)).to_lowercase();
            t.pattern = get_text(hwnd, ID_TPATTERN);
        }
    }
}

// ---- hotkey checking ----------------------------------------------------------------------

fn check_hotkey(hwnd: HWND, edit: i32, status: i32) {
    let text = get_text(hwnd, edit);
    if edit == ID_PICK_HK && text.trim().is_empty() {
        set_status(hwnd, status, &crate::i18n::tr("Hotkey check: no shortcut is set; use the tray command."));
        relayout(hwnd);
        return;
    }
    let msg = match Hotkey::parse(hotkey::to_english(&text).trim()) {
        Err(e) => crate::i18n::trf("Not usable: {0}", &[&e.to_string()]),
        Ok(hk) => match super::probe_hotkey(hk) {
            Ok(()) => {
                set_text(hwnd, edit, &hotkey::to_local(&hk.to_string()));
                crate::i18n::trf("{0}: available", &[&hotkey::to_local(&hk.to_string())])
            }
            Err(e) => crate::i18n::trf("{0}: {1}. Windows does not say which program holds it. Choose another.", &[&hotkey::to_local(&hk.to_string()), &e.to_string()]),
        },
    };
    set_status(hwnd, status, &crate::i18n::trf("Hotkey check: {0}", &[&msg]));
    relayout(hwnd);
}

fn find_free(hwnd: HWND, edit: i32, status: i32, default_key: &str) {
    let key = hotkey::parse_combo(hotkey::to_english(&get_text(hwnd, edit)).trim())
        .map(|(_, vk)| vk)
        .unwrap_or_else(|_| hotkey::key_to_vk(default_key).unwrap_or(b'V' as u32));
    match super::first_free(&hotkey::suggestions(key)) {
        Some(hk) => {
            set_text(hwnd, edit, &hotkey::to_local(&hk.to_string()));
            set_status(hwnd, status, &crate::i18n::trf("Hotkey check: {0}: available", &[&hotkey::to_local(&hk.to_string())]));
        }
        None => set_status(
            hwnd,
            status,
            &crate::i18n::trf("Hotkey check: no free ctrl+...+{0} combination found.", &[&hotkey::vk_to_name(key).to_string()]),
        ),
    }
    relayout(hwnd);
}

/// Opens one of the fixed addresses of the About page in the default browser.
/// Shows the settings file in Explorer (selected), or its folder if the file does not exist yet.
fn open_settings_folder() {
    let path = super::config_path_display();
    if path.is_empty() {
        return;
    }
    let args = if std::path::Path::new(&path).exists() {
        format!("/select,\"{path}\"")
    } else {
        let dir = std::path::Path::new(&path).parent().map(|d| d.display().to_string()).unwrap_or_default();
        format!("\"{dir}\"")
    };
    // SAFETY: plain call with NUL-terminated strings; explorer.exe is a fixed name and the argument is our own
    // settings path.
    unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            wide("open").as_ptr(),
            wide("explorer.exe").as_ptr(),
            wide(&args).as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

fn open_url(url: &str) {
    // SAFETY: plain call with NUL-terminated strings; the address is one of our constants.
    unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            wide("open").as_ptr(),
            wide(url).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

// ---- actions ------------------------------------------------------------------------------

fn save(hwnd: HWND) {
    let Some(mut cfg) = with_state(|s| s.cfg.clone()) else { return };
    {
        let sel = with_state(|s| s.sel).flatten();
        templates::commit_detail(hwnd, &mut cfg, sel);
    }
    let was_elevated_option = cfg.elevated;
    if let Err(e) = read_general(hwnd, &mut cfg) {
        super::message_box(hwnd, &e, MB_OK | MB_ICONWARNING);
        return;
    }
    let errors = cfg.validate();
    if !errors.is_empty() {
        super::message_box(hwnd, &format!("Cannot save:\n{}", errors.join("\n")), MB_OK | MB_ICONWARNING);
        return;
    }
    // Re:Type's hotkeys come back before the new ones are registered
    LISTENING.with(|l| l.set(0));
    LISTEN_RELEASED.with(|l| l.set(false));
    super::set_hotkeys_released(false);
    set_release_button(hwnd, false);
    match super::commit_config(cfg.clone()) {
        Err(e) => super::message_box(hwnd, &e, MB_OK | MB_ICONERROR),
        Ok(failed) => {
            with_state(|s| s.cfg = cfg.clone());
            apply_theme(hwnd, cfg.theme);
            if let Err(e) = super::startup::apply(cfg.start_at_logon) {
                super::message_box(hwnd, &e, MB_OK | MB_ICONWARNING);
            }
            refresh_state(hwnd);
            if cfg.elevated && !was_elevated_option && !super::foreground::self_elevated() {
                let answer = super::message_box_ask(
                    hwnd,
                    &crate::i18n::tr("Run as administrator is now on. It applies the next time Re:Type starts.\n\nRestart Re:Type now? Windows will ask for your consent (UAC prompt). If you decline, Re:Type keeps running without administrator rights."),
                    MB_YESNO | MB_ICONQUESTION,
                );
                if answer == IDYES && super::restart_elevated() {
                    if let Some(main) = super::with_app(|a| a.hwnd) {
                        // SAFETY: posts the Quit command to our own main window.
                        unsafe { PostMessageW(main, WM_COMMAND, super::tray::ID_QUIT, 0) };
                    }
                }
            }
            {
                let sel = with_state(|s| s.sel).flatten();
                templates::refresh_list(hwnd, &cfg, sel);
            }
            if failed.is_empty() {
                set_status(hwnd, ID_FOOTER, &crate::i18n::tr("Saved and applied."));
            } else {
                set_status(hwnd, ID_FOOTER, &crate::i18n::tr("Saved, but some hotkeys are taken."));
                super::message_box(
                    hwnd,
                    &crate::i18n::trf("These hotkeys could not be registered:\n{0}\n\nUse \"Find free\" to choose another.", &[&failed.join("\n")]),
                    MB_OK | MB_ICONWARNING,
                );
            }
        }
    }
}

fn on_command(hwnd: HWND, id: i32, code: u32) {
    if code == richfield::EN_CHANGE {
        if let Some((_, kind)) = RICH_FIELDS.iter().find(|(i, _)| *i == id) {
            richfield::restyle(item(hwnd, id), *kind);
        }
        return;
    }
    match (id, code) {
        (ID_TAB_CONTROL, 1 /* LBN_SELCHANGE */) => {
            let list = with_state(|s| s.tab).unwrap_or(std::ptr::null_mut());
            // SAFETY: plain message to our own list box.
            let index = unsafe { SendMessageW(list, LB_GETCURSEL, 0, 0) }.max(0) as usize;
            show_page(index);
            relayout(hwnd);
        }
        (ID_SAVE, 0) => save(hwnd),
        (ID_CLOSE, 0) => {
            // SAFETY: destroying our own window.
            unsafe { DestroyWindow(hwnd) };
        }
        (2 /* IDCANCEL: the Esc key */, 0) => {
            // SAFETY: destroying our own window.
            unsafe { DestroyWindow(hwnd) };
        }
        (ID_FILE, 0) => open_settings_folder(),
        (ID_ABOUT_WEB, 0) => open_url(crate::about::WEBSITE_URL),
        (ID_ABOUT_SRC, 0) => open_url(crate::about::SOURCE_URL),
        (ID_LANG_ISSUES, 0) => open_url(crate::about::ISSUES_URL),
        (ID_CHECK, 0) => check_hotkey(hwnd, ID_HOTKEY, ID_STATUS),
        (ID_FIND, 0) => find_free(hwnd, ID_HOTKEY, ID_STATUS, "v"),
        (ID_PICK_ON, 0) => pick_controls(hwnd, get_check(hwnd, ID_PICK_ON)),
        (ID_LISTEN, 0) => listen_toggle(hwnd, ID_HOTKEY),
        (ID_PICK_LISTEN, 0) => listen_toggle(hwnd, ID_PICK_HK),
        (ID_TLISTEN, 0) => listen_toggle(hwnd, ID_THOTKEY),
        (ID_FREE_KEYS, 0) => release_toggle(hwnd),
        (ID_PICK_CHECK, 0) => check_hotkey(hwnd, ID_PICK_HK, ID_PICK_STATUS),
        (ID_PICK_FIND, 0) => find_free(hwnd, ID_PICK_HK, ID_PICK_STATUS, "p"),
        (ID_TCHECK, 0) => check_hotkey(hwnd, ID_THOTKEY, ID_TSTATUS),
        (ID_TFIND, 0) => find_free(hwnd, ID_THOTKEY, ID_TSTATUS, "1"),
        (ID_LIST, 1 /* LBN_SELCHANGE */) => {
            // SAFETY: plain message.
            let new = unsafe { SendMessageW(item(hwnd, ID_LIST), LB_GETCURSEL, 0, 0) };
            let new = usize::try_from(new).ok();
            if let Some((mut cfg, old)) = with_state(|s| (s.cfg.clone(), s.sel)) {
                templates::commit_detail(hwnd, &mut cfg, old);
                with_state(|s| {
                    s.cfg = cfg.clone();
                    s.sel = new;
                });
                templates::refresh_list(hwnd, &cfg, new);
            }
        }
        (ID_ADD, 0) => {
            if let Some((mut cfg, old)) = with_state(|s| (s.cfg.clone(), s.sel)) {
                templates::commit_detail(hwnd, &mut cfg, old);
                cfg.templates.push(config::Template {
                    name: format!("Template {}", cfg.templates.len() + 1),
                    enabled: false,
                    hotkey: String::new(),
                    pattern: "{CLIPBOARD}".into(),
                });
                let new = Some(cfg.templates.len() - 1);
                with_state(|s| {
                    s.cfg = cfg.clone();
                    s.sel = new;
                });
                templates::refresh_list(hwnd, &cfg, new);
            }
        }
        (ID_REMOVE, 0) => {
            if let Some((mut cfg, Some(i))) = with_state(|s| (s.cfg.clone(), s.sel)) {
                if i < cfg.templates.len() {
                    cfg.templates.remove(i);
                }
                with_state(|s| {
                    s.cfg = cfg.clone();
                    s.sel = None;
                });
                templates::refresh_list(hwnd, &cfg, None);
            }
        }
        _ => {}
    }
}

// ---- window procedures --------------------------------------------------------------------

/// Positions everything for the current client size.
fn relayout(_hwnd: HWND) {
    let Some((hwnd, tab, pages, font, dpi)) = with_state(|s| (s.hwnd, s.tab, s.pages.clone(), s.font, s.dpi)) else {
        return;
    };
    // SAFETY: moves our own child windows; the measuring DC is released by `Metrics`.
    unsafe {
        let mut rc: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rc);
        let m = Metrics::new(hwnd, font, dpi);
        let margin = m.dp(10);
        let button_h = m.line_height() + m.dp(12);
        let footer_h = button_h + margin;
        // left pane: the list of sections; right pane: the page of the selected section
        let list_w = list_width(&m);
        let area_bottom = rc.bottom - footer_h - margin;
        SetWindowPos(tab, std::ptr::null_mut(), margin, margin, list_w, area_bottom - margin, SWP_NOZORDER | SWP_NOACTIVATE);
        let inner = RECT { left: margin + list_w + margin, top: margin, right: rc.right - margin, bottom: area_bottom };
        let (pw, ph) = (inner.right - inner.left, inner.bottom - inner.top);
        for (i, &page) in pages.iter().enumerate() {
            SetWindowPos(page, std::ptr::null_mut(), inner.left, inner.top, pw, ph, SWP_NOZORDER | SWP_NOACTIVATE);
            layout_page(&m, PAGES[i].0, page, pw, ph, false);
        }
        // footer: status text on the left, buttons on the right
        let y = rc.bottom - footer_h;
        let bw = |id: i32| (m.text_width(&plain(&text_of(GetDlgItem(hwnd, id)))) + m.dp(28)).max(m.dp(90));
        let (save_w, close_w) = (bw(ID_SAVE), bw(ID_CLOSE));
        let close_x = rc.right - margin - close_w;
        let save_x = close_x - m.dp(8) - save_w;
        let put = |id: i32, x: i32, w: i32| {
            SetWindowPos(GetDlgItem(hwnd, id), std::ptr::null_mut(), x, y, w, button_h, SWP_NOZORDER | SWP_NOACTIVATE);
        };
        put(ID_CLOSE, close_x, close_w);
        put(ID_SAVE, save_x, save_w);
        let line_h = m.line_height();
        let file_w = bw(ID_FILE);
        put(ID_FILE, margin, file_w);
        let status_x = margin + file_w + m.dp(8);
        SetWindowPos(
            GetDlgItem(hwnd, ID_FOOTER),
            std::ptr::null_mut(),
            status_x,
            y + (button_h - line_h) / 2,
            (save_x - m.dp(8) - status_x).max(m.dp(40)),
            line_h + 2,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        // repaint the window and every control in it: erasing only the parent would paint over
        // controls that are not themselves marked as needing a redraw
        RedrawWindow(hwnd, std::ptr::null(), std::ptr::null_mut(), RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN);
    }
}

/// Width of the left pane: the widest section name plus room for the selection mark.
fn list_width(m: &Metrics) -> i32 {
    let w = PAGES.iter().map(|(_, n)| m.text_width(&plain(&crate::i18n::tr(n)))).max().unwrap_or(0);
    (w + m.dp(32)).max(m.dp(110))
}

/// Smallest client size at which every page fits, measured at the narrowest supported width.
fn compute_min_client(hwnd: HWND, font: HFONT, dpi: u32, pages: &[HWND], tab: HWND) -> (i32, i32) {
    // SAFETY: measuring only; nothing is moved (dry run).
    {
        let m = Metrics::new(hwnd, font, dpi);
        let margin = m.dp(10);
        let min_w = m.dp(600);
        let button_h = m.line_height() + m.dp(12);
        let _ = tab;
        let list_w = list_width(&m);
        let inner_w = min_w;
        let chrome_h = 0;
        let mut need = 0;
        for (i, &p) in pages.iter().enumerate() {
            need = need.max(layout_page(&m, PAGES[i].0, p, inner_w, 0, true));
        }
        (min_w + list_w + 3 * margin, need + chrome_h + button_h + 3 * margin)
    }
}

unsafe extern "system" fn page_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            // controls on a page report to the page; the settings window handles them
            SendMessageW(GetParent(hwnd), WM_COMMAND, wp, lp)
        }
        // the pages use the window colour of the active scheme, like the tab control around them,
        // so labels, notes and check boxes never show as patches of a different colour
        WM_ERASEBKGND => {
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            theme::fill_bg(wp as HDC, &rc);
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let control = lp as HWND;
            // the read-only token list is meant to look like a field, the notes are not
            theme::control_colors(msg, wp as HDC, control, GetDlgCtrlID(control) == ID_THELP)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn show_page(index: usize) {
    if let Some(pages) = with_state(|s| s.pages.clone()) {
        for (i, p) in pages.iter().enumerate() {
            // SAFETY: shows or hides our own page windows.
            unsafe { ShowWindow(*p, if i == index { SW_SHOW } else { SW_HIDE }) };
        }
        LAST_TAB.with(|t| t.set(index));
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let handled = catch_unwind(AssertUnwindSafe(|| match msg {
        WM_COMMAND => {
            on_command(hwnd, (wp & 0xFFFF) as i32, ((wp >> 16) & 0xFFFF) as u32);
            Some(0)
        }
        WM_SIZE => {
            relayout(hwnd);
            Some(0)
        }
        WM_ERASEBKGND => {
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            theme::fill_bg(wp as HDC, &rc);
            Some(1)
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            Some(theme::control_colors(msg, wp as HDC, lp as HWND, false))
        }
        // the user changed the Windows colour mode or switched high contrast on or off
        WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED => {
            if let Some(t) = with_state(|s| s.cfg.theme) {
                apply_theme(hwnd, t);
            }
            None
        }
        WM_GETMINMAXINFO => {
            if let Some((min_client, _)) = with_state(|s| (s.min_client, ())) {
                let (mut wr, mut cr): (RECT, RECT) = (std::mem::zeroed(), std::mem::zeroed());
                GetWindowRect(hwnd, &mut wr);
                GetClientRect(hwnd, &mut cr);
                let (cw, ch) = ((wr.right - wr.left) - (cr.right - cr.left), (wr.bottom - wr.top) - (cr.bottom - cr.top));
                let mut work: RECT = std::mem::zeroed();
                SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work as *mut RECT as *mut _, 0);
                let info = &mut *(lp as *mut MINMAXINFO);
                info.ptMinTrackSize = POINT {
                    x: (min_client.0 + cw).min(work.right - work.left),
                    y: (min_client.1 + ch).min(work.bottom - work.top),
                };
            }
            Some(0)
        }
        WM_DPICHANGED => {
            let new_dpi = ((wp >> 16) & 0xFFFF) as u32;
            let suggested = &*(lp as *const RECT);
            with_state(|s| {
                s.dpi = new_dpi;
                DeleteObject(s.font as HGDIOBJ);
                s.font = make_font(new_dpi);
                apply_font(s);
                s.min_client = compute_min_client(s.hwnd, s.font, new_dpi, &s.pages, s.tab);
            });
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                suggested.left,
                suggested.top,
                suggested.right - suggested.left,
                suggested.bottom - suggested.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            relayout(hwnd);
            Some(0)
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            Some(0)
        }
        WM_DESTROY => {
            LISTENING.with(|l| l.set(0));
            LISTEN_RELEASED.with(|l| l.set(false));
            super::set_hotkeys_released(false);
            if let Some(st) = STATE.with(|s| s.borrow_mut().take()) {
                DeleteObject(st.font as HGDIOBJ);
            }
            theme::release();
            Some(0)
        }
        _ => None,
    }));
    match handled {
        Ok(Some(r)) => r,
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// Applies the colour scheme to the window, its title bar and every control (REQ-UX-007).
fn apply_theme(hwnd: HWND, theme_setting: Theme) {
    let p = theme::activate(theme_setting);
    theme::init_menus(theme_setting);
    theme::apply_window(hwnd, p.dark);
    theme::apply_tree(hwnd, p.dark);
    for (id, kind) in RICH_FIELDS {
        let h = item(hwnd, id);
        if !h.is_null() {
            richfield::setup(h, kind);
        }
    }
    let help = item(hwnd, ID_HELPTEXT);
    if !help.is_null() {
        richhelp::fill(help);
    }
    // SAFETY: repaints our own window and its children.
    unsafe { RedrawWindow(hwnd, std::ptr::null(), std::ptr::null_mut(), RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_FRAME) };
}

/// Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+PgDn and Ctrl+PgUp switch the tab page from anywhere in the window.
fn cycle_tab(forward: bool) {
    let Some(tab) = with_state(|s| s.tab) else { return };
    // SAFETY: plain messages to our tab control.
    unsafe {
        let n = PAGES.len();
        let cur = SendMessageW(tab, LB_GETCURSEL, 0, 0).max(0) as usize;
        let next = if forward { (cur + 1) % n } else { (cur + n - 1) % n };
        SendMessageW(tab, LB_SETCURSEL, next, 0);
        show_page(next);
        relayout(tab);
        // the old page is hidden now: keep the focus on something that is visible
        windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(tab);
    }
}

/// Shows page `index` and puts the focus on the tab control.
fn select_page(index: usize) {
    let Some(tab) = with_state(|s| s.tab) else { return };
    // SAFETY: plain messages to our tab control.
    unsafe {
        SendMessageW(tab, LB_SETCURSEL, index, 0);
        show_page(index);
        relayout(tab);
        windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(tab);
    }
}

/// Lets Tab, Enter, Esc, access keys and tab-page shortcuts work inside the settings window.
/// Called from the message loop.
pub fn is_dialog_message(msg: &MSG) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_SHIFT};
    let hwnd = STATE.with(|s| s.try_borrow().ok().and_then(|s| s.as_ref().map(|s| s.hwnd)));
    match hwnd {
        // SAFETY: `hwnd` is the live settings window; `msg` comes from GetMessageW.
        Some(h) if !h.is_null() => unsafe {
            if (msg.hwnd == h || IsChild(h, msg.hwnd) != 0) && listen_key(h, msg) {
                return true;
            }
            if msg.message == WM_KEYDOWN && GetKeyState(i32::from(VK_CONTROL)) < 0 && (msg.hwnd == h || IsChild(h, msg.hwnd) != 0) {
                let shift = GetKeyState(i32::from(VK_SHIFT)) < 0;
                match msg.wParam as u16 {
                    VK_TAB_KEY => {
                        cycle_tab(!shift);
                        return true;
                    }
                    VK_NEXT_KEY => {
                        cycle_tab(true);
                        return true;
                    }
                    VK_PRIOR_KEY => {
                        cycle_tab(false);
                        return true;
                    }
                    _ => {}
                }
            }
            if msg.message == WM_KEYDOWN && msg.wParam == 0x70 /* F1 */ && (msg.hwnd == h || IsChild(h, msg.hwnd) != 0) {
                select_page(HELP_PAGE);
                return true;
            }
            IsDialogMessageW(h, msg) != 0
        },
        _ => false,
    }
}

fn register_classes() {
    // SAFETY: standard class registration; registering twice fails harmlessly.
    unsafe {
        let hinst = GetModuleHandleW(std::ptr::null());
        let class = wide(CLASS);
        let main = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: super::tray::app_icon(true),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as usize as _,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
            hIconSm: super::tray::app_icon(false),
        };
        RegisterClassExW(&main);
        let pclass = wide(PAGE_CLASS);
        let page = WNDCLASSEXW {
            lpfnWndProc: Some(page_proc),
            hbrBackground: (COLOR_BTNFACE + 1) as usize as _,
            lpszClassName: pclass.as_ptr(),
            style: 0,
            ..main
        };
        RegisterClassExW(&page);
    }
}

/// Opens the settings window on the Help page.
pub fn open_help(owner: HWND) {
    LAST_TAB.with(|t| t.set(HELP_PAGE));
    open(owner);
    select_page(HELP_PAGE);
}

/// Opens the settings window, or brings the open one to the front.
pub fn open(_owner: HWND) {
    if let Some(existing) = with_state(|s| s.hwnd) {
        // SAFETY: the window belongs to this thread.
        unsafe {
            ShowWindow(existing, SW_RESTORE);
            SetForegroundWindow(existing);
        }
        return;
    }
    register_classes();
    // SAFETY: window creation and control setup on the UI thread.
    unsafe {
        InitCommonControlsEx(&INITCOMMONCONTROLSEX { dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32, dwICC: ICC_TAB_CLASSES });
        richfield::load_library();
        let hinst = GetModuleHandleW(std::ptr::null());
        let dpi = GetDpiForSystem();
        let scale = |v: i32| (v as u32 * dpi / 96) as i32;
        let title = format!("{} - Settings", super::product_title());
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            wide(CLASS).as_ptr(),
            wide(&title).as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            scale(700),
            scale(640),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            crate::log::error(&format!("cannot create the settings window (error {})", windows_sys::Win32::Foundation::GetLastError()));
            return;
        }
        let dpi = GetDpiForWindow(hwnd).max(96);
        let font = make_font(dpi);
        let tab = create(hwnd, "LISTBOX", "", WS_TABSTOP | WS_CLIPSIBLINGS | WS_VSCROLL | (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32, WS_EX_CLIENTEDGE, ID_TAB_CONTROL);
        for (i, (_, name)) in PAGES.iter().enumerate() {
            let _ = i;
            SendMessageW(tab, LB_ADDSTRING_, 0, wide(&plain(&crate::i18n::tr(name))).as_ptr() as LPARAM);
        }
        let mut pages = Vec::new();
        for (page, _) in PAGES {
            let p = create(hwnd, PAGE_CLASS, "", WS_CLIPCHILDREN, WS_EX_CONTROLPARENT, 0);
            build_page(*page, p);
            pages.push(p);
        }
        button(hwnd, ID_FILE, "settings.json");
        button(hwnd, ID_SAVE, T_SAVE);
        button(hwnd, ID_CLOSE, T_CLOSE);
        // status line: a read-only edit control, so that it can be focused and read (REQ-ACC-002)
        create(hwnd, "EDIT", "", WS_TABSTOP | (ES_READONLY | ES_AUTOHSCROLL) as u32, 0, ID_FOOTER);
        // names for assistive technology: the tab control and each page
        SetWindowTextW(tab, wide(&crate::i18n::tr("Settings sections")).as_ptr());
        for (i, p) in pages.iter().enumerate() {
            SetWindowTextW(*p, wide(&crate::i18n::trf("{0} settings", &[&plain(&crate::i18n::tr(PAGES[i].1))])).as_ptr());
        }
        let cfg = super::current_config();
        STATE.with(|s| {
            *s.borrow_mut() = Some(State {
                hwnd,
                tab,
                pages: pages.clone(),
                font,
                dpi,
                cfg: cfg.clone(),
                min_client: (0, 0),
                sel: None,
            })
        });
        with_state(|s| {
            apply_font(s);
            s.min_client = compute_min_client(hwnd, font, dpi, &pages, tab);
        });
        fill(hwnd, &cfg);
        templates::refresh_list(hwnd, &cfg, None);
        set_text(hwnd, ID_FOOTER, "");
        apply_theme(hwnd, cfg.theme);
        // always show focus marks and access-key underlines in this window
        SendMessageW(hwnd, WM_CHANGEUISTATE, 2 | (3 << 16), 0);
        // grow to the minimum if the initial size is smaller than the content needs
        let min = with_state(|s| s.min_client).unwrap_or((0, 0));
        let (mut wr, mut cr): (RECT, RECT) = (std::mem::zeroed(), std::mem::zeroed());
        GetWindowRect(hwnd, &mut wr);
        GetClientRect(hwnd, &mut cr);
        let want_w = (wr.right - wr.left).max(min.0 + (wr.right - wr.left) - (cr.right - cr.left));
        let want_h = (wr.bottom - wr.top).max(min.1 + (wr.bottom - wr.top) - (cr.bottom - cr.top));
        SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, want_w, want_h, SWP_NOMOVE | SWP_NOZORDER);
        let last = LAST_TAB.with(|t| t.get()).min(PAGES.len() - 1);
        SendMessageW(tab, LB_SETCURSEL, last, 0);
        show_page(last);
        relayout(hwnd);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(tab);
    }
}
