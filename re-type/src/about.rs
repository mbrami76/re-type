// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Text of the About page (REQ-DOC-002): who wrote the program, under which licence it is
//! distributed, where the source is. Independent of the operating system, so it is unit-tested.


pub const COPYRIGHT_YEAR: &str = "2026";
pub const COPYRIGHT_HOLDER: &str = "Michele Brami";
pub const WEBSITE_URL: &str = "https://michelebrami.it";
pub const SOURCE_URL: &str = "https://github.com/mbrami76/re-type";
pub const ISSUES_URL: &str = "https://github.com/mbrami76/re-type/issues";
pub const LICENSE_URL: &str = "https://www.gnu.org/licenses/gpl-3.0.html";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Texts of the About page that are translated (the licence notice stays in English: only the
/// English text of the GNU General Public License is authoritative).
pub const TAGLINE: &str = "Types the text on the clipboard into the field you are working in, as keystrokes. An assistive tool for places where pasting is blocked.";
pub const L_WEBSITE: &str = "Website";
pub const L_SOURCE: &str = "Source code";
pub const L_LICENCE: &str = "LICENCE";
pub const L_THIRD: &str = "THIRD-PARTY COMPONENTS";
pub const THIRD_TEXT: &str = "Re:Type is written in Rust and uses these libraries, all under licences that are compatible with the GPL: serde_json, serde, serde_core, itoa, memchr and zmij (JSON reading and writing), and windows-sys and windows-link (Windows API declarations). The full list with their licences is in the file THIRD-PARTY-NOTICES.md.";

/// The translated texts, in the order of the page.
pub fn keys() -> Vec<String> {
    [TAGLINE, L_WEBSITE, L_SOURCE, L_LICENCE, L_THIRD, THIRD_TEXT].iter().map(|s| s.to_string()).collect()
}

fn build(t: &dyn Fn(&str) -> String) -> String {
    let s = format!(
        "\
{name} {VERSION}

{tag}

Copyright (C) {COPYRIGHT_YEAR} {COPYRIGHT_HOLDER}
{web}: {WEBSITE_URL}
{src}: {SOURCE_URL}

{lic}
This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with this program (file LICENSE). If not, see {LICENSE_URL}

{third}
{third_text}
",
        name = crate::PRODUCT_NAME,
        tag = t(TAGLINE),
        web = t(L_WEBSITE),
        src = t(L_SOURCE),
        lic = t(L_LICENCE),
        third = t(L_THIRD),
        third_text = t(THIRD_TEXT),
    );
    s.replace("\r\n", "\n").replace('\n', "\r\n")
}

/// The complete About text (CR LF line breaks, plain text) in the language in use.
pub fn text() -> String {
    build(&crate::i18n::tr)
}

#[cfg(test)]
fn text_en() -> String {
    build(&|s: &str| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_holder_licence_warranty_and_source() {
        {
            let t = text_en();
            assert!(t.contains("Michele Brami") && t.contains("2026"));
            assert!(t.contains("version 3") && t.contains("any later version"));
            assert!(t.contains("WITHOUT ANY WARRANTY"));
            assert!(t.contains(SOURCE_URL) && t.contains(WEBSITE_URL));
            assert!(t.contains(crate::PRODUCT_NAME) && t.contains(VERSION));
            assert!(!t.replace("\r\n", "").contains('\n'));
        }
    }

    #[test]
    fn urls_are_https_and_fixed() {
        for u in [WEBSITE_URL, SOURCE_URL, LICENSE_URL] {
            assert!(u.starts_with("https://") && !u.contains(' '));
        }
    }
}
