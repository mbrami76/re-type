// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami
//! Every text of the user interface that is meant to be translated has an entry in every
//! built-in catalogue, and no catalogue has entries the program no longer uses.

use re_type::i18n::{parse_catalog, BUILTIN, NAME_KEY};
use std::collections::BTreeSet;

/// Reads the Rust string literal that starts at `s[0] == '"'`; returns its value and the rest.
fn literal(s: &str) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut it = s.char_indices().skip(1);
    while let Some((i, c)) = it.next() {
        match c {
            '"' => return Some((out, &s[i + 1..])),
            '\\' => match it.next()?.1 {
                'n' => out.push('\n'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'u' => {
                    let rest: String = it.by_ref().map(|x| x.1).take_while(|c| *c != '}').collect();
                    out.push(char::from_u32(u32::from_str_radix(rest.trim_start_matches('{'), 16).ok()?)?);
                }
                '\n' | '\r' => {
                    // line continuation: skip the line break and the indentation of the next line
                    let mut rest = it.clone().peekable();
                    while let Some((_, c)) = rest.peek() {
                        if c.is_whitespace() {
                            rest.next();
                            it.next();
                        } else {
                            break;
                        }
                    }
                }
                _ => return None,
            },
            c => out.push(c),
        }
    }
    None
}

/// Texts the program asks to have translated: `tr("..")`, `trf("..")`, `const T_*: &str = ".."`,
/// `const CHECK_HINT`, and page names `(Page::X, "..")`.
fn keys_in(src: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for marker in ["tr(", "trf(", "const T_", "const CHECK_HINT", "(Page::"] {
        let mut from = 0;
        while let Some(p) = src[from..].find(marker) {
            let at = from + p + marker.len();
            from = at;
            if (marker == "tr(" || marker == "trf(") && src[..at - marker.len()].chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_' ) {
                continue; // e.g. `entry(` or `str(`
            }
            let rest = &src[at..];
            let rest = match marker {
                "tr(" | "trf(" => rest.trim_start(),
                _ => match rest.find(['"', '\n', ')']) {
                    Some(i) if rest.as_bytes()[i] == b'"' => &rest[i..],
                    _ => continue,
                },
            };
            if let Some(after_q) = rest.strip_prefix('"') {
                let _ = after_q;
                if let Some((v, _)) = literal(rest) {
                    keys.insert(v);
                }
            }
        }
    }
    keys
}

#[test]
fn every_catalogue_has_every_text_and_nothing_else() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/platform/windows");
    let mut keys = BTreeSet::new();
    for f in ["settings.rs", "mod.rs", "tray.rs", "templates.rs"] {
        if let Ok(s) = std::fs::read_to_string(dir.join(f)) {
            keys.extend(keys_in(&s));
        }
    }
    keys.extend(re_type::help::keys());
    keys.extend(re_type::about::keys());
    keys.extend(re_type::hotkey::name_keys());
    assert!(keys.len() > 80, "the scan found only {} texts", keys.len());
    for (code, json) in BUILTIN {
        let (cat, _) = parse_catalog(json).unwrap();
        let have: BTreeSet<String> = cat.entries.keys().cloned().collect();
        let missing: Vec<_> = keys.difference(&have).collect();
        let unused: Vec<_> = have.difference(&keys).collect();
        assert!(missing.is_empty(), "{code}: missing {missing:?}");
        assert!(unused.is_empty(), "{code}: unused {unused:?}");
        assert!(json.contains(NAME_KEY));
    }
}

#[test]
fn access_keys_survive_translation() {
    // a translated menu or label has at most one access key marker, like the English text
    for (code, json) in BUILTIN {
        let (cat, _) = parse_catalog(json).unwrap();
        for (en, tr) in &cat.entries {
            let amp = |s: &str| s.matches('&').count() - s.matches("&&").count() * 2;
            assert!(amp(tr) <= amp(en).max(1), "{code}: {tr:?}");
            assert_eq!(amp(en) > 0, amp(tr) > 0, "{code}: access key lost or added in {tr:?}");
        }
    }
}
