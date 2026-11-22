// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Key names and hotkey parsing (REQ-FUNC-002, REQ-FUNC-003, REQ-FUNC-006).
//!
//! Modifier flag values equal the Windows `RegisterHotKey` ones so the Windows layer can pass
//! them through unchanged.

use std::fmt;

pub const MOD_ALT: u32 = 0x1;
pub const MOD_CONTROL: u32 = 0x2;
pub const MOD_SHIFT: u32 = 0x4;
pub const MOD_WIN: u32 = 0x8;

pub const VK_BACK: u32 = 0x08;
pub const VK_TAB: u32 = 0x09;
pub const VK_RETURN: u32 = 0x0D;
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_SPACE: u32 = 0x20;

const MOD_ORDER: [(&str, u32); 4] =
    [("ctrl", MOD_CONTROL), ("win", MOD_WIN), ("alt", MOD_ALT), ("shift", MOD_SHIFT)];

const NAMED: &[(&str, u32)] = &[
    ("backspace", 0x08), ("tab", 0x09), ("enter", 0x0D), ("pause", 0x13), ("capslock", 0x14),
    ("esc", 0x1B), ("space", 0x20), ("pgup", 0x21), ("pgdn", 0x22), ("end", 0x23),
    ("home", 0x24), ("left", 0x25), ("up", 0x26), ("right", 0x27), ("down", 0x28),
    ("printscreen", 0x2C), ("ins", 0x2D), ("del", 0x2E), ("menu", 0x5D),
    ("numlock", 0x90), ("scrolllock", 0x91),
    ("plus", 0xBB), ("comma", 0xBC), ("minus", 0xBD), ("period", 0xBE),
    // aliases (parsed, never printed: the first entry for a code is its display name)
    ("bksp", 0x08), ("return", 0x0D), ("escape", 0x1B), ("pageup", 0x21),
    ("pagedown", 0x22), ("insert", 0x2D), ("delete", 0x2E), ("apps", 0x5D),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ParseError {}

fn err<T>(s: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError(s.into()))
}

/// Virtual-key code for a key name such as `v`, `7`, `tab`, `f5`, `num3`.
pub fn key_to_vk(name: &str) -> Result<u32, ParseError> {
    let n = name.trim().to_ascii_lowercase();
    let b = n.as_bytes();
    if b.len() == 1 && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit()) {
        return Ok(b[0].to_ascii_uppercase() as u32);
    }
    if let Some((_, vk)) = NAMED.iter().find(|(k, _)| *k == n) {
        return Ok(*vk);
    }
    if let Some(num) = n.strip_prefix('f').and_then(|d| d.parse::<u32>().ok()) {
        if (1..=24).contains(&num) {
            return Ok(0x6F + num);
        }
    }
    if let Some(num) = n.strip_prefix("num").and_then(|d| d.parse::<u32>().ok()) {
        if num <= 9 {
            return Ok(0x60 + num);
        }
    }
    err(format!("unknown key name: '{name}'"))
}

pub fn vk_to_name(vk: u32) -> String {
    if (0x30..=0x39).contains(&vk) || (0x41..=0x5A).contains(&vk) {
        return (vk as u8 as char).to_ascii_lowercase().to_string();
    }
    if (0x70..=0x87).contains(&vk) {
        return format!("f{}", vk - 0x6F);
    }
    if (0x60..=0x69).contains(&vk) {
        return format!("num{}", vk - 0x60);
    }
    NAMED.iter().find(|(_, v)| *v == vk).map(|(k, _)| k.to_string()).unwrap_or_else(|| format!("vk{vk:02x}"))
}

/// Parses `ctrl+shift+tab` into (modifier flags, virtual key). Modifiers are optional.
pub fn parse_combo(text: &str) -> Result<(u32, u32), ParseError> {
    let parts: Vec<String> = text.split('+').map(|p| p.trim().to_ascii_lowercase()).collect();
    if parts.iter().any(|p| p.is_empty()) {
        return err(format!("malformed key combination '{text}' (write 'plus' for the + key)"));
    }
    let (key, mods) = parts.split_last().expect("split on '+' yields at least one part");
    let mut flags = 0;
    for m in mods {
        flags |= match m.as_str() {
            "ctrl" | "control" => MOD_CONTROL,
            "alt" => MOD_ALT,
            "shift" => MOD_SHIFT,
            "win" => MOD_WIN,
            _ => return err(format!("unknown modifier '{m}'")),
        };
    }
    if ["ctrl", "control", "alt", "shift", "win"].contains(&key.as_str()) {
        return err(format!("'{text}' must end with a key that is not a modifier"));
    }
    Ok((flags, key_to_vk(key)?))
}

/// Names of keys that have a name in the user's language (REQ-I18N-004): the four modifiers and
/// the named keys. Letters, digits, function keys and `numN` are the same everywhere.
pub fn localizable() -> Vec<&'static str> {
    let mut v = vec!["ctrl", "win", "alt", "shift"];
    for (name, vk) in NAMED {
        // aliases come after the display name of their code and are skipped
        if NAMED.iter().find(|(_, c)| c == vk).map(|(k, _)| k) == Some(name) {
            v.push(name);
        }
    }
    v
}

