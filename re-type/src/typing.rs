// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! One typing run, independent of the operating system (REQ-FUNC-001, REQ-SAFE-001,
//! REQ-SEC-003). The OS-specific parts are supplied through `Backend`.

use crate::actions::{self, Action, TextOptions};
use crate::pattern::{self, Segment};
use crate::secure::SecureBuf;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// What to do when the clipboard holds copied files instead of text (REQ-FUNC-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesMode {
    /// Type the full paths, one per line.
    Paths,
    /// Type only the file names, one per line.
    Names,
    /// Type nothing.
    Ignore,
}

impl FilesMode {
    pub const ALL: [FilesMode; 3] = [Self::Paths, Self::Names, Self::Ignore];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paths => "paths",
            Self::Names => "names",
            Self::Ignore => "ignore",
        }
    }
    pub fn parse_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardRead {
    /// Text was copied into the buffer.
    Text,
    /// Copied files: the buffer holds their full paths separated by line feeds.
    Files,
    /// The clipboard holds an image and no text.
    Image,
    /// The clipboard holds something else that is not text.
    Other,
    /// The clipboard is empty or its text is empty.
    NoText,
    /// The text is longer than the buffer; nothing was copied.
    TooLong,
    /// Another program holds the clipboard.
    Busy,
}

pub trait Backend {
    /// Copies the clipboard text into `buf`, up to its capacity.
    fn read_clipboard(&self, buf: &mut SecureBuf) -> ClipboardRead;
    fn send(&self, action: &Action) -> Result<(), String>;
    /// Empties the clipboard, but only if it still holds what `read_clipboard` read (nothing
    /// newer is destroyed). Returns whether it was emptied.
    fn clear_clipboard(&self) -> bool;
    fn escape_pressed(&self) -> bool;
    fn sleep_ms(&self, ms: u32);
    /// Waits until the keys of the hotkey are released, then makes sure they are not "stuck".
    fn release_modifiers(&self, hotkey_mods: u32, timeout_ms: u32);
}

#[derive(Debug, Clone, Copy)]
pub struct RunOptions {
    pub text: TextOptions,
    pub initial_delay_ms: u32,
    pub char_delay_ms: u32,
    pub release_timeout_ms: u32,
    /// Longest accepted clipboard text, in UTF-16 units.
    pub max_chars: usize,
    pub files: FilesMode,
    /// Empty the clipboard after a successful run that typed it.
    pub clear_clipboard: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Completed { actions: usize },
    Aborted { after: usize },
    NoText,
    /// The clipboard holds an image, nothing was typed.
    Image,
    /// The clipboard holds non-text content, nothing was typed.
    NotText,
    /// The clipboard holds files and typing them is switched off.
    FilesIgnored,
    TooLong { limit: usize },
    ClipboardBusy,
    Failed(String),
}

/// Runs `segments`. The typing buffer lives only inside this call and is wiped before it returns.
pub fn run<B: Backend>(b: &B, hotkey_mods: u32, segments: &[Segment], o: &RunOptions) -> Outcome {
    let capacity = if pattern::needs_clipboard(segments) { o.max_chars } else { 0 };
    let mut buf = SecureBuf::with_capacity(capacity);
    run_with_buf(b, hotkey_mods, segments, o, &mut buf)
}

pub(crate) fn run_with_buf<B: Backend>(
    b: &B,
    hotkey_mods: u32,
    segments: &[Segment],
    o: &RunOptions,
    buf: &mut SecureBuf,
) -> Outcome {
    let result = catch_unwind(AssertUnwindSafe(|| inner(b, hotkey_mods, segments, o, buf)));
    buf.clear(); // every exit path, including a panic caught above
    result.unwrap_or_else(|_| Outcome::Failed("internal error".into()))
}

