// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami
//! Which parts of a shortcut or of a template pattern the program recognises (REQ-UX-014,
//! REQ-UX-016). Pure functions: the settings window colours the recognised parts like small keys
//! (inverted colours) and underlines the rest in red.

use crate::{hotkey, pattern};

/// A recognised or unrecognised piece of the text; positions are UTF-16 code units, `end` exclusive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub valid: bool,
    /// The text of the span in capitals, which is what a recognised key is written as. Equal to
    /// the original text when there is nothing to change.
    pub canonical: String,
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn is_modifier(canon: &str) -> bool {
    matches!(canon, "ctrl" | "alt" | "shift" | "win")
}

/// Splits a shortcut such as `ctrl + Alt+v` at the `+` signs and judges every key name. Names are
/// accepted in English and in the language in use, in any letter case; a recognised name is
/// written in capitals in the language in use.
pub fn shortcut_spans(text: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut canon: Vec<Option<String>> = Vec::new();
    let mut pos = 0usize; // utf16 position of the start of the current part
    for part in text.split('+') {
        let plen = utf16_len(part);
        let lead = utf16_len(&part[..part.len() - part.trim_start().len()]);
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            let c = hotkey::canonical(trimmed);
            out.push(Span {
                start: pos + lead,
                end: pos + lead + utf16_len(trimmed),
                valid: c.is_some(),
                canonical: c.as_deref().map(hotkey::display_name).unwrap_or_else(|| trimmed.to_string()),
            });
            canon.push(c);
        }
        pos += plen + 1; // the '+'
    }
    // A global shortcut is modifiers followed by exactly one key, and needs a modifier: a key
    // that stands alone ("HOME"), comes before a modifier or is not the last part is not usable.
    let has_modifier = canon.iter().flatten().any(|c| is_modifier(c));
    let last = out.len().saturating_sub(1);
    for (i, sp) in out.iter_mut().enumerate() {
        if let Some(c) = &canon[i] {
            if !is_modifier(c) && (!has_modifier || i != last) {
                sp.valid = false;
                sp.canonical = text_of(text, sp.start, sp.end);
            }
        }
    }
    out
}

/// The UTF-16 range `start..end` of `text`.
fn text_of(text: &str, start: usize, end: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16_lossy(&units[start..end])
}

/// Puts a shortcut in the fixed order CTRL, WIN, ALT, SHIFT, then the key (REQ-UX-014). Returns the
/// rewritten text only when every part is recognised and the order or repetition needs changing;
/// text that is still being typed (ending in `+`) is left alone.
pub fn ordered_shortcut(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() || t.ends_with('+') || t.starts_with('+') {
        return None;
    }
    let mut canon = Vec::new();
    for p in t.split('+') {
        canon.push(hotkey::canonical(p)?);
    }
    let mut out: Vec<String> = Vec::new();
    for m in ["ctrl", "win", "alt", "shift"] {
        if canon.iter().any(|c| c == m) {
            out.push(hotkey::display_name(m));
        }
    }
    out.extend(canon.iter().filter(|c| !is_modifier(c)).map(|c| hotkey::display_name(c)));
    let joined = out.join("+");
    let current = t.split('+').map(|p| p.trim().to_uppercase()).collect::<Vec<_>>().join("+");
    (joined != current).then_some(joined)
}

