// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! "Safe characters" conversion of clipboard text before it is typed (REQ-FUNC-007).
//!
//! * `Off`: the text is typed as it is.
//! * `Typographic`: smart quotes, dashes, the ellipsis, non-breaking and other special spaces
//!   become plain ASCII; invisible formatting characters (zero-width, bidirectional embeddings, overrides and isolates,
//!   soft hyphen) are removed; Unicode line and paragraph separators become line breaks.
//! * `Ascii`: as `Typographic`, then every remaining non-ASCII character is replaced by an
//!   ASCII equivalent (accents removed, `EUR` for the euro sign, ...) or by `?` when there is
//!   none. Combining marks are removed.
//!
//! The conversion works on UTF-16 units in place of the typing buffer and allocates nothing
//! (REQ-SEC-003).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SafeMode {
    Off,
    Typographic,
    Ascii,
}

impl SafeMode {
    pub const ALL: [SafeMode; 3] = [Self::Off, Self::Typographic, Self::Ascii];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Typographic => "typographic",
            Self::Ascii => "ascii",
        }
    }
    pub fn parse_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == s)
    }
}

/// Result of converting one character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mapped {
    /// Keep this UTF-16 unit (it may be a different one from the input).
    Unit(u16),
    /// Type this ASCII text instead.
    Str(&'static str),
    /// Type nothing.
    Drop,
}

const LATIN1: [&str; 64] = ["A", "A", "A", "A", "A", "A", "AE", "C", "E", "E", "E", "E", "I", "I", "I", "I", "D", "N", "O", "O", "O", "O", "O", "x", "O", "U", "U", "U", "U", "Y", "TH", "ss", "a", "a", "a", "a", "a", "a", "ae", "c", "e", "e", "e", "e", "i", "i", "i", "i", "d", "n", "o", "o", "o", "o", "o", "/", "o", "u", "u", "u", "u", "y", "th", "y"];
const LATIN_A: [&str; 128] = ["A", "a", "A", "a", "A", "a", "C", "c", "C", "c", "C", "c", "C", "c", "D", "d", "D", "d", "E", "e", "E", "e", "E", "e", "E", "e", "E", "e", "G", "g", "G", "g", "G", "g", "G", "g", "H", "h", "H", "h", "I", "i", "I", "i", "I", "i", "I", "i", "I", "i", "IJ", "ij", "J", "j", "K", "k", "k", "L", "l", "L", "l", "L", "l", "L", "l", "L", "l", "N", "n", "N", "n", "N", "n", "n", "N", "n", "O", "o", "O", "o", "O", "o", "OE", "oe", "R", "r", "R", "r", "R", "r", "S", "s", "S", "s", "S", "s", "S", "s", "T", "t", "T", "t", "T", "t", "U", "u", "U", "u", "U", "u", "U", "u", "U", "u", "U", "u", "W", "w", "Y", "y", "Y", "Z", "z", "Z", "z", "Z", "z", "s"];

fn is_high(u: u16) -> bool {
    (0xD800..0xDC00).contains(&u)
}
fn is_low(u: u16) -> bool {
    (0xDC00..0xE000).contains(&u)
}

/// Punctuation and whitespace clean-up shared by both modes.
fn typographic(u: u16) -> Option<Mapped> {
    Some(match u {
        0x2018 | 0x2019 | 0x201A | 0x201B | 0x2032 => Mapped::Str("'"),
        0x201C | 0x201D | 0x201E | 0x201F | 0x2033 | 0xAB | 0xBB => Mapped::Str("\""),
        0x2010..=0x2015 | 0x2212 => Mapped::Str("-"),
        0x2026 => Mapped::Str("..."),
        0x2022 | 0x25CF | 0x25E6 => Mapped::Str("*"),
        0xA0 | 0x1680 | 0x2000..=0x200A | 0x202F | 0x205F | 0x3000 => Mapped::Unit(0x20),
        0xAD | 0x180E | 0x200B..=0x200D | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x2069 | 0xFEFF => {
            Mapped::Drop
        }
        0x85 | 0x2028 | 0x2029 => Mapped::Unit(0x0A),
        _ => return None,
    })
}

fn ascii_equivalent(u: u16) -> Mapped {
    match u {
        0x300..=0x36F => Mapped::Drop, // combining marks
        0xC0..=0xFF => Mapped::Str(LATIN1[(u - 0xC0) as usize]),
        0x100..=0x17F => Mapped::Str(LATIN_A[(u - 0x100) as usize]),
        0x20AC => Mapped::Str("EUR"),
        0xA3 => Mapped::Str("GBP"),
        0xA5 => Mapped::Str("JPY"),
        0xA2 => Mapped::Str("c"),
        0xA9 => Mapped::Str("(c)"),
        0xAE => Mapped::Str("(r)"),
        0x2122 => Mapped::Str("(tm)"),
        0xB0 => Mapped::Str("deg"),
        0xB1 => Mapped::Str("+/-"),
        0xB5 => Mapped::Str("u"),
        0xB2 => Mapped::Str("2"),
        0xB3 => Mapped::Str("3"),
        0xB9 => Mapped::Str("1"),
        0xBC => Mapped::Str("1/4"),
        0xBD => Mapped::Str("1/2"),
        0xBE => Mapped::Str("3/4"),
        0xAA => Mapped::Str("a"),
        0xBA => Mapped::Str("o"),
        0xA1 => Mapped::Str("!"),
        0xBF => Mapped::Str("?"),
        0xA6 => Mapped::Str("|"),
        0xB7 => Mapped::Str("."),
        _ => Mapped::Str("?"),
    }
}