/// The catalogue key under which the name of `canonical` is translated.
pub fn name_key(canonical: &str) -> String {
    format!("Key name: {}", canonical.to_uppercase())
}

/// The catalogue keys of all translatable key names.
pub fn name_keys() -> Vec<String> {
    localizable().iter().map(|n| name_key(n)).collect()
}

/// The English, lower-case name of the key or modifier written as `token` in English or in the
/// language in use (`Strg`, `maiusc`, `Entrée`...); None when it is not a key name at all.
pub fn canonical(token: &str) -> Option<String> {
    let t = token.trim().to_lowercase();
    if t.is_empty() {
        return None;
    }
    match t.as_str() {
        "control" => return Some("ctrl".into()),
        "ctrl" | "win" | "alt" | "shift" => return Some(t),
        _ => {}
    }
    if let Ok(vk) = key_to_vk(&t) {
        return Some(vk_to_name(vk));
    }
    localizable()
        .into_iter()
        .find(|n| crate::i18n::lookup(&name_key(n)).is_some_and(|l| l.to_lowercase() == t))
        .map(str::to_string)
}

/// How a key is written in capitals in the language in use: its translated name, or the English one.
pub fn display_name(canonical: &str) -> String {
    crate::i18n::lookup(&name_key(canonical)).unwrap_or_else(|| canonical.to_string()).to_uppercase()
}

fn map_parts(text: &str, f: impl Fn(&str) -> Option<String>) -> String {
    text.split('+')
        .map(|p| {
            let p = p.trim();
            f(p).unwrap_or_else(|| p.to_string())
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// A shortcut as typed in the user's language, back to the English text that is stored.
pub fn to_english(text: &str) -> String {
    map_parts(text, canonical)
}

/// A stored (English) shortcut as it is shown in the user's language, in capitals.
pub fn to_local(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    map_parts(text, |p| canonical(p).map(|c| display_name(&c)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub mods: u32,
    pub vk: u32,
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (name, flag) in MOD_ORDER {
            if self.mods & flag != 0 {
                write!(f, "{name}+")?;
            }
        }
        f.write_str(&vk_to_name(self.vk))
    }
}

impl Hotkey {
    /// A global hotkey needs at least one modifier, otherwise it would swallow a normal key.
    pub fn parse(text: &str) -> Result<Hotkey, ParseError> {
        let (mods, vk) = parse_combo(text)?;
        if mods == 0 {
            return err("a global hotkey needs at least one modifier (ctrl, alt, shift or win)");
        }
        Ok(Hotkey { mods, vk })
    }
}

/// Candidate hotkeys for `key`: Ctrl plus at least one more modifier, fewest modifiers first
/// (REQ-FUNC-002, REQ-FUNC-003).
pub fn suggestions(key: u32) -> Vec<Hotkey> {
    let extras = [MOD_ALT, MOD_SHIFT, MOD_WIN];
    let mut out = Vec::new();
    for size in 1..=extras.len() {
        for mask in 1u32..(1 << extras.len()) {
            if mask.count_ones() as usize != size {
                continue;
            }
            let mut mods = MOD_CONTROL;
            for (i, e) in extras.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    mods |= e;
                }
            }
            out.push(Hotkey { mods, vk: key });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints_canonically() {
        let h = Hotkey::parse("Shift + Alt+CTRL+V").unwrap();
        assert_eq!(h, Hotkey { mods: MOD_CONTROL | MOD_ALT | MOD_SHIFT, vk: b'V' as u32 });
        assert_eq!(h.to_string(), "ctrl+alt+shift+v");
        let w = Hotkey::parse("shift+alt+win+ctrl+v").unwrap();
        assert_eq!(w.to_string(), "ctrl+win+alt+shift+v");
        assert_eq!(Hotkey::parse(&h.to_string()).unwrap(), h);
    }

    #[test]
    fn rejects_bad_input() {
        for bad in ["v", "ctrl+", "+v", "ctrl+shift", "ctrl+nokey", "hyper+v", "", "ctrl++v"] {
            assert!(Hotkey::parse(bad).is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn key_names() {
        assert_eq!(key_to_vk("F5").unwrap(), 0x74);
        assert_eq!(key_to_vk("num3").unwrap(), 0x63);
        assert_eq!(key_to_vk("Return").unwrap(), VK_RETURN);
        assert_eq!(vk_to_name(VK_RETURN), "enter");
        assert_eq!(vk_to_name(0x74), "f5");
        assert!(key_to_vk("f25").is_err());
        assert_eq!(parse_combo("tab").unwrap(), (0, VK_TAB));
        assert_eq!(parse_combo("ctrl+plus").unwrap(), (MOD_CONTROL, 0xBB));
    }

    #[test]
    fn suggestions_keep_ctrl_and_add_a_modifier() {
        let s = suggestions(b'V' as u32);
        assert_eq!(s.len(), 7);
        assert!(s.iter().all(|h| h.mods & MOD_CONTROL != 0 && h.mods.count_ones() >= 2));
        assert_eq!(s[0].mods.count_ones(), 2);
        assert_eq!(s.last().unwrap().mods.count_ones(), 4);
    }
}
