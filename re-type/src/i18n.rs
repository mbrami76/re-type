// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami
//! Translations (REQ-I18N-001, REQ-I18N-002).
//!
//! A catalogue is a JSON file that maps an English text to its translation. It is *data only*:
//! a translation is never used as a format string, a path, a command or an address, and it is
//! only ever shown as text. The loader is strict, so that a file written by somebody else cannot
//! change what the program does, whatever it contains:
//!
//! * the file size, the number of entries and the length of every text are limited;
//! * every value must be a JSON string; anything else is ignored;
//! * control characters, invisible characters and text-direction overrides are refused;
//! * `{...}` tokens (placeholders such as `{0}`) must be exactly the ones of the English text,
//!   otherwise the entry is ignored and the English text is shown;
//! * a file that cannot be read or parsed is ignored as a whole.
//!
//! The built-in catalogues are embedded in the program. Users can add their own in the `lang`
//! folder next to `settings.json`; a user file wins over a built-in one with the same code.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Biggest catalogue file that is read.
pub const MAX_FILE_BYTES: u64 = 256 * 1024;
/// Most entries taken from one catalogue.
pub const MAX_ENTRIES: usize = 1_000;
/// Longest translated text, in characters.
pub const MAX_TEXT_CHARS: usize = 4_000;
/// Longest language name, in characters.
pub const MAX_NAME_CHARS: usize = 40;
/// Key that holds the name of the language in its own language.
pub const NAME_KEY: &str = "_name";

/// Built-in catalogues: (code, JSON text). English is the source language and needs none.
pub const BUILTIN: &[(&str, &str)] = &[
    ("it", include_str!("../lang/it.json")),
    ("fr", include_str!("../lang/fr.json")),
    ("de", include_str!("../lang/de.json")),
    ("es", include_str!("../lang/es.json")),
    ("pt", include_str!("../lang/pt.json")),
];

/// One validated catalogue.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalog {
    pub name: String,
    pub entries: HashMap<String, String>,
}

static ACTIVE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
static ACTIVE_CODE: Mutex<String> = Mutex::new(String::new());

/// Code of the language in use (`en` when none was loaded).
pub fn active_code() -> String {
    ACTIVE_CODE.lock().map(|c| if c.is_empty() { "en".to_string() } else { c.clone() }).unwrap_or_else(|_| "en".into())
}

fn set_code(code: &str) {
    if let Ok(mut c) = ACTIVE_CODE.lock() {
        *c = code.to_string();
    }
}

/// Returns the `{...}` tokens of `s`, in order.
fn tokens(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(a) = rest.find('{') {
        match rest[a..].find('}') {
            Some(b) => {
                out.push(&rest[a..a + b + 1]);
                rest = &rest[a + b + 1..];
            }
            None => break,
        }
    }
    out
}

/// True for characters a translation must not contain: controls (except a line feed), invisible
/// characters and the text-direction overrides that can make text read differently from how it
/// is stored.
fn forbidden(c: char) -> bool {
    (c.is_control() && c != '\n')
        || matches!(c, '\u{ad}' | '\u{61c}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}' | '\u{fff9}'..='\u{fffb}')
}

fn same_tokens(a: &str, b: &str) -> bool {
    let (mut x, mut y) = (tokens(a), tokens(b));
    x.sort_unstable();
    y.sort_unstable();
    x == y
}

/// Validates a language code such as `it`, `pt-br`.
pub fn valid_code(code: &str) -> bool {
    let parts: Vec<&str> = code.split('-').collect();
    let ok = |p: &str, lo: usize, hi: usize| (lo..=hi).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_lowercase());
    match parts.as_slice() {
        [a] => ok(a, 2, 3),
        [a, b] => ok(a, 2, 3) && ok(b, 2, 4),
        _ => false,
    }
}

