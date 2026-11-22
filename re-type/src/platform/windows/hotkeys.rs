// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Global hotkey registration and conflict probing (REQ-FUNC-002, REQ-FUNC-003, REQ-FUNC-006).

use crate::hotkey::Hotkey;
use windows_sys::Win32::Foundation::{GetLastError, HWND};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey};

const MOD_NOREPEAT: u32 = 0x4000;
const ERROR_HOTKEY_ALREADY_REGISTERED: u32 = 1409;
const PROBE_ID: i32 = 0xBFFF;

fn last_error() -> String {
    // SAFETY: plain query.
    match unsafe { GetLastError() } {
        ERROR_HOTKEY_ALREADY_REGISTERED => "already registered by another program".into(),
        code => format!("Windows refused it (error {code})"),
    }
}

pub fn register(hwnd: HWND, id: u32, hk: Hotkey) -> Result<(), String> {
    // SAFETY: `hwnd` is the main window, owned by the calling thread.
    if unsafe { RegisterHotKey(hwnd, id as i32, hk.mods | MOD_NOREPEAT, hk.vk) } != 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn unregister(hwnd: HWND, id: u32) {
    // SAFETY: see `register`.
    unsafe { UnregisterHotKey(hwnd, id as i32) };
}

/// Registers and immediately releases `hk` to find out whether it is free. A hotkey this
/// application already holds must be skipped by the caller.
pub fn probe(hwnd: HWND, hk: Hotkey) -> Result<(), String> {
    // SAFETY: see `register`.
    unsafe {
        if RegisterHotKey(hwnd, PROBE_ID, hk.mods | MOD_NOREPEAT, hk.vk) != 0 {
            UnregisterHotKey(hwnd, PROBE_ID);
            Ok(())
        } else {
            Err(last_error())
        }
    }
}
