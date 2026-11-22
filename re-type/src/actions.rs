// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Turns a pattern and the clipboard text into a stream of keystroke actions
//! (REQ-FUNC-001, REQ-FUNC-004, REQ-FUNC-005, REQ-UX-004).
//!
//! The stream is produced lazily and reads the clipboard text in place, so no second copy of
//! it is made (REQ-SEC-003).

use crate::hotkey::{MOD_SHIFT, VK_RETURN, VK_TAB};
use crate::pattern::Segment;
use crate::safechars::{self, Mapped, SafeMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// One UTF-16 code unit, sent as a Unicode key event (a surrogate pair is two units).
    Unit(u16),
    Key { mods: u32, vk: u32 },
    DelayMs(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewlineMode {
    Enter,
    ShiftEnter,
    Space,
    Skip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabMode {
    Key,
    Spaces,
    Skip,
}

impl NewlineMode {
    pub const ALL: [NewlineMode; 4] = [Self::Enter, Self::ShiftEnter, Self::Space, Self::Skip];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::ShiftEnter => "shift_enter",
            Self::Space => "space",
            Self::Skip => "skip",
        }
    }
    pub fn parse_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == s)
    }
}

impl TabMode {
    pub const ALL: [TabMode; 3] = [Self::Key, Self::Spaces, Self::Skip];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Key => "key",
            Self::Spaces => "spaces",
            Self::Skip => "skip",
        }
    }
    pub fn parse_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == s)
    }
}

/// The options that change what is typed (REQ-FUNC-004). Timing options live in the config.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextOptions {
    pub newline: NewlineMode,
    pub tab: TabMode,
    pub tab_spaces: u32,
    pub strip_trailing_newline: bool,
    /// Conversion of special characters of the clipboard text (REQ-FUNC-007).
    pub safe: SafeMode,
}

impl Default for TextOptions {
    fn default() -> Self {
        TextOptions {
            // typing must not press Enter or Tab on its own: both can submit a form or move the
            // focus, so they are skipped unless the user switches them on
            newline: NewlineMode::Skip,
            tab: TabMode::Skip,
            tab_spaces: 4,
            strip_trailing_newline: true,
            safe: SafeMode::Typographic,
        }
    }
}

enum Step {
    One(Action),
    Repeat(Action, u32),
    /// ASCII text to type in place of one character.
    Str(&'static str),
    Skip,
}

const LF: u16 = 0x0A;
const CR: u16 = 0x0D;
const HT: u16 = 0x09;

/// Reads one logical character from `units[*pos..end]` and translates it. `safe` is the
/// special-character conversion to apply (off for template text, which the user wrote).
fn step(units: &[u16], pos: &mut usize, end: usize, o: &TextOptions, safe: SafeMode) -> Step {
    let u = match safechars::map(safe, units, pos, end) {
        Mapped::Unit(u) => u,
        Mapped::Str(t) => return Step::Str(t),
        Mapped::Drop => return Step::Skip,
    };
    match u {
        CR | LF => {
            if u == CR && *pos < end && units[*pos] == LF {
                *pos += 1; // CRLF is one newline
            }
            match o.newline {
                NewlineMode::Enter => Step::One(Action::Key { mods: 0, vk: VK_RETURN }),
                NewlineMode::ShiftEnter => Step::One(Action::Key { mods: MOD_SHIFT, vk: VK_RETURN }),
                NewlineMode::Space => Step::One(Action::Unit(0x20)),
                NewlineMode::Skip => Step::Skip,
            }
        }
        HT => match o.tab {
            TabMode::Key => Step::One(Action::Key { mods: 0, vk: VK_TAB }),
            TabMode::Spaces => Step::Repeat(Action::Unit(0x20), o.tab_spaces),
            TabMode::Skip => Step::Skip,
        },
        // Other control characters cannot be typed.
        0..=0x1F | 0x7F => Step::Skip,
        _ => Step::One(Action::Unit(u)),
    }
}

/// Length of `clip` without trailing newlines, if the option asks for it.
fn effective_end(clip: &[u16], o: &TextOptions) -> usize {
    let mut end = clip.len();
    if o.strip_trailing_newline {
        while end > 0 && (clip[end - 1] == LF || clip[end - 1] == CR) {
            end -= 1;
        }
    }
    end
}

pub struct Actions<'a> {
    segments: &'a [Segment],
    clip: &'a [u16],
    clip_end: usize,
    opts: TextOptions,
    seg: usize,
    pos: usize,
    repeat: Option<(Action, u32)>,
    /// Remaining ASCII text of a replacement, typed one unit at a time.
    text: &'static [u8],
}