/// Parses and validates a catalogue. Returns it with one note per ignored entry; `Err` when
/// the file as a whole is unusable.
pub fn parse_catalog(json: &str) -> Result<(Catalog, Vec<String>), String> {
    if json.len() as u64 > MAX_FILE_BYTES {
        return Err("file too big".into());
    }
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("not valid JSON: {e}"))?;
    let obj = value.as_object().ok_or("not a JSON object")?;
    let mut cat = Catalog::default();
    let mut notes = Vec::new();
    for (key, val) in obj.iter().take(MAX_ENTRIES + 1) {
        if cat.entries.len() >= MAX_ENTRIES {
            notes.push("too many entries, the rest is ignored".to_string());
            break;
        }
        let Some(text) = val.as_str() else {
            notes.push(format!("{key:?}: not a text, ignored"));
            continue;
        };
        let chars = text.chars().count();
        if key == NAME_KEY {
            if chars == 0 || chars > MAX_NAME_CHARS || text.chars().any(|c| forbidden(c) || c == '\n') {
                notes.push("_name is not usable, ignored".to_string());
            } else {
                cat.name = text.to_string();
            }
            continue;
        }
        if chars == 0 || chars > MAX_TEXT_CHARS || text.chars().any(forbidden) {
            notes.push(format!("{key:?}: text not allowed, ignored"));
        } else if !same_tokens(key, text) {
            notes.push(format!("{key:?}: {{...}} tokens differ from the English text, ignored"));
        } else {
            cat.entries.insert(key.clone(), text.to_string());
        }
    }
    Ok((cat, notes))
}

/// Reads a user catalogue from `path` (size checked before reading).
fn read_user_file(path: &Path) -> Result<(Catalog, Vec<String>), String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return Err("not a file, or too big".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_catalog(&text)
}

fn user_file(dir: &Path, code: &str) -> Option<(Catalog, Vec<String>)> {
    if !valid_code(code) {
        return None;
    }
    read_user_file(&dir.join(format!("{code}.json"))).ok()
}

fn builtin(code: &str) -> Option<Catalog> {
    BUILTIN.iter().find(|(c, _)| *c == code).and_then(|(_, j)| parse_catalog(j).ok()).map(|(c, _)| c)
}

/// Languages to try for `wanted` (`it-it` tries `it-it`, then `it`).
fn candidates(wanted: &str) -> Vec<String> {
    let w = wanted.trim().to_ascii_lowercase().replace('_', "-");
    let mut v = vec![w.clone()];
    if let Some((base, _)) = w.split_once('-') {
        v.push(base.to_string());
    }
    v
}

/// Loads the catalogue for `setting` (`auto` or a code) and makes it the active one. `system` is
/// the Windows display language (e.g. `it-IT`). Returns the code in use (`en` when none) and notes
/// about ignored entries or files.
pub fn init(setting: &str, system: &str, user_dir: &Path) -> (String, Vec<String>) {
    let wanted = if setting.eq_ignore_ascii_case("auto") || setting.is_empty() { system } else { setting };
    let mut notes = Vec::new();
    for code in candidates(wanted) {
        if code == "en" || code.starts_with("en-") {
            set_active(None);
            set_code("en");
            return ("en".into(), notes);
        }
        if let Some((cat, n)) = user_file(user_dir, &code) {
            notes.extend(n.into_iter().map(|x| format!("{code}.json: {x}")));
            set_active(Some(cat.entries));
            set_code(&code);
            return (code, notes);
        }
        if let Some(cat) = builtin(&code) {
            set_active(Some(cat.entries));
            set_code(&code);
            return (code, notes);
        }
    }
    set_active(None);
    set_code("en");
    ("en".into(), notes)
}

fn set_active(entries: Option<HashMap<String, String>>) {
    if let Ok(mut g) = ACTIVE.lock() {
        *g = entries;
    }
}

