// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Keystroke injection and key state (REQ-FUNC-001, REQ-FUNC-004, REQ-UX-004, REQ-SAFE-001).

use crate::actions::Action;
use crate::hotkey::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, VK_ESCAPE};
use crate::secure::SecureBuf;
use crate::typing::{Backend, ClipboardRead};
use std::time::{Duration, Instant};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_CONTROL, VK_LWIN, VK_MENU,
    VK_RWIN, VK_SHIFT,
};

/// An unassigned virtual key. Tapping it while Alt or Win is held stops the later release of
/// that key from opening a menu or the Start menu.
const VK_NEUTRAL: u16 = 0xE8;

const EXTENDED: [u16; 14] =
    [0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2C, 0x2D, 0x2E, 0x5B, 0x5C, 0x5D];

pub struct WinBackend;

const CLIPBOARD_TIMEOUT_MS: u64 = 4000;

fn key_event(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn vk_event(vk: u16, up: bool) -> INPUT {
    let mut flags = if up { KEYEVENTF_KEYUP } else { 0 };
    if EXTENDED.contains(&vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    // SAFETY: plain lookup.
    let scan = unsafe { MapVirtualKeyW(u32::from(vk), 0) } as u16;
    key_event(vk, scan, flags)
}

fn send_all(events: &[INPUT]) -> Result<(), String> {
    // SAFETY: `events` is a valid array of INPUT structures of the size passed.
    let sent = unsafe { SendInput(events.len() as u32, events.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    if sent as usize == events.len() {
        Ok(())
    } else {
        Err("Windows refused the keystrokes. The target window may be running as administrator.".into())
    }
}

fn is_down(vk: u16) -> bool {
    // SAFETY: plain query.
    unsafe { GetAsyncKeyState(i32::from(vk)) as u16 & 0x8000 != 0 }
}

const MOD_KEYS: [(u32, u16); 4] =
    [(MOD_CONTROL, VK_CONTROL), (MOD_ALT, VK_MENU), (MOD_SHIFT, VK_SHIFT), (MOD_WIN, VK_LWIN)];

impl Backend for WinBackend {
    /// The read runs on its own thread with a time limit: a program that renders its clipboard
    /// data on demand can block indefinitely, and that must not hang Re:Type (REQ-FUNC-008).
    /// A late result is wiped when the abandoned thread drops its buffer (REQ-SEC-003).
    fn read_clipboard(&self, buf: &mut SecureBuf) -> ClipboardRead {
        let capacity = buf.capacity();
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new().name("clipboard".into()).spawn(move || {
            let mut tmp = SecureBuf::with_capacity(capacity);
            let result = super::clipboard::read(&mut tmp);
            let _ = tx.send((result, tmp)); // if nobody waits any more, `tmp` is dropped (wiped) here
        });
        if spawned.is_err() {
            return ClipboardRead::Busy;
        }
        match rx.recv_timeout(Duration::from_millis(CLIPBOARD_TIMEOUT_MS)) {
            Ok((result, tmp)) => {
                super::clipboard::remember_read();
                if matches!(result, ClipboardRead::Text | ClipboardRead::Files) && buf.fill_from(tmp.as_units()).is_err() {
                    return ClipboardRead::TooLong;
                }
                result
            }
            Err(_) => ClipboardRead::Busy,
        }
    }

    fn send(&self, action: &Action) -> Result<(), String> {
        match *action {
            Action::Unit(u) => send_all(&[
                key_event(0, u, KEYEVENTF_UNICODE),
                key_event(0, u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
            ]),
            Action::Key { mods, vk } => {
                let held: Vec<u16> = MOD_KEYS.iter().filter(|(f, _)| mods & f != 0).map(|(_, k)| *k).collect();
                let mut ev: Vec<INPUT> = held.iter().map(|k| vk_event(*k, false)).collect();
                ev.push(vk_event(vk as u16, false));
                ev.push(vk_event(vk as u16, true));
                ev.extend(held.iter().rev().map(|k| vk_event(*k, true)));
                send_all(&ev)
            }
            Action::DelayMs(ms) => {
                self.sleep_ms(ms);
                Ok(())
            }
        }
    }

    fn clear_clipboard(&self) -> bool {
        super::clipboard::clear_if_unchanged()
    }

    fn escape_pressed(&self) -> bool {
        is_down(VK_ESCAPE as u16)
    }

    fn sleep_ms(&self, ms: u32) {
        if ms > 0 {
            std::thread::sleep(Duration::from_millis(u64::from(ms)));
        }
    }

    fn release_modifiers(&self, hotkey_mods: u32, timeout_ms: u32) {
        if hotkey_mods & (MOD_ALT | MOD_WIN) != 0 {
            let _ = send_all(&[key_event(VK_NEUTRAL, 0, 0), key_event(VK_NEUTRAL, 0, KEYEVENTF_KEYUP)]);
        }
        let keys = [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN];
        let end = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
        while Instant::now() < end {
            if !keys.iter().any(|k| is_down(*k)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // the user is still holding them: release them logically so typing is not turned into shortcuts
        let stuck: Vec<INPUT> = keys.iter().filter(|k| is_down(**k)).map(|k| vk_event(*k, true)).collect();
        if !stuck.is_empty() {
            let _ = send_all(&stuck);
        }
    }
}
