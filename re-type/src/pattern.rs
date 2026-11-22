// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Typing-template patterns (REQ-FUNC-005).
//!
//! A pattern is plain text with `{...}` tokens:
//!
//! | token | meaning |
//! |---|---|
//! | `{CLIPBOARD}` or `{CLIP}` | the clipboard text |
//! | `{TAB}`, `{ENTER}`, `{F5}`, `{CTRL+A}`, `{SHIFT+TAB}` | a key or key combination |
//! | `{TAB 3}` | the key repeated 3 times (1..=100) |
//! | `{DELAY 200}` or `{WAIT 200}` | pause 200 ms (at most 60000) |
//! | `{{` and `}}` | literal braces |

use crate::hotkey::{self, ParseError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Literal text, as UTF-16.
    Text(Vec<u16>),
    Clipboard,
    Keys { mods: u32, vk: u32, count: u32 },
    Delay(u32),
}

pub const MAX_REPEAT: u32 = 100;
/// Limits for templates : they add text and field navigation around the clipboard text,
/// they are not macros (REQ-FUNC-005).
pub const TEMPLATE_MAX_REPEAT: u32 = 5;
pub const TEMPLATE_MAX_DELAY_MS: u32 = 1_000;
const TEMPLATE_KEYS: [(u32, &str); 10] = [
    (0x09, "TAB"),
    (0x0D, "ENTER"),
    (0x26, "UP"),
    (0x28, "DOWN"),
    (0x25, "LEFT"),
    (0x27, "RIGHT"),
    (0x24, "HOME"),
    (0x23, "END"),
    (0x08, "BACKSPACE"),
    (0x2E, "DEL"),
];
pub const MAX_DELAY_MS: u32 = 60_000;

fn bad<T>(s: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError(s.into()))
}

fn parse_token(body: &str) -> Result<Segment, ParseError> {
    let t = body.trim();
    if t.is_empty() {
        return bad("empty token '{}'");
    }
    let upper = t.to_ascii_uppercase();
    if upper == "CLIPBOARD" || upper == "CLIP" {
        return Ok(Segment::Clipboard);
    }
    // A trailing number after whitespace is a count (keys) or a duration (DELAY/WAIT).
    let (head, number) = match t.rsplit_once(char::is_whitespace) {
        Some((h, n)) if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => {
            let value: u32 = n.parse().map_err(|_| ParseError(format!("number too large in '{{{t}}}'")))?;
            (h.trim(), Some(value))
        }
        _ => (t, None),
    };
    let head_upper = head.to_ascii_uppercase();
    if head_upper == "DELAY" || head_upper == "WAIT" {
        return match number {
            Some(ms) if ms <= MAX_DELAY_MS => Ok(Segment::Delay(ms)),
            Some(_) => bad(format!("delay over {MAX_DELAY_MS} ms in '{{{t}}}'")),
            None => bad(format!("'{{{t}}}' needs a duration in ms, e.g. {{DELAY 200}}")),
        };
    }
    let (mods, vk) = hotkey::parse_combo(head)?;
    let count = number.unwrap_or(1);
    if !(1..=MAX_REPEAT).contains(&count) {
        return bad(format!("repeat count must be 1..{MAX_REPEAT} in '{{{t}}}'"));
    }
    Ok(Segment::Keys { mods, vk, count })
}

/// Whether the text between braces is a token the parser understands.
/// With `template` set, the template limits apply as well (DC-025).
pub fn token_is_valid(body: &str, template: bool) -> bool {
    match parse_token(body) {
        Ok(seg) => !template || check_template(&[Segment::Clipboard, seg]).is_ok(),
        Err(_) => false,
    }
}

pub fn parse(pattern: &str) -> Result<Vec<Segment>, ParseError> {
    let mut out: Vec<Segment> = Vec::new();
    let mut text = String::new();
    let flush = |text: &mut String, out: &mut Vec<Segment>| {
        if !text.is_empty() {
            out.push(Segment::Text(text.encode_utf16().collect()));
            text.clear();
        }
    };
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                text.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                text.push('}');
            }
            '{' => {
                let mut body = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some('{') | None => return bad("unclosed '{' (write '{{' for a literal brace)"),
                        Some(ch) => body.push(ch),
                    }
                }
                flush(&mut text, &mut out);
                out.push(parse_token(&body)?);
            }
            '}' => return bad("single '}' (write '}}' for a literal brace)"),
            _ => text.push(c),
        }
    }
    flush(&mut text, &mut out);
    Ok(out)
}