pub fn build<'a>(segments: &'a [Segment], clipboard: &'a [u16], opts: TextOptions) -> Actions<'a> {
    Actions {
        segments,
        clip: clipboard,
        clip_end: effective_end(clipboard, &opts),
        opts,
        seg: 0,
        pos: 0,
        repeat: None,
        text: &[],
    }
}

impl Iterator for Actions<'_> {
    type Item = Action;

    fn next(&mut self) -> Option<Action> {
        loop {
            if let Some((&b, rest)) = self.text.split_first() {
                self.text = rest;
                return Some(Action::Unit(u16::from(b)));
            }
            if let Some((a, n)) = self.repeat {
                self.repeat = if n > 1 { Some((a, n - 1)) } else { None };
                return Some(a);
            }
            let segment = self.segments.get(self.seg)?;
            let (units, end): (&[u16], usize) = match segment {
                Segment::Text(t) => (t, t.len()),
                Segment::Clipboard => (self.clip, self.clip_end),
                Segment::Keys { mods, vk, count } => {
                    self.seg += 1;
                    self.pos = 0;
                    self.repeat = Some((Action::Key { mods: *mods, vk: *vk }, *count));
                    continue;
                }
                Segment::Delay(ms) => {
                    self.seg += 1;
                    self.pos = 0;
                    return Some(Action::DelayMs(*ms));
                }
            };
            if self.pos >= end {
                self.seg += 1;
                self.pos = 0;
                continue;
            }
            // The newline, tab and special-character options are for the clipboard text only.
            // Template text is what the user wrote: its line breaks and tabs are typed as Enter
            // and Tab.
            let from_clipboard = matches!(segment, Segment::Clipboard);
            let (opts, safe) = if from_clipboard {
                (self.opts, self.opts.safe)
            } else {
                (TextOptions { newline: NewlineMode::Enter, tab: TabMode::Key, ..self.opts }, SafeMode::Off)
            };
            match step(units, &mut self.pos, end, &opts, safe) {
                Step::One(a) => return Some(a),
                Step::Str(t) => self.text = t.as_bytes(),
                Step::Repeat(a, n) if n > 0 => {
                    self.repeat = Some((a, n));
                }
                Step::Repeat(..) | Step::Skip => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern;

    fn u(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }
    fn run(pat: &str, clip: &str, o: TextOptions) -> Vec<Action> {
        let segs = pattern::parse(pat).unwrap();
        let c = u(clip);
        build(&segs, &c, o).collect()
    }
    fn chars(s: &str) -> Vec<Action> {
        u(s).into_iter().map(Action::Unit).collect()
    }
    /// Enter and Tab switched on (they are skipped by default).
    fn keys_on() -> TextOptions {
        TextOptions { newline: NewlineMode::Enter, tab: TabMode::Key, ..TextOptions::default() }
    }
    const ENTER: Action = Action::Key { mods: 0, vk: VK_RETURN };
    const TAB: Action = Action::Key { mods: 0, vk: VK_TAB };

    #[test]
    fn plain_clipboard() {
        assert_eq!(run("{CLIPBOARD}", "abc", TextOptions::default()), chars("abc"));
    }

    #[test]
    fn prepend_and_append() {
        let mut want = vec![TAB];
        want.extend(chars("x"));
        want.push(ENTER);
        assert_eq!(run("{TAB}{CLIPBOARD}{ENTER}", "x", TextOptions::default()), want);
    }

    #[test]
    fn newline_modes_and_crlf() {
        let o = keys_on();
        assert_eq!(run("{CLIP}", "a\r\nb\rc\nd", o).iter().filter(|a| **a == ENTER).count(), 3);
        let shift = Action::Key { mods: MOD_SHIFT, vk: VK_RETURN };
        let o2 = TextOptions { newline: NewlineMode::ShiftEnter, ..o };
        assert_eq!(run("{CLIP}", "a\nb", o2), vec![Action::Unit(97), shift, Action::Unit(98)]);
        let o3 = TextOptions { newline: NewlineMode::Space, ..o };
        assert_eq!(run("{CLIP}", "a\r\nb", o3), chars("a b"));
        let o4 = TextOptions { newline: NewlineMode::Skip, ..o };
        assert_eq!(run("{CLIP}", "a\nb", o4), chars("ab"));
    }

    #[test]
    fn tab_modes() {
        let o = keys_on();
        assert_eq!(run("{CLIP}", "\t", o), vec![TAB]);
        let sp = TextOptions { tab: TabMode::Spaces, tab_spaces: 3, ..o };
        assert_eq!(run("{CLIP}", "\t", sp), chars("   "));
        let skip = TextOptions { tab: TabMode::Skip, ..o };
        assert!(run("{CLIP}", "\t", skip).is_empty());
    }

    #[test]
    fn trailing_newlines_stripped_only_when_asked() {
        let o = keys_on();
        assert_eq!(run("{CLIP}", "a\r\n\n", o), chars("a"));
        let keep = TextOptions { strip_trailing_newline: false, ..o };
        assert_eq!(run("{CLIP}", "a\n", keep), vec![Action::Unit(97), ENTER]);
        assert!(run("{CLIP}", "\n\n", o).is_empty());
    }

    #[test]
    fn control_characters_are_dropped_and_emoji_survive() {
        assert_eq!(run("{CLIP}", "a\u{7}b\u{7f}", TextOptions::default()), chars("ab"));
        let emoji = run("{CLIP}", "😀", TextOptions::default());
        assert_eq!(emoji, vec![Action::Unit(0xD83D), Action::Unit(0xDE00)]);
    }

    #[test]
    fn keys_repeat_delay_and_empty_clipboard() {
        assert_eq!(run("{TAB 2}{DELAY 10}", "", TextOptions::default()), vec![TAB, TAB, Action::DelayMs(10)]);
        assert_eq!(run("a{CLIP}b", "", TextOptions::default()), chars("ab"));
    }

    #[test]
    fn template_text_is_typed_as_written_whatever_the_clipboard_options() {
        let d = TextOptions::default(); // Enter and Tab skipped for the clipboard text
        assert_eq!(run("a\nb", "", d), vec![Action::Unit(97), ENTER, Action::Unit(98)]);
        assert_eq!(run("a\tb", "", d), vec![Action::Unit(97), TAB, Action::Unit(98)]);
        let space = TextOptions { newline: NewlineMode::Space, tab: TabMode::Spaces, ..d };
        assert_eq!(run("a\nb\tc", "", space), vec![Action::Unit(97), ENTER, Action::Unit(98), TAB, Action::Unit(99)]);
        // the same options still apply to the clipboard part of the same template
        assert_eq!(run("a\n{CLIP}", "x\ny", d), vec![Action::Unit(97), ENTER, Action::Unit(120), Action::Unit(121)]);
    }

    #[test]
    fn enter_and_tab_are_skipped_by_default() {
        let d = TextOptions::default();
        assert_eq!((d.newline, d.tab, d.safe), (NewlineMode::Skip, TabMode::Skip, SafeMode::Typographic));
        assert_eq!(run("{CLIP}", "a\r\nb\tc", d), chars("abc"));
        assert_eq!(run("{CLIP}", "\u{201C}q\u{201D}\u{2026}", d), chars("\"q\"..."));
        // explicit key tokens in a template are the user's choice and still work
        assert_eq!(run("{ENTER}{TAB}", "", d), vec![ENTER, TAB]);
    }

    #[test]
    fn safe_modes_apply_to_clipboard_text_only() {
        let o = TextOptions { safe: SafeMode::Ascii, ..TextOptions::default() };
        assert_eq!(run("\u{E9}{CLIP}", "\u{E9}\u{20AC}", o), {
            let mut v = chars("\u{E9}"); // template text is the user's own and stays as written
            v.extend(chars("eEUR"));
            v
        });
        let t = TextOptions { safe: SafeMode::Typographic, ..TextOptions::default() };
        assert_eq!(run("{CLIP}", "\u{201C}a\u{201D}\u{2026}", t), chars("\"a\"..."));
    }

    #[test]
    fn safe_mode_replacement_can_end_a_run_and_mix_with_newlines() {
        let o = TextOptions { safe: SafeMode::Ascii, ..keys_on() };
        let got = run("{CLIP}", "\u{A9}\r\n\u{2028}x\u{1F600}", o);
        let mut want = chars("(c)");
        want.push(ENTER);
        want.push(ENTER);
        want.extend(chars("x?"));
        assert_eq!(got, want);
    }
}