/// Languages that can be chosen: English, the built-in ones and the user files; `(code, name)`.
pub fn available(user_dir: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![("en".into(), "English".into())];
    for (code, json) in BUILTIN {
        let name = parse_catalog(json).map(|(c, _)| c.name).unwrap_or_default();
        out.push((code.to_string(), if name.is_empty() { code.to_string() } else { name }));
    }
    if let Ok(rd) = std::fs::read_dir(user_dir) {
        let mut found: Vec<(String, String)> = Vec::new();
        for e in rd.flatten().take(64) {
            let path = e.path();
            let (Some(stem), Some(ext)) = (path.file_stem().and_then(|s| s.to_str()), path.extension().and_then(|s| s.to_str())) else {
                continue;
            };
            if !ext.eq_ignore_ascii_case("json") || !valid_code(stem) {
                continue;
            }
            if let Ok((cat, _)) = read_user_file(&path) {
                let name = if cat.name.is_empty() { stem.to_string() } else { cat.name };
                found.push((stem.to_string(), name));
            }
        }
        found.sort();
        for (code, name) in found {
            match out.iter_mut().find(|(c, _)| *c == code) {
                Some(slot) => slot.1 = name, // a user file replaces a built-in one
                None => out.push((code, name)),
            }
        }
    }
    out
}

/// The text in the active language, or `key` itself (English) when there is no translation.
pub fn tr(key: &str) -> String {
    if let Ok(g) = ACTIVE.lock() {
        if let Some(t) = g.as_ref().and_then(|m| m.get(key)) {
            return t.clone();
        }
    }
    key.to_string()
}

