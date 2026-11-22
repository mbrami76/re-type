// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Re:Type entry point.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    re_type::platform::windows::run();
    #[cfg(not(windows))]
    eprintln!("Re:Type runs on Windows only (Linux support is planned).");
}
