// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami
//! The Help page as formatted text: a title, coloured headings, bullets, bold item names, a
//! monospace font for pattern tokens and a warning colour (REQ-DOC-001). The text comes from
//! `crate::help::blocks()` and is translated line by line; the formatting depends only on the kind
//! of line, never on the words, so every language is formatted alike. No pictures or symbols are
//! used: bullets are the paragraph bullets of the rich-text control.

use super::richfield::{
    format, CharFormat2W, CFM_COLOR, EM_REPLACESEL, EM_SETBKGNDCOLOR, EM_SETCHARFORMAT, SCF_SELECTION,
};
use super::theme;
use crate::help::{self, Block, Kind};
use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;

const EM_SETSEL: u32 = 0x00B1;
const EM_SCROLLCARET: u32 = 0x00B7;
const EM_SETREADONLY: u32 = 0x00CF;
const EM_SETPARAFORMAT: u32 = 0x0447;
const CFM_BOLD: u32 = 0x0000_0001;
const CFE_BOLD: u32 = 0x0000_0001;
const CFM_FACE: u32 = 0x2000_0000;
const CFM_SIZE: u32 = 0x8000_0000;
const PFM_STARTINDENT: u32 = 0x0000_0001;
const PFM_OFFSET: u32 = 0x0000_0004;
const PFM_NUMBERING: u32 = 0x0000_0020;
const PFM_SPACEBEFORE: u32 = 0x0000_0040;
const PFM_SPACEAFTER: u32 = 0x0000_0080;
const PFM_NUMBERINGTAB: u32 = 0x0000_4000;
const PFN_BULLET: u16 = 1;

/// PARAFORMAT2 of the rich-text control (188 bytes).
#[repr(C)]
struct ParaFormat2 {
    cb_size: u32,
    mask: u32,
    numbering: u16,
    effects: u16,
    start_indent: i32,
    right_indent: i32,
    offset: i32,
    alignment: u16,
    tab_count: i16,
    tabs: [i32; 32],
    space_before: i32,
    space_after: i32,
    line_spacing: i32,
    style: i16,
    line_spacing_rule: u8,
    outline_level: u8,
    shading_weight: u16,
    shading_style: u16,
    numbering_start: u16,
    numbering_style: u16,
    numbering_tab: u16,
    border_space: u16,
    border_width: u16,
    borders: u16,
}

/// Paragraph layout in twips (1/1440 inch).
struct Layout {
    start: i32,
    offset: i32,
    before: i32,
    after: i32,
    bullet: bool,
}

fn layout(kind: Kind) -> Layout {
    let l = |start, offset, before, after, bullet| Layout { start, offset, before, after, bullet };
    match kind {
        Kind::Title => l(0, 0, 0, 160, false),
        Kind::Section | Kind::WarnHeading => l(0, 0, 320, 100, false),
        Kind::Page => l(0, 0, 220, 80, false),
        Kind::Body | Kind::Tip | Kind::Warning => l(0, 0, 0, 100, false),
        Kind::Indented => l(220, 0, 0, 100, false),
        Kind::Step => l(0, 300, 0, 60, false),
        Kind::Bullet => l(120, 240, 0, 60, true),
        Kind::SubBullet => l(420, 240, 0, 60, true),
        Kind::Code => l(480, 0, 0, 20, false),
    }
}

fn para(h: HWND, kind: Kind) {
    let lay = layout(kind);
    // SAFETY: plain data, all-zero is valid for every field.
    let mut p: ParaFormat2 = unsafe { std::mem::zeroed() };
    p.cb_size = std::mem::size_of::<ParaFormat2>() as u32;
    p.mask = PFM_STARTINDENT | PFM_OFFSET | PFM_SPACEBEFORE | PFM_SPACEAFTER | PFM_NUMBERING | PFM_NUMBERINGTAB;
    p.start_indent = lay.start;
    p.offset = lay.offset;
    p.space_before = lay.before;
    p.space_after = lay.after;
    if lay.bullet {
        p.numbering = PFN_BULLET;
        p.numbering_tab = lay.offset as u16;
    }
    // SAFETY: the structure outlives the call.
    unsafe { SendMessageW(h, EM_SETPARAFORMAT, 0, &p as *const ParaFormat2 as LPARAM) };
}

fn face(f: &mut CharFormat2W, name: &str) {
    for (i, u) in name.encode_utf16().take(31).enumerate() {
        f.face[i] = u;
    }
}