/// Finds the `{...}` tokens of a template pattern. `{{` and `}}` are literal braces and are not
/// tokens; an unfinished token is reported as not recognised. With `template` set, tokens that a
/// template may not contain (shortcuts, function keys, long repeats) are not recognised either.
pub fn pattern_spans(text: &str, template: bool) -> Vec<Span> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut pos) = (0usize, 0usize); // char index, utf16 position
    while i < chars.len() {
        let c = chars[i];
        if (c == '{' || c == '}') && chars.get(i + 1) == Some(&c) {
            i += 2;
            pos += 2;
            continue;
        }
        if c == '{' {
            let start = pos;
            let mut body = String::new();
            let mut j = i + 1;
            let mut closed = false;
            while j < chars.len() {
                match chars[j] {
                    '}' => {
                        closed = true;
                        break;
                    }
                    '{' => break,
                    ch => body.push(ch),
                }
                j += 1;
            }
            let end_char = if closed { j + 1 } else { j };
            let whole: String = chars[i..end_char].iter().collect();
            let valid = closed && pattern::token_is_valid(&body, template);
            let end = start + utf16_len(&whole);
            out.push(Span { start, end, valid, canonical: if valid { whole.to_ascii_uppercase() } else { whole } });
            pos = end;
            i = end_char;
            continue;
        }
        pos += c.len_utf16();
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(spans: &[Span]) -> Vec<(usize, usize, bool, String)> {
        spans.iter().map(|s| (s.start, s.end, s.valid, s.canonical.clone())).collect()
    }

    #[test]
    fn shortcut_parts_are_judged_in_any_case_and_become_capitals() {
        let _g = crate::i18n::english_guard();
        let s = shortcut_spans("ctrl+Alt + v");
        assert_eq!(flags(&s), [(0, 4, true, "CTRL".into()), (5, 8, true, "ALT".into()), (11, 12, true, "V".into())]);
        let s = shortcut_spans("Ctrl+bogus+F5");
        assert_eq!(s.iter().map(|x| x.valid).collect::<Vec<_>>(), [true, false, true]);
        assert_eq!(s[1].canonical, "bogus"); // an unrecognised word is left as it was written
        assert_eq!(s[2].canonical, "F5");
    }

    #[test]
    fn a_key_without_modifier_or_out_of_place_is_not_recognised() {
        let _g = crate::i18n::english_guard();
        let s = shortcut_spans("home");
        assert!(!s[0].valid && s[0].canonical == "home");
        let s = shortcut_spans("Ctrl+a+b");
        assert_eq!(s.iter().map(|x| x.valid).collect::<Vec<_>>(), [true, false, true]);
        let s = shortcut_spans("a+ctrl");
        assert_eq!(s.iter().map(|x| x.valid).collect::<Vec<_>>(), [false, true]);
        assert!(shortcut_spans("ctrl+home")[1].valid);
        assert!(shortcut_spans("ctrl").iter().all(|x| x.valid)); // still being typed
    }

    #[test]
    fn shortcuts_are_put_in_the_fixed_order() {
        let _g = crate::i18n::english_guard();
        assert_eq!(ordered_shortcut("a+ctrl").as_deref(), Some("CTRL+A"));
        assert_eq!(ordered_shortcut("shift+alt+win+ctrl+f5").as_deref(), Some("CTRL+WIN+ALT+SHIFT+F5"));
        assert_eq!(ordered_shortcut("ctrl+ctrl+a").as_deref(), Some("CTRL+A"));
        assert_eq!(ordered_shortcut("control+a").as_deref(), Some("CTRL+A"));
        assert_eq!(ordered_shortcut("CTRL+ALT+A"), None); // already right
        assert_eq!(ordered_shortcut("ctrl + alt + a"), None); // order is right; capitals are done per part
        assert_eq!(ordered_shortcut("a+"), None); // still typing
        assert_eq!(ordered_shortcut("a+bogus"), None);
    }

    #[test]
    fn shortcut_edge_cases() {
        let _g = crate::i18n::english_guard();
        assert!(shortcut_spans("").is_empty());
        assert!(shortcut_spans(" + + ").is_empty());
        let s = shortcut_spans("ctrl+");
        assert_eq!(s.len(), 1);
        let s = shortcut_spans("alt+PageDown");
        assert!(s[1].valid && s[1].canonical == "PGDN");
        // a character outside the Basic Latin range is simply not recognised, positions stay right
        let s = shortcut_spans("\u{20AC}+ctrl");
        assert_eq!(flags(&s), [(0, 1, false, "\u{20AC}".into()), (2, 6, true, "CTRL".into())]);
    }

    #[test]
    fn pattern_tokens_are_found_and_judged() {
        let s = pattern_spans("ID-{clipboard}{tab 2}x{bogus}", true);
        assert_eq!(
            flags(&s),
            [(3, 14, true, "{CLIPBOARD}".into()), (14, 21, true, "{TAB 2}".into()), (22, 29, false, "{bogus}".into())]
        );
    }

    #[test]
    fn pattern_literal_braces_and_unfinished_tokens() {
        assert!(pattern_spans("a {{b}} c", false).is_empty());
        let s = pattern_spans("x{ent", false);
        assert_eq!(flags(&s), [(1, 5, false, "{ent".into())]);
        let s = pattern_spans("{tab{enter}", false);
        assert_eq!(s.len(), 2);
        assert!(!s[0].valid && s[1].valid);
        // positions count UTF-16 units: an emoji is two
        let s = pattern_spans("\u{1F600}{tab}", false);
        assert_eq!(s[0].start, 2);
    }

    #[test]
    fn recognition_matches_the_real_parsers() {
        let _g = crate::i18n::english_guard();
        for t in ["ctrl+alt+shift+v", "alt+f5", "ctrl+num3", "shift+esc"] {
            assert!(shortcut_spans(t).iter().all(|s| s.valid), "{t}");
            assert!(hotkey::parse_combo(t).is_ok(), "{t}");
        }
        for t in ["{CLIPBOARD}", "{TAB}", "{DELAY 200}", "{SHIFT+TAB}", "{BACKSPACE 3}"] {
            assert!(pattern_spans(t, false).iter().all(|s| s.valid), "{t}");
            assert!(pattern::parse(t).is_ok(), "{t}");
        }
        for t in ["{TAB 101}", "{DELAY}", "{NOPE}", "{}"] {
            assert!(pattern_spans(t, false).iter().any(|s| !s.valid), "{t}");
        }
        // templates are stricter than patterns in general
        for t in ["{CTRL+A}", "{F5}", "{TAB 9}", "{DELAY 5000}", "{ESC}"] {
            assert!(pattern_spans(t, false).iter().all(|s| s.valid), "{t}");
            assert!(pattern_spans(t, true).iter().any(|s| !s.valid), "{t} in a template");
        }
        for t in ["{TAB}", "{SHIFT+TAB}", "{ENTER}", "{DEL 5}", "{DELAY 1000}"] {
            assert!(pattern_spans(t, true).iter().all(|s| s.valid), "{t} in a template");
        }
    }

    #[test]
    fn local_key_names_are_accepted_and_shown_in_capitals() {
        let _g = crate::i18n::english_guard();
        let dir = std::env::temp_dir();
        crate::i18n::init("it", "en", &dir);
        let s = shortcut_spans("maiusc + Invio");
        assert_eq!(s.iter().map(|x| x.valid).collect::<Vec<_>>(), [true, true]);
        assert_eq!(s[0].canonical, "MAIUSC");
        assert_eq!(s[1].canonical, "INVIO");
        // English names still work, and are shown in the local language
        assert_eq!(ordered_shortcut("v+shift+ctrl").as_deref(), Some("CTRL+MAIUSC+V"));
        assert_eq!(ordered_shortcut("canc+alt").as_deref(), Some("ALT+CANC"));
        assert_eq!(hotkey::to_english("CTRL+MAIUSC+Invio"), "ctrl+shift+enter");
        assert_eq!(hotkey::to_local("ctrl+alt+shift+v"), "CTRL+ALT+MAIUSC+V");
        assert!(!shortcut_spans("strg+v")[0].valid); // German is not the language in use
        crate::i18n::init("en", "en", &dir);
    }
}