/// Reduces `units[..len]`, lines of full paths, to lines of file names, in place. Empty names
/// (a path ending in a separator, such as a drive root) are dropped. Returns the new length.
pub fn names_only(units: &mut [u16], len: usize) -> usize {
    let (mut write, mut line_start) = (0usize, 0usize);
    while line_start <= len {
        let line_end = units[line_start..len].iter().position(|&u| u == 0x0A).map_or(len, |i| line_start + i);
        let name_start = units[line_start..line_end]
            .iter()
            .rposition(|&u| u == 0x5C || u == 0x2F)
            .map_or(line_start, |i| line_start + i + 1);
        if name_start < line_end {
            if write > 0 {
                units[write] = 0x0A;
                write += 1;
            }
            // regions never overlap in the wrong direction: write <= name_start always
            units.copy_within(name_start..line_end, write);
            write += line_end - name_start;
        }
        line_start = line_end + 1;
    }
    for u in &mut units[write..len] {
        *u = 0; // do not leave the longer original text behind
    }
    write
}

fn inner<B: Backend>(b: &B, mods: u32, segments: &[Segment], o: &RunOptions, buf: &mut SecureBuf) -> Outcome {
    b.release_modifiers(mods, o.release_timeout_ms);
    b.sleep_ms(o.initial_delay_ms);
    if pattern::needs_clipboard(segments) {
        match b.read_clipboard(buf) {
            ClipboardRead::Text => {}
            ClipboardRead::Files => match o.files {
                FilesMode::Paths => {}
                FilesMode::Names => {
                    let len = buf.len();
                    let kept = names_only(buf.storage_mut(), len);
                    buf.set_len(kept);
                    if kept == 0 {
                        return Outcome::NoText;
                    }
                }
                FilesMode::Ignore => return Outcome::FilesIgnored,
            },
            ClipboardRead::Image => return Outcome::Image,
            ClipboardRead::Other => return Outcome::NotText,
            ClipboardRead::NoText => return Outcome::NoText,
            ClipboardRead::TooLong => return Outcome::TooLong { limit: o.max_chars },
            ClipboardRead::Busy => return Outcome::ClipboardBusy,
        }
    }
    let mut done = 0usize;
    for action in actions::build(segments, buf.as_units(), o.text) {
        if b.escape_pressed() {
            return Outcome::Aborted { after: done };
        }
        match action {
            Action::DelayMs(ms) => {
                // sleep in slices so Esc stays responsive
                let mut left = ms;
                while left > 0 {
                    let slice = left.min(20);
                    b.sleep_ms(slice);
                    left -= slice;
                    if b.escape_pressed() {
                        return Outcome::Aborted { after: done };
                    }
                }
            }
            a => {
                if let Err(e) = b.send(&a) {
                    return Outcome::Failed(e);
                }
                if o.char_delay_ms > 0 {
                    b.sleep_ms(o.char_delay_ms);
                }
            }
        }
        done += 1;
    }
    if o.clear_clipboard && pattern::needs_clipboard(segments) {
        b.clear_clipboard();
    }
    Outcome::Completed { actions: done }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct Mock {
        clip: Option<Vec<u16>>,
        sent: RefCell<Vec<Action>>,
        esc_after: Cell<Option<usize>>,
        fail_at: Option<usize>,
        panic_at: Option<usize>,
        slept: Cell<u32>,
        forced: Option<ClipboardRead>,
        cleared: Cell<u32>,
    }
    impl Mock {
        fn new(clip: Option<&str>) -> Self {
            Mock {
                clip: clip.map(|s| s.encode_utf16().collect()),
                sent: RefCell::new(vec![]),
                esc_after: Cell::new(None),
                fail_at: None,
                panic_at: None,
                slept: Cell::new(0),
                forced: None,
                cleared: Cell::new(0),
            }
        }
    }
    impl Backend for Mock {
        fn read_clipboard(&self, buf: &mut SecureBuf) -> ClipboardRead {
            match &self.clip {
                None => ClipboardRead::NoText,
                Some(c) => match buf.fill_from(c) {
                    Ok(()) => self.forced.unwrap_or(ClipboardRead::Text),
                    Err(_) => ClipboardRead::TooLong,
                },
            }
        }
        fn send(&self, a: &Action) -> Result<(), String> {
            let n = self.sent.borrow().len();
            if self.panic_at == Some(n) {
                panic!("boom");
            }
            if self.fail_at == Some(n) {
                return Err("SendInput blocked".into());
            }
            self.sent.borrow_mut().push(*a);
            Ok(())
        }
        fn clear_clipboard(&self) -> bool {
            self.cleared.set(self.cleared.get() + 1);
            true
        }
        fn escape_pressed(&self) -> bool {
            self.esc_after.get().is_some_and(|n| self.sent.borrow().len() >= n)
        }
        fn sleep_ms(&self, ms: u32) {
            self.slept.set(self.slept.get() + ms);
        }
        fn release_modifiers(&self, _: u32, _: u32) {}
    }

    fn opts() -> RunOptions {
        RunOptions {
            text: TextOptions::default(),
            initial_delay_ms: 10,
            char_delay_ms: 2,
            release_timeout_ms: 100,
            max_chars: 16,
            files: FilesMode::Paths,
            clear_clipboard: true,
        }
    }
    fn segs(p: &str) -> Vec<Segment> {
        pattern::parse(p).unwrap()
    }
    fn run_buf(m: &Mock, p: &str, o: &RunOptions) -> (Outcome, SecureBuf) {
        let mut buf = SecureBuf::with_capacity(o.max_chars);
        let out = run_with_buf(m, 0, &segs(p), o, &mut buf);
        (out, buf)
    }

    #[test]
    fn completes_and_wipes() {
        let m = Mock::new(Some("hello"));
        let (out, buf) = run_buf(&m, "{CLIP}{TAB}", &opts());
        assert_eq!(out, Outcome::Completed { actions: 6 });
        assert_eq!(m.sent.borrow().len(), 6);
        assert!(buf.is_wiped() && buf.is_empty());
        assert_eq!(m.slept.get(), 10 + 6 * 2);
    }

    #[test]
    fn esc_aborts_and_wipes() {
        let m = Mock::new(Some("hello"));
        m.esc_after.set(Some(2));
        let (out, buf) = run_buf(&m, "{CLIP}", &opts());
        assert_eq!(out, Outcome::Aborted { after: 2 });
        assert_eq!(m.sent.borrow().len(), 2);
        assert!(buf.is_wiped());
    }

    #[test]
    fn send_failure_wipes() {
        let mut m = Mock::new(Some("hello"));
        m.fail_at = Some(1);
        let (out, buf) = run_buf(&m, "{CLIP}", &opts());
        assert_eq!(out, Outcome::Failed("SendInput blocked".into()));
        assert!(buf.is_wiped());
    }

    #[test]
    fn panic_wipes_and_is_reported_without_content() {
        let mut m = Mock::new(Some("secret"));
        m.panic_at = Some(3);
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let (out, buf) = run_buf(&m, "{CLIP}", &opts());
        std::panic::set_hook(prev);
        assert_eq!(out, Outcome::Failed("internal error".into()));
        assert!(buf.is_wiped());
    }

    #[test]
    fn too_long_is_refused_before_any_key() {
        let m = Mock::new(Some("this text is longer than sixteen units"));
        let (out, buf) = run_buf(&m, "{TAB}{CLIP}", &opts());
        assert_eq!(out, Outcome::TooLong { limit: 16 });
        assert!(m.sent.borrow().is_empty() && buf.is_wiped());
    }

    #[test]
    fn clipboard_is_cleared_only_after_a_successful_run_that_typed_it() {
        let m = Mock::new(Some("abc"));
        assert!(matches!(run_buf(&m, "{CLIP}", &opts()).0, Outcome::Completed { .. }));
        assert_eq!(m.cleared.get(), 1);
        // the option off: not cleared
        let m = Mock::new(Some("abc"));
        let off = RunOptions { clear_clipboard: false, ..opts() };
        run_buf(&m, "{CLIP}", &off);
        assert_eq!(m.cleared.get(), 0);
        // aborted, failed, refused, not text, ignored files and a pattern without the clipboard: not cleared
        let m = Mock::new(Some("abcdef"));
        m.esc_after.set(Some(2));
        assert!(matches!(run_buf(&m, "{CLIP}", &opts()).0, Outcome::Aborted { .. }));
        let mut m2 = Mock::new(Some("abc"));
        m2.fail_at = Some(1);
        assert!(matches!(run_buf(&m2, "{CLIP}", &opts()).0, Outcome::Failed(_)));
        let m3 = Mock::new(Some("this text is longer than sixteen units"));
        run_buf(&m3, "{CLIP}", &opts());
        let mut m4 = Mock::new(Some("abc"));
        m4.forced = Some(ClipboardRead::Image);
        run_buf(&m4, "{CLIP}", &opts());
        let mut m5 = Mock::new(Some("abc"));
        m5.forced = Some(ClipboardRead::Files);
        run_buf(&m5, "{CLIP}", &RunOptions { files: FilesMode::Ignore, ..opts() });
        let m6 = Mock::new(Some("abc"));
        run_buf(&m6, "ab{ENTER}", &opts());
        for m in [&m, &m2, &m3, &m4, &m5, &m6] {
            assert_eq!(m.cleared.get(), 0);
        }
    }

    #[test]
    fn no_text_and_pattern_without_clipboard() {
        let m = Mock::new(None);
        assert_eq!(run_buf(&m, "{CLIP}", &opts()).0, Outcome::NoText);
        let out = run(&m, 0, &segs("ab{ENTER}"), &opts());
        assert_eq!(out, Outcome::Completed { actions: 3 });
    }

    #[test]
    fn delay_is_sliced_and_interruptible() {
        let m = Mock::new(None);
        let out = run(&m, 0, &segs("{DELAY 100}a"), &opts());
        assert_eq!(out, Outcome::Completed { actions: 2 });
        let m2 = Mock::new(None);
        m2.esc_after.set(Some(0));
        assert_eq!(run(&m2, 0, &segs("{DELAY 100}a"), &opts()), Outcome::Aborted { after: 0 });
    }

    #[test]
    fn names_only_keeps_last_path_component() {
        let mut v: Vec<u16> = "C:\\a\\b.txt\nD:\\c\\d e.pdf\nC:\\\n/x/y.z".encode_utf16().collect();
        let len = v.len();
        let n = names_only(&mut v, len);
        assert_eq!(String::from_utf16(&v[..n]).unwrap(), "b.txt\nd e.pdf\ny.z");
        assert!(v[n..].iter().all(|&u| u == 0));
        let mut empty: Vec<u16> = "C:\\".encode_utf16().collect();
        assert_eq!(names_only(&mut empty, 3), 0);
        assert_eq!(names_only(&mut [], 0), 0);
    }

    fn files_run(mode: FilesMode, o: &mut RunOptions) -> (Outcome, Mock) {
        let mut m = Mock::new(Some("C:\\dir\\one.txt\nC:\\dir\\two.txt"));
        m.forced = Some(ClipboardRead::Files);
        o.files = mode;
        o.max_chars = 64;
        let out = run(&m, 0, &segs("{CLIP}"), o);
        (out, m)
    }

    #[test]
    fn copied_files_typed_as_paths_names_or_not_at_all() {
        let mut o = opts();
        o.text.newline = crate::actions::NewlineMode::Enter; // one Enter between the files
        let (out, m) = files_run(FilesMode::Paths, &mut o);
        assert!(matches!(out, Outcome::Completed { .. }));
        let typed = m.sent.borrow().len();
        assert_eq!(typed, "C:\\dir\\one.txt".len() + 1 + "C:\\dir\\two.txt".len());
        let (out, m) = files_run(FilesMode::Names, &mut o);
        assert!(matches!(out, Outcome::Completed { .. }));
        assert_eq!(m.sent.borrow().len(), "one.txt".len() + 1 + "two.txt".len());
        let (out, m) = files_run(FilesMode::Ignore, &mut o);
        assert_eq!(out, Outcome::FilesIgnored);
        assert!(m.sent.borrow().is_empty());
    }

    #[test]
    fn images_and_other_content_type_nothing_and_leave_no_trace() {
        for (kind, want) in [(ClipboardRead::Image, Outcome::Image), (ClipboardRead::Other, Outcome::NotText)] {
            let mut m = Mock::new(Some("x"));
            m.forced = Some(kind);
            let (out, buf) = run_buf(&m, "a{CLIP}b", &opts());
            assert_eq!(out, want);
            assert!(m.sent.borrow().is_empty() && buf.is_wiped());
        }
    }
}