/// Character format: size in twips (1/20 point), bold, colour, font.
fn chars(size: i32, bold: bool, color: u32, mono: bool) -> CharFormat2W {
    let mut f = format(CFM_COLOR | CFM_BOLD | CFM_SIZE | CFM_FACE, if bold { CFE_BOLD } else { 0 }, color, 0);
    f.height = size;
    face(&mut f, if mono { "Consolas" } else { "Segoe UI" });
    f
}

fn put(h: HWND, text: &str, f: &CharFormat2W) {
    if text.is_empty() {
        return;
    }
    let w = super::tray::wide(text);
    // SAFETY: plain messages to a rich-text control of this thread; the buffers outlive the calls.
    unsafe {
        SendMessageW(h, EM_SETSEL, usize::MAX as WPARAM, -1);
        SendMessageW(h, EM_SETCHARFORMAT, SCF_SELECTION, f as *const CharFormat2W as LPARAM);
        SendMessageW(h, EM_REPLACESEL, 0, w.as_ptr() as LPARAM);
    }
}

/// Splits `text` after the first colon when it is within the first 70 characters (the name of an item).
fn split_label(text: &str) -> (&str, &str) {
    match text.char_indices().take(70).find(|(_, c)| *c == ':') {
        Some((i, _)) => text.split_at(i + 1),
        None => ("", text),
    }
}

/// Writes one block with its formatting.
fn block(h: HWND, b: &Block, colors: &Colors) {
    let text = crate::i18n::tr(&b.key);
    para(h, b.kind);
    let (body, size) = (colors.text, 180);
    match b.kind {
        Kind::Title => put(h, &text, &chars(320, true, colors.accent, false)),
        Kind::Section => put(h, &text, &chars(250, true, colors.accent, false)),
        Kind::WarnHeading => put(h, &text, &chars(250, true, colors.warn, false)),
        Kind::Page => put(h, &text, &chars(210, true, colors.accent, false)),
        Kind::Body | Kind::Indented | Kind::Step => put(h, &text, &chars(size, false, body, false)),
        Kind::Bullet | Kind::SubBullet => {
            let t = text.strip_prefix("- ").unwrap_or(&text);
            let (label, rest) = split_label(t);
            put(h, label, &chars(size, true, body, false));
            put(h, rest, &chars(size, false, body, false));
        }
        Kind::Warning => {
            let (label, rest) = split_label(&text);
            put(h, label, &chars(size, true, colors.warn, false));
            put(h, rest, &chars(size, false, body, false));
        }
        Kind::Tip => {
            let (label, rest) = split_label(&text);
            put(h, label, &chars(size, true, colors.accent, false));
            put(h, rest, &chars(size, false, body, false));
        }
        Kind::Code => {
            // the token (up to two spaces) in monospace, then its explanation
            let (token, rest) = match text.find("  ") {
                Some(i) => text.split_at(i),
                None => (text.as_str(), ""),
            };
            put(h, token, &chars(170, true, colors.accent, true));
            put(h, rest, &chars(size, false, body, false));
        }
    }
    put(h, "\r", &chars(size, false, body, false));
}

struct Colors {
    text: u32,
    accent: u32,
    warn: u32,
}

/// Turns the control into the read-only help and fills it in the language and colours in use.
pub fn fill(h: HWND) {
    let p = theme::palette();
    let colors = Colors {
        text: p.edit_text,
        accent: p.accent,
        // a red that reads on light and on dark backgrounds
        warn: if p.dark { 0x0080_8AFF } else { 0x0020_00B0 },
    };
    // SAFETY: plain messages to a rich-text control of this thread.
    unsafe {
        SendMessageW(h, EM_SETREADONLY, 0, 0); // writable while it is filled
        SendMessageW(h, EM_SETBKGNDCOLOR, 0, p.edit_bg as LPARAM);
        SendMessageW(h, EM_SETSEL, 0, -1);
        SendMessageW(h, EM_REPLACESEL, 0, super::tray::wide("").as_ptr() as LPARAM);
    }
    for b in help::blocks() {
        block(h, &b, &colors);
    }
    // SAFETY: as above.
    unsafe {
        SendMessageW(h, EM_SETSEL, 0, 0);
        SendMessageW(h, EM_SCROLLCARET, 0, 0);
        SendMessageW(h, EM_SETREADONLY, 1, 0);
    }
}