/// Converts the character at `units[*pos]` and advances `*pos` past it.
pub fn map(mode: SafeMode, units: &[u16], pos: &mut usize, end: usize) -> Mapped {
    let u = units[*pos];
    *pos += 1;
    if mode == SafeMode::Off {
        return Mapped::Unit(u);
    }
    if let Some(m) = typographic(u) {
        return m;
    }
    if mode == SafeMode::Typographic {
        return Mapped::Unit(u); // includes surrogate halves: emoji pass through untouched
    }
    // ASCII mode
    if u < 0x80 {
        return Mapped::Unit(u);
    }
    if is_high(u) {
        if *pos < end && is_low(units[*pos]) {
            *pos += 1; // one character outside the BMP (emoji, ...) becomes one '?'
        }
        return Mapped::Str("?");
    }
    if is_low(u) {
        return Mapped::Drop; // broken pair
    }
    ascii_equivalent(u)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `map` over the whole text and returns what would be typed.
    fn run(mode: SafeMode, s: &str) -> String {
        let units: Vec<u16> = s.encode_utf16().collect();
        let (mut pos, mut out) = (0, Vec::<u16>::new());
        while pos < units.len() {
            match map(mode, &units, &mut pos, units.len()) {
                Mapped::Unit(u) => out.push(u),
                Mapped::Str(t) => out.extend(t.encode_utf16()),
                Mapped::Drop => {}
            }
        }
        String::from_utf16(&out).unwrap()
    }

    #[test]
    fn off_changes_nothing() {
        let s = "\u{201C}caf\u{E9}\u{201D} \u{20AC} \u{1F600}";
        assert_eq!(run(SafeMode::Off, s), s);
    }

    #[test]
    fn typographic_cleans_punctuation_and_keeps_letters() {
        assert_eq!(
            run(SafeMode::Typographic, "\u{201C}l\u{2019}ora\u{201D} \u{2013} 5\u{A0}\u{20AC}\u{2026} \u{2022}x"),
            "\"l'ora\" - 5 \u{20AC}... *x"
        );
        assert_eq!(run(SafeMode::Typographic, "a\u{200B}b\u{FEFF}c\u{AD}d\u{202E}e"), "abcde");
        // the left-to-right and right-to-left marks are part of right-to-left text and are kept
        assert_eq!(run(SafeMode::Typographic, "a\u{200F}b\u{200E}c"), "a\u{200F}b\u{200E}c");
        assert_eq!(run(SafeMode::Typographic, "\u{5E9}\u{5DC}\u{5D5}\u{5DD} \u{645}\u{631}\u{62D}\u{628}\u{627}"), "\u{5E9}\u{5DC}\u{5D5}\u{5DD} \u{645}\u{631}\u{62D}\u{628}\u{627}");
        assert_eq!(run(SafeMode::Typographic, "a\u{2028}b\u{85}c"), "a\nb\nc");
        assert_eq!(run(SafeMode::Typographic, "perch\u{E9} \u{1F600}"), "perch\u{E9} \u{1F600}");
    }

    #[test]
    fn ascii_transliterates_and_marks_the_rest() {
        assert_eq!(run(SafeMode::Ascii, "perch\u{E9} \u{E0}\u{E8}\u{EC}\u{F2}\u{F9} \u{C0}\u{C9}"), "perche aeiou AE");
        assert_eq!(run(SafeMode::Ascii, "\u{20AC}5 \u{A3}3 \u{A9} \u{2122} \u{BD}"), "EUR5 GBP3 (c) (tm) 1/2");
        assert_eq!(run(SafeMode::Ascii, "\u{DF}\u{E6}\u{153} \u{141}\u{F3}d\u{17A} \u{15E}"), "ssaeoe Lodz S");
        assert_eq!(run(SafeMode::Ascii, "e\u{301}"), "e"); // decomposed accent
        assert_eq!(run(SafeMode::Ascii, "ok \u{1F600}\u{1F600} \u{43F}\u{440}"), "ok ?? ??"); // emoji, Cyrillic
        assert_eq!(run(SafeMode::Ascii, "\u{201C}x\u{201D}"), "\"x\"");
    }

    #[test]
    fn lone_surrogates_do_not_panic() {
        let units = [0xD83Du16, 0x61, 0xDE00];
        for mode in SafeMode::ALL {
            let mut pos = 0;
            while pos < units.len() {
                let _ = map(mode, &units, &mut pos, units.len());
            }
        }
    }

    #[test]
    fn every_unit_value_is_handled() {
        for mode in SafeMode::ALL {
            for u in 0..=0xFFFFu16 {
                let units = [u, 0x61];
                let mut pos = 0;
                let _ = map(mode, &units, &mut pos, 2);
                assert!(pos >= 1);
            }
        }
    }

    #[test]
    fn ascii_mode_output_is_ascii() {
        for u in 0x80..=0xFFFFu16 {
            if (0xD800..0xE000).contains(&u) {
                continue;
            }
            let units = [u];
            let mut pos = 0;
            match map(SafeMode::Ascii, &units, &mut pos, 1) {
                Mapped::Unit(x) => assert!(x < 0x80, "U+{u:04X} -> unit {x:#x}"),
                Mapped::Str(t) => assert!(t.is_ascii(), "U+{u:04X} -> {t}"),
                Mapped::Drop => {}
            }
        }
    }
}
