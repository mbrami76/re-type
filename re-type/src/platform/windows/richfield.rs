// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami
//! Shortcut and pattern fields that show what the program recognises (REQ-UX-014, REQ-UX-016).
//!
//! The fields are Windows rich-text edit controls that hold plain text. After every change the
//! text is judged by `crate::highlight`: a recognised key name is written in capitals and drawn
//! in inverted colours, like a small key; a name that is not recognised is drawn in red and
//! underlined. Only colours and the underline are changed, so the text stays what the user
//! typed (apart from the capitals), and a screen reader reads it like any edit field.

use super::theme;
use crate::highlight::{self, Span};
use std::cell::Cell;
use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::System::LibraryLoader::LoadLibraryW;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, SendMessageW};

const EM_EXGETSEL: u32 = 0x0434;
const EM_EXSETSEL: u32 = 0x0437;
pub(super) const EM_SETBKGNDCOLOR: u32 = 0x0443;
pub(super) const EM_SETCHARFORMAT: u32 = 0x0444;
const EM_SETEVENTMASK: u32 = 0x0445;
const EM_GETTEXTEX: u32 = 0x045E;
pub(super) const EM_REPLACESEL: u32 = 0x00C2;
const ENM_CHANGE: isize = 1;
pub(super) const SCF_SELECTION: usize = 1;
pub(super) const SCF_ALL: usize = 4;
pub(super) const CFM_UNDERLINE: u32 = 0x0000_0004;
pub(super) const CFE_UNDERLINE: u32 = 0x0000_0004;
pub(super) const CFM_COLOR: u32 = 0x4000_0000;
pub(super) const CFM_BACKCOLOR: u32 = 0x0400_0000;
/// Notification code sent through WM_COMMAND when the text changed.
pub const EN_CHANGE: u32 = 0x0300;
const RED: u32 = 0x0000_00D0; // COLORREF 0x00BBGGRR: a red that reads on light and on dark

#[repr(C)]
pub(super) struct CharFormat2W {
    pub(super) cb_size: u32,
    pub(super) mask: u32,
    pub(super) effects: u32,
    pub(super) height: i32,
    pub(super) offset: i32,
    pub(super) text_color: u32,
    pub(super) charset: u8,
    pub(super) pitch: u8,
    pub(super) face: [u16; 32],
    pub(super) weight: u16,
    pub(super) spacing: i16,
    pub(super) back_color: u32,
    pub(super) lcid: u32,
    pub(super) reserved: u32,
    pub(super) style: i16,
    pub(super) kerning: u16,
    pub(super) underline_type: u8,
    pub(super) animation: u8,
    pub(super) rev_author: u8,
    pub(super) underline_color: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CharRange {
    min: i32,
    max: i32,
}

#[repr(C)]
struct GetTextEx {
    cb: u32,
    flags: u32,
    codepage: u32,
    default_char: *const i8,
    used_default: *mut i32,
}

/// What a field holds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Shortcut,
    /// A template pattern; the template limits apply.
    Pattern,
}

thread_local! {
    static BUSY: Cell<bool> = const { Cell::new(false) };
}

/// Loads the library that provides the control (once per process).
pub fn load_library() {
    // SAFETY: NUL-terminated name of a system library; the handle is kept for the process lifetime.
    unsafe { LoadLibraryW(super::tray::wide("Msftedit.dll").as_ptr()) };
}

pub const CLASS: &str = "RICHEDIT50W";

pub(super) fn format(mask: u32, effects: u32, text: u32, back: u32) -> CharFormat2W {
    // SAFETY: plain data, all-zero is a valid value for every field.
    let mut f: CharFormat2W = unsafe { std::mem::zeroed() };
    f.cb_size = std::mem::size_of::<CharFormat2W>() as u32;
    f.mask = mask;
    f.effects = effects;
    f.text_color = text;
    f.back_color = back;
    f
}

/// Sets the colours of an empty-formatted field to the active scheme and styles its text.
pub fn setup(h: HWND, kind: Kind) {
    let p = theme::palette();
    // SAFETY: plain messages to a rich-text control of this thread.
    unsafe {
        SendMessageW(h, EM_SETBKGNDCOLOR, 0, p.edit_bg as LPARAM);
        SendMessageW(h, EM_SETEVENTMASK, 0, ENM_CHANGE);
    }
    restyle(h, kind);
}

