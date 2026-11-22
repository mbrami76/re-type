// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Re:Type: types the clipboard text into the foreground window as keystrokes.
//!
//! Everything here except `platform` is independent of the operating system and is
//! unit-tested on any host (REQ-COMP-001, REQ-COMP-002).

pub mod about;
pub mod actions;
pub mod config;
pub mod help;
pub mod highlight;
pub mod i18n;
pub mod hotkey;
pub mod log;
pub mod pattern;
pub mod platform;
pub mod safechars;
pub mod secure;
pub mod typing;

/// Product name shown in titles, notices and the start-up entry.
pub const PRODUCT_NAME: &str = "Re:Type";

pub const APP_DIR: &str = "re-type";
