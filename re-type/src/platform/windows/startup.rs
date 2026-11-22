// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Start with Windows (REQ-UX-009): a per-user `Run` value, plus the Task Manager switch.
//!
//! Only the current user's registry is touched, so no administrator rights are needed. Elevated
//! start is deliberately NOT done here (no scheduled task with highest privileges): see DC-026.

use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
};

const RUN: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const APPROVED: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn value_name() -> String {
    crate::PRODUCT_NAME.to_string()
}

/// The command line that is stored: the quoted path of this program.
fn command() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot find the program file: {e}"))?;
    Ok(format!("\"{}\"", exe.display()))
}

fn open(path: &str, access: u32, create: bool) -> Option<HKEY> {
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: plain registry calls with NUL-terminated names; the key is closed by the caller.
    let rc = unsafe {
        if create {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(path).as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                access,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        } else {
            RegOpenKeyExW(HKEY_CURRENT_USER, wide(path).as_ptr(), 0, access, &mut key)
        }
    };
    (rc == 0).then_some(key)
}

fn read(path: &str, name: &str) -> Option<Vec<u8>> {
    let key = open(path, KEY_QUERY_VALUE, false)?;
    let mut buf = vec![0u8; 4096];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` and `len` describe a valid buffer.
    let rc = unsafe {
        RegQueryValueExW(key, wide(name).as_ptr(), std::ptr::null(), std::ptr::null_mut(), buf.as_mut_ptr(), &mut len)
    };
    // SAFETY: the key was opened above.
    unsafe { RegCloseKey(key) };
    (rc == 0).then(|| {
        buf.truncate(len as usize);
        buf
    })
}

/// The stored command line, if the Run value exists.
pub fn registered_command() -> Option<String> {
    let raw = read(RUN, &value_name())?;
    let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
    Some(String::from_utf16_lossy(&units))
}

/// True when Windows' Task Manager has switched the entry off (the value stays, Windows ignores it).
pub fn disabled_by_windows() -> bool {
    // StartupApproved holds 12 bytes; an odd first byte (03) means disabled, 02 or 06 enabled.
    read(APPROVED, &value_name()).map(|v| v.first().is_some_and(|b| b & 1 == 1)).unwrap_or(false)
}

/// Writes the Run value only.
fn write_run() -> Result<(), String> {
    let cmd = command()?;
    let key = open(RUN, KEY_SET_VALUE, true).ok_or("cannot open the Windows startup list")?;
    let data = wide(&cmd);
    // SAFETY: `data` is a NUL-terminated UTF-16 string; the byte length includes the NUL.
    let rc = unsafe { RegSetValueExW(key, wide(&value_name()).as_ptr(), 0, REG_SZ, data.as_ptr() as *const u8, (data.len() * 2) as u32) };
    // SAFETY: the key was opened above.
    unsafe { RegCloseKey(key) };
    if rc != 0 {
        return Err(format!("Windows refused to write the startup entry (error {rc})"));
    }
    Ok(())
}

fn delete_value(path: &str) {
    if let Some(key) = open(path, KEY_SET_VALUE, false) {
        // SAFETY: deleting our own value; a missing value is fine.
        unsafe {
            RegDeleteValueW(key, wide(&value_name()).as_ptr());
            RegCloseKey(key);
        }
    }
}

/// Switches the entry on or off. On: writes the Run value and clears Task Manager's "disabled"
/// mark, so that the option really means what it says.
pub fn apply(on: bool) -> Result<(), String> {
    if on {
        write_run()?;
        delete_value(APPROVED);
    } else {
        delete_value(RUN);
    }
    Ok(())
}

/// At program start: if the option is on but the entry is missing or points elsewhere (the
/// program was moved), write it again.
pub fn repair(wanted: bool) {
    if !wanted {
        return;
    }
    let ok = match (registered_command(), command()) {
        (Some(have), Ok(want)) => have.eq_ignore_ascii_case(&want),
        _ => false,
    };
    if !ok {
        match write_run() {
            Ok(()) => crate::log::info("start-with-Windows entry repaired"),
            Err(e) => crate::log::warn(&e),
        }
    }
}