fn read_text(h: HWND) -> Vec<u16> {
    // SAFETY: the buffer has room for the text and a terminator; the structure outlives the call.
    unsafe {
        let n = GetWindowTextLengthW(h).max(0) as usize;
        let mut buf = vec![0u16; n + 2];
        let req = GetTextEx { cb: (buf.len() * 2) as u32, flags: 0, codepage: 1200, default_char: std::ptr::null(), used_default: std::ptr::null_mut() };
        let got = SendMessageW(h, EM_GETTEXTEX, &req as *const GetTextEx as WPARAM, buf.as_mut_ptr() as LPARAM).max(0) as usize;
        buf.truncate(got.min(n + 1));
        buf
    }
}

fn select(h: HWND, start: usize, end: usize) {
    let r = CharRange { min: start as i32, max: end as i32 };
    // SAFETY: the range outlives the call.
    unsafe { SendMessageW(h, EM_EXSETSEL, 0, &r as *const CharRange as LPARAM) };
}

/// Judges the text of the field and redraws it. Safe to call from the change notification: it
/// does nothing while it is already running.
pub fn restyle(h: HWND, kind: Kind) {
    if BUSY.with(|b| b.replace(true)) {
        return;
    }
    let p = theme::palette();
    // SAFETY: plain messages to a rich-text control of this thread; every structure outlives its call.
    unsafe {
        SendMessageW(h, EM_SETEVENTMASK, 0, 0);
        let mut keep = CharRange { min: 0, max: 0 };
        SendMessageW(h, EM_EXGETSEL, 0, &mut keep as *mut CharRange as LPARAM);
        let mut units = read_text(h);
        let mut text = String::from_utf16_lossy(&units);
        // a shortcut typed in another order is put in the fixed order CTRL, WIN, ALT, SHIFT, key
        if kind == Kind::Shortcut {
            if let Some(fixed) = highlight::ordered_shortcut(&text) {
                select(h, 0, units.len());
                let w = super::tray::wide(&fixed);
                SendMessageW(h, EM_REPLACESEL, 0, w.as_ptr() as LPARAM);
                keep = CharRange { min: fixed.len() as i32, max: fixed.len() as i32 };
                units = fixed.encode_utf16().collect();
                text = fixed;
            }
        }
        let spans: Vec<Span> = match kind {
            Kind::Shortcut => highlight::shortcut_spans(&text),
            Kind::Pattern => highlight::pattern_spans(&text, true),
        };
        // recognised names are written in capitals
        for sp in &spans {
            let now = String::from_utf16_lossy(&units[sp.start.min(units.len())..sp.end.min(units.len())]);
            if sp.valid && now != sp.canonical {
                select(h, sp.start, sp.end);
                let w = super::tray::wide(&sp.canonical);
                SendMessageW(h, EM_REPLACESEL, 0, w.as_ptr() as LPARAM);
            }
        }
        // plain colours for everything, then the marks
        let base = format(CFM_COLOR | CFM_BACKCOLOR | CFM_UNDERLINE, 0, p.edit_text, p.edit_bg);
        SendMessageW(h, EM_SETCHARFORMAT, SCF_ALL, &base as *const CharFormat2W as LPARAM);
        for sp in &spans {
            select(h, sp.start, sp.end);
            let f = if sp.valid {
                // inverted colours, like a small key
                format(CFM_COLOR | CFM_BACKCOLOR, 0, p.edit_bg, p.edit_text)
            } else {
                format(CFM_COLOR | CFM_UNDERLINE, CFE_UNDERLINE, RED, 0)
            };
            SendMessageW(h, EM_SETCHARFORMAT, SCF_SELECTION, &f as *const CharFormat2W as LPARAM);
        }
        select(h, keep.min.max(0) as usize, keep.max.max(0) as usize);
        SendMessageW(h, EM_SETEVENTMASK, 0, ENM_CHANGE);
    }
    BUSY.with(|b| b.set(false));
}