/// Whether a template pattern stays within what templates are for: the clipboard text plus
/// plain text, a few navigation and editing keys (Tab, Shift+Tab, Enter, arrows, Home, End,
/// Backspace, Delete) repeated at most 5 times, and pauses of at most 1 s. No shortcuts (Ctrl, Alt
/// or Windows combinations), function keys or Esc. Returns the reason in plain words.
pub fn check_template(segments: &[Segment]) -> Result<(), String> {
    if !needs_clipboard(segments) {
        return Err("a template must contain {CLIPBOARD}; it adds text and field navigation around the clipboard text".into());
    }
    for seg in segments {
        match seg {
            Segment::Keys { mods, vk, count } => {
                let plain = *mods == 0 && TEMPLATE_KEYS.iter().any(|(k, _)| k == vk);
                let shift_tab = *mods == crate::hotkey::MOD_SHIFT && *vk == 0x09;
                if !(plain || shift_tab) {
                    let key = hotkey::Hotkey { mods: *mods, vk: *vk };
                    return Err(format!(
                        "the key {{{}}} is not allowed in a template; allowed keys: TAB, SHIFT+TAB, ENTER, UP, DOWN, LEFT, RIGHT, HOME, END, BACKSPACE, DEL",
                        key.to_string().to_ascii_uppercase()
                    ));
                }
                if *count > TEMPLATE_MAX_REPEAT {
                    return Err(format!("a key can be repeated at most {TEMPLATE_MAX_REPEAT} times in a template"));
                }
            }
            Segment::Delay(ms) if *ms > TEMPLATE_MAX_DELAY_MS => {
                return Err(format!("a pause can last at most {TEMPLATE_MAX_DELAY_MS} ms in a template"));
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn needs_clipboard(segments: &[Segment]) -> bool {
    segments.iter().any(|s| matches!(s, Segment::Clipboard))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::{MOD_CONTROL, MOD_SHIFT, VK_RETURN, VK_TAB};

    fn t(s: &str) -> Segment {
        Segment::Text(s.encode_utf16().collect())
    }

    #[test]
    fn text_and_tokens() {
        assert_eq!(
            parse("user:{CLIPBOARD}{TAB}{ENTER}").unwrap(),
            vec![
                t("user:"),
                Segment::Clipboard,
                Segment::Keys { mods: 0, vk: VK_TAB, count: 1 },
                Segment::Keys { mods: 0, vk: VK_RETURN, count: 1 },
            ]
        );
    }

    #[test]
    fn combos_counts_and_delays() {
        assert_eq!(
            parse("{ctrl+a}{Shift+Tab 3}{delay 250}").unwrap(),
            vec![
                Segment::Keys { mods: MOD_CONTROL, vk: b'A' as u32, count: 1 },
                Segment::Keys { mods: MOD_SHIFT, vk: VK_TAB, count: 3 },
                Segment::Delay(250),
            ]
        );
    }

    #[test]
    fn literal_braces_and_unicode() {
        assert_eq!(parse("{{x}} à😀").unwrap(), vec![t("{x} à😀")]);
    }

    #[test]
    fn errors() {
        for bad in ["{", "}", "{}", "{nokey}", "{TAB 0}", "{TAB 101}", "{DELAY}", "{DELAY 60001}", "a{b{c}", "{ctrl+}"] {
            assert!(parse(bad).is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn clipboard_detection() {
        assert!(needs_clipboard(&parse("a{CLIP}").unwrap()));
        assert!(!needs_clipboard(&parse("a{TAB}").unwrap()));
    }

    #[test]
    fn templates_are_limited_to_text_entry() {
        let ok = |p: &str| check_template(&parse(p).unwrap());
        assert!(ok("{CLIPBOARD}").is_ok());
        assert!(ok("ID-{CLIPBOARD}{TAB}{SHIFT+TAB}{ENTER}").is_ok());
        assert!(ok("{HOME}{CLIPBOARD}{END}{BACKSPACE}{DEL}{UP}{DOWN}{LEFT}{RIGHT}").is_ok());
        assert!(ok("{CLIPBOARD}{TAB 5}{DELAY 1000}").is_ok());
        // needs the clipboard
        assert!(ok("hello{TAB}").is_err());
        // no shortcuts, function keys, Esc, paging keys
        for bad in ["{CTRL+A}{CLIPBOARD}", "{CLIPBOARD}{ALT+F4}", "{WIN+R}{CLIPBOARD}", "{F5}{CLIPBOARD}", "{ESC}{CLIPBOARD}", "{CLIPBOARD}{PGDN}", "{SHIFT+HOME}{CLIPBOARD}"] {
            assert!(ok(bad).is_err(), "{bad}");
        }
        // limits
        assert!(ok("{CLIPBOARD}{TAB 6}").is_err());
        assert!(ok("{CLIPBOARD}{DELAY 1001}").is_err());
        let msg = ok("{CTRL+A}{CLIPBOARD}").unwrap_err();
        assert!(msg.contains("{CTRL+A}"), "{msg}");
    }
}