/// Translates a multi-line text line by line: blank lines stay, the indentation of a line is kept,
/// and the rest of each line is looked up with [`tr`] (an unknown line stays English).
pub fn tr_lines(text: &str) -> String {
    text.split('\n')
        .map(|l| {
            let body = l.trim_start_matches(' ');
            if body.is_empty() {
                l.to_string()
            } else {
                format!("{}{}", &l[..l.len() - body.len()], tr(body))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The keys [`tr_lines`] looks up for `text`: its non-blank lines without indentation, once each.
pub fn line_keys(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in text.split('\n') {
        let b = l.trim_start_matches(' ');
        if !b.is_empty() && !out.iter().any(|o| o == b) {
            out.push(b.to_string());
        }
    }
    out
}

/// The translation of `key`, or None when the active language has none.
pub fn lookup(key: &str) -> Option<String> {
    ACTIVE.lock().ok().and_then(|g| g.as_ref().and_then(|m| m.get(key).cloned()))
}

/// Serialises the tests that depend on the active language (it is process-wide state).
#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

/// Holds the language lock and selects English; for tests of code that looks names up.
#[cfg(test)]
pub(crate) fn english_guard() -> std::sync::MutexGuard<'static, ()> {
    let g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    set_active(None);
    if let Ok(mut c) = ACTIVE_CODE.lock() {
        *c = "en".to_string();
    }
    g
}

/// Like [`tr`], then replaces `{0}`, `{1}`... by `args`. Only placeholders are replaced, and the
/// loader guarantees that the translation has the same ones as the English text.
pub fn trf(key: &str, args: &[&str]) -> String {
    let mut t = tr(key);
    for (i, a) in args.iter().enumerate() {
        t = t.replace(&format!("{{{i}}}"), a);
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The active catalogue is global: tests that change it run one at a time.
    use super::TEST_LOCK as LOCK;

    #[test]
    fn built_in_files_are_valid_and_lose_nothing() {
        for (code, json) in BUILTIN {
            let (cat, notes) = parse_catalog(json).unwrap_or_else(|e| panic!("{code}: {e}"));
            assert!(notes.is_empty(), "{code}: {notes:?}");
            assert!(!cat.name.is_empty(), "{code} has no _name");
            assert!(valid_code(code));
        }
    }

    #[test]
    fn code_rules() {
        for ok in ["it", "pt-br", "zh-hans", "fil"] {
            assert!(valid_code(ok), "{ok}");
        }
        for bad in ["", "IT", "i", "it_IT", "it-", "../x", "it/x", "a-b-c", "italian1", "it-123"] {
            assert!(!valid_code(bad), "{bad}");
        }
    }

    #[test]
    fn only_json_strings_with_matching_tokens_are_taken() {
        let j = r#"{"_name":"Test","Save":"Salva","Hello {0}":"Ciao {0}","Hello {1}":"Ciao {0}","Num":5,"Obj":{"a":1},
            "Bad":"x\u0007y","Rtl":"a\u202eb","Zero":"a\u200bb","Empty":"","Tok {x}":"Tok","Extra":"a {0}","Ok {a} {b}":"ok {b} {a}"}"#;
        let (c, notes) = parse_catalog(j).unwrap();
        assert_eq!(c.name, "Test");
        let mut keys: Vec<_> = c.entries.keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["Hello {0}", "Ok {a} {b}", "Save"]);
        assert_eq!(notes.len(), 9, "{notes:?}");
    }

    #[test]
    fn a_new_token_in_a_translation_is_refused() {
        let (c, n) = parse_catalog(r#"{"Open":"Apri {COMMAND}"}"#).unwrap();
        assert!(c.entries.is_empty() && n.len() == 1);
    }

    #[test]
    fn limits_are_enforced() {
        let long = "a".repeat(MAX_TEXT_CHARS + 1);
        let (c, n) = parse_catalog(&format!(r#"{{"k":"{long}"}}"#)).unwrap();
        assert!(c.entries.is_empty() && n.len() == 1);
        let many: String = (0..MAX_ENTRIES + 50).map(|i| format!("\"k{i}\":\"v\"")).collect::<Vec<_>>().join(",");
        let (c, n) = parse_catalog(&format!("{{{many}}}")).unwrap();
        assert_eq!(c.entries.len(), MAX_ENTRIES);
        assert!(!n.is_empty());
        let big = format!("{{\"k\":\"{}\"}}", "a".repeat(MAX_FILE_BYTES as usize));
        assert!(parse_catalog(&big).is_err());
    }

    #[test]
    fn broken_files_are_refused_as_a_whole() {
        for j in ["", "[]", "\"x\"", "{", "null", "{\"a\":"] {
            assert!(parse_catalog(j).is_err(), "{j}");
        }
    }

    #[test]
    fn a_translation_is_never_used_as_a_format_string() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // `{0}` is replaced; any other brace text stays text
        let mut e = HashMap::new();
        e.insert("Hi {0}".to_string(), "Ciao {0} %s %n {}".to_string());
        set_active(Some(e));
        assert_eq!(trf("Hi {0}", &["Mario"]), "Ciao Mario %s %n {}");
        set_active(None);
        assert_eq!(trf("Hi {0}", &["Mario"]), "Hi Mario");
    }

    #[test]
    fn unknown_text_stays_english_and_codes_resolve() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join("re-type-i18n-none");
        let (code, _) = init("it", "en-US", &dir);
        assert_eq!(code, "it");
        assert_eq!(active_code(), "it");
        assert_eq!(tr("Never translated text"), "Never translated text");
        assert_eq!(init("auto", "fr-FR", &dir).0, "fr");
        assert_eq!(init("auto", "pt-BR", &dir).0, "pt");
        assert_eq!(init("auto", "ja-JP", &dir).0, "en");
        assert_eq!(init("../../etc/passwd", "en", &dir).0, "en");
        init("en", "en", &dir);
    }

    #[test]
    fn user_files_win_and_bad_ones_are_ignored() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("re-type-i18n-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("nl.json"), r#"{"_name":"Nederlands","Save":"Opslaan"}"#).unwrap();
        std::fs::write(dir.join("it.json"), r#"{"_name":"Mio","&Close":"&Chiudi!"}"#).unwrap();
        std::fs::write(dir.join("xx.json"), "not json").unwrap();
        std::fs::write(dir.join("Bad Name.json"), r#"{"_name":"x"}"#).unwrap();
        let av = available(&dir);
        assert!(av.contains(&("nl".into(), "Nederlands".into())));
        assert!(av.contains(&("it".into(), "Mio".into())));
        assert!(!av.iter().any(|(c, _)| c == "xx" || c.contains(' ')));
        assert_eq!(init("nl", "en", &dir).0, "nl");
        assert_eq!(tr("Save"), "Opslaan");
        assert_eq!(init("it", "en", &dir).0, "it");
        assert_eq!(tr("&Close"), "&Chiudi!");
        // a broken user file falls back to English, not to a built-in of the same name
        assert_eq!(init("xx", "en", &dir).0, "en");
        init("en", "en", &dir);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
