// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Operating-system layer. Only Windows exists today (REQ-COMP-001); a Linux backend for KDE and
//! GNOME (REQ-COMP-002) would implement `typing::Backend` plus its own tray and hotkey code.

#[cfg(windows)]
pub mod windows;
