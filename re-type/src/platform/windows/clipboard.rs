// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Clipboard content, copied straight into the wiped-on-drop typing buffer (REQ-FUNC-001,
//! REQ-FUNC-008, REQ-SEC-003). The clipboard memory belongs to Windows; it is only read.
//!
//! Order of preference: Unicode text, then copied files (their paths), then "image" and
//! "other non-text". Windows converts ANSI text to Unicode by itself, so plain `CF_TEXT` is
//! covered by the first case.

use crate::secure::SecureBuf;
use crate::typing::ClipboardRead;
use std::sync::atomic::{AtomicU32, Ordering};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, CountClipboardFormats, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
};
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows_sys::Win32::System::Threading::Sleep;
use windows_sys::Win32::UI::Shell::DragQueryFileW;

const CF_BITMAP: u32 = 2;
const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;
const CF_DIBV5: u32 = 17;
const LF: u16 = 0x0A;

/// Sequence number of the clipboard content that was last read for typing.
static LAST_READ: AtomicU32 = AtomicU32::new(0);

/// Notes which clipboard content was just read.
pub fn remember_read() {
    // SAFETY: plain query.
    LAST_READ.store(unsafe { GetClipboardSequenceNumber() }, Ordering::SeqCst);
}

/// Empties the clipboard if it still holds the content that was read; anything copied since
/// is left alone. Returns whether the clipboard was emptied (REQ-SEC-005).
pub fn clear_if_unchanged() -> bool {
    // SAFETY: plain Win32 calls; the clipboard is closed on every path.
    unsafe {
        let seq = LAST_READ.load(Ordering::SeqCst);
        if seq == 0 || GetClipboardSequenceNumber() != seq {
            crate::log::info("clipboard not cleared: it changed during typing");
            return false;
        }
        for _ in 0..20 {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                // check again now that the clipboard is held: nobody can change it any more
                let same = GetClipboardSequenceNumber() == seq;
                let emptied = same && EmptyClipboard() != 0;
                CloseClipboard();
                crate::log::info(if emptied { "clipboard cleared after typing" } else { "clipboard could not be cleared" });
                return emptied;
            }
            Sleep(25);
        }
        crate::log::info("clipboard could not be cleared: it is in use");
        false
    }
}

pub fn read(buf: &mut SecureBuf) -> ClipboardRead {
    // SAFETY: plain Win32 calls; the clipboard is closed on every path below, and the locked
    // memory is read only up to the size Windows reports for it.
    unsafe {
        let mut opened = false;
        for _ in 0..20 {
            // another program may hold the clipboard for a moment
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                opened = true;
                break;
            }
            Sleep(25);
        }
        if !opened {
            return ClipboardRead::Busy;
        }
        let result = read_open(buf);
        CloseClipboard();
        result
    }
}

unsafe fn read_open(buf: &mut SecureBuf) -> ClipboardRead {
    if IsClipboardFormatAvailable(CF_UNICODETEXT) != 0 {
        return read_text(buf);
    }
    if IsClipboardFormatAvailable(CF_HDROP) != 0 {
        return read_files(buf);
    }
    let png = RegisterClipboardFormatW(crate::platform::windows::tray::wide("PNG").as_ptr());
    if [CF_DIB, CF_DIBV5, CF_BITMAP, png].iter().any(|f| *f != 0 && IsClipboardFormatAvailable(*f) != 0) {
        return ClipboardRead::Image;
    }
    if CountClipboardFormats() == 0 {
        ClipboardRead::NoText
    } else {
        ClipboardRead::Other
    }
}

unsafe fn read_text(buf: &mut SecureBuf) -> ClipboardRead {
    let handle = GetClipboardData(CF_UNICODETEXT);
    if handle.is_null() {
        return ClipboardRead::NoText;
    }
    let p = GlobalLock(handle) as *const u16;
    if p.is_null() {
        return ClipboardRead::Busy;
    }
    let limit = GlobalSize(handle) / 2;
    let cap = buf.capacity();
    let mut n = 0usize;
    while n < limit && *p.add(n) != 0 {
        n += 1;
        if n > cap {
            break;
        }
    }
    let result = if n > cap {
        ClipboardRead::TooLong
    } else if n == 0 {
        ClipboardRead::NoText
    } else {
        std::ptr::copy_nonoverlapping(p, buf.storage_mut().as_mut_ptr(), n);
        buf.set_len(n);
        ClipboardRead::Text
    };
    GlobalUnlock(handle);
    result
}

/// Copied files (Explorer, other file managers): their full paths, separated by line feeds.
unsafe fn read_files(buf: &mut SecureBuf) -> ClipboardRead {
    let drop = GetClipboardData(CF_HDROP);
    if drop.is_null() {
        return ClipboardRead::Other;
    }
    let count = DragQueryFileW(drop, u32::MAX, std::ptr::null_mut(), 0);
    let cap = buf.capacity();
    let mut used = 0usize;
    for i in 0..count {
        let len = DragQueryFileW(drop, i, std::ptr::null_mut(), 0) as usize;
        if len == 0 {
            continue;
        }
        let sep = usize::from(used > 0);
        // the API writes a terminating NUL after the name, so one more unit must fit
        if used + sep + len + 1 > cap {
            buf.clear();
            return ClipboardRead::TooLong;
        }
        let storage = buf.storage_mut();
        if sep == 1 {
            storage[used] = LF;
            used += 1;
        }
        let written = DragQueryFileW(drop, i, storage[used..].as_mut_ptr(), (len + 1) as u32) as usize;
        used += written.min(len);
    }
    if used == 0 {
        return ClipboardRead::Other;
    }
    buf.set_len(used);
    ClipboardRead::Files
}
