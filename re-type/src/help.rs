// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! The in-app help text (REQ-DOC-001). Plain text with CR LF line breaks, shown in a read-only
//! edit control, so it is reachable by keyboard and by screen readers. Independent of the
//! operating system so that it can be unit-tested everywhere.


const COMMON_START: &str = "\
RE:TYPE HELP

Re:Type types the text on the clipboard into the field you are working in, as if you pressed the keys yourself. Use it where pasting is blocked: remote desktops, virtual machines, password fields, installers, web forms. It is an assistive tool: it only types text and a few editing keys, and it never reads the screen.

HOW TO USE IT
1. Copy the text (Ctrl+C).
2. Click the field where the text should go, so that the cursor is in it.
3. Press the hotkey (default Ctrl+Alt+Shift+V). Release the keys; Re:Type waits for that, then types.
Press Esc while it types to stop at once. If nothing is typed, a notice appears and says why.
Tip: if Windows clipboard history is on (Win+V), open it, select the item you want, and then press the hotkey to type it. You can switch between items this way. Re:Type empties only the current clipboard after typing (if that option is on); the items kept in the history are not removed, so you can choose another one and type again.

THREE WAYS TO START A RUN
- Hotkey: types into whatever window and field has the keyboard focus.
- Tray icon, Type clipboard now: types into the window you were working in before you clicked the tray icon, and into the field that had the focus there.
- Tray icon, Pick target and type: you click the field you want, anywhere on screen. A frame follows the control under the pointer and your click is not passed on to the program. Esc or a right click cancels; the mode ends by itself after 30 seconds. Buttons, labels, taskbar items and Re:Type's own windows are refused. Programs that draw their own fields (web browsers, WPF, Qt, Java) have no separate control per field, so Re:Type clicks the picked point once to put the cursor there. Warning: pick a text field, never a button, because that click is real.
- Tray icon, Restart as administrator: restarts Re:Type with administrator rights for this run only, after the Windows consent prompt (UAC). The command is shown only while Re:Type runs without them. If you decline, nothing changes. Nothing is saved: the next start is normal again, unless the option Always run Re:Type as administrator is on (Startup page).
Nothing is remembered after a run, except your settings.

WARNINGS THAT APPLY EVERYWHERE
- Re:Type types into whatever has the focus. If the focus is not on a text field, the keystrokes act as commands (Tab and Enter included). Click into the right field first.
- Never leave a password or other secret on the clipboard longer than needed. Windows clipboard history (Win+V), the cloud clipboard and clipboard managers may keep their own copy; Re:Type cannot remove it.
- Do not type secrets while someone can watch the screen or while screen sharing is on: the text appears in the field like any typed text.
- Programs that read raw keyboard scan codes (some games and some remote-desktop clients) may ignore the typed characters. Increase the delay between keystrokes for slow targets.

SETTINGS, PAGE BY PAGE
";

const HOTKEY: &str = "\
Hotkeys page
- Hotkey: the key combination that types the clipboard. Write it like ctrl+alt+shift+v. It needs at least one of Ctrl, Alt, Shift or Win, plus one key. Combinations with Ctrl+Alt+Shift are the least likely to clash with other programs.
- Key names: write modifiers and keys in English or in your language, in any letter case (Ctrl, Alt, Shift, Win, Enter, Home...). Re:Type writes recognised names in capitals, shows each one as a small key in reversed colours, underlines in red what it does not recognise or cannot use, and puts the keys in the order Ctrl, Win, Alt, Shift, then the key. A key on its own (Home, F5) is not usable: at least one modifier is needed. There is one Shift; left and right Shift cannot be told apart.
- Listen: records a combination for you. Click it, then press the modifier keys and one key; Esc cancels. You can still edit the result by hand.
- Release Re:Type shortcuts: unregisters Re:Type's own shortcuts until you restore them (same button), save, or close the window, so that Windows can tell you whether a combination is free. Windows does not say which program holds a taken shortcut.
- Check hotkey: asks Windows whether the combination is free. Windows can only report hotkeys that other programs registered globally. A shortcut handled inside a program (for example Ctrl+Shift+V in a browser) cannot be detected; it then only works while that program does not have the focus.
- Find free hotkey: tries a few combinations and fills in the first free one.
- If the hotkey is taken when you save, Re:Type saves your settings anyway, tells you which hotkey failed and keeps running; choose another one.
- Enable \"Pick target and type\" (default on): switches the pick mode (see Three ways to start a run) on or off. Off hides its tray command and it cannot be started at all; the hotkey and Type clipboard now are not affected.
- Shortcut key (optional): a global shortcut that starts the pick mode, like the tray command does. Leave it empty for none (default). It follows the same rules as the main hotkey, must differ from every other hotkey, and works only while the pick mode is enabled. Check hotkey and Find free hotkey work as above. Warning: the click on the picked field is a real click, so pick a text field, never a button.

";

const TYPING: &str = "\
Typing page
- Delay before typing (ms): wait this long after the hotkey, so that you can release the keys. 0 to 5000.
- Delay between keystrokes (ms): pause after each character. Raise it if the target loses characters. 0 to 1000.
- Max wait for key release (ms): if you still hold Ctrl, Alt, Shift or Win after this time, nothing is typed and a notice appears (held keys would change what is typed). 0 to 10000.
- Longest clipboard text (characters): longer text is not typed at all, to protect you from pasting a huge text by accident. 1 to 1,000,000.
- Newline is typed as: how line breaks of the clipboard text are typed: Enter, Shift+Enter, Space or skipped. Default: skipped. Warning: Enter submits forms and runs commands; only choose it when you want that.
- Tab is typed as: Tab key, Spaces or skipped. Default: skipped. In web browsers the Tab key moves the focus out of a text box, so use Spaces if your text contains tabs.
- Spaces per tab: how many spaces replace a tab when Tab is typed as Spaces.
- Drop trailing line breaks: removes the line breaks at the very end of the clipboard text, so that a copied line does not end with an extra Enter.
These options never change text you write in a template; a template is typed as written.

";

const CLIPBOARD: &str = "\
Clipboard page
- Special characters: Type as they are; Plain punctuation and spaces (default: curly quotes, long dashes, the ellipsis and non-breaking spaces become ASCII, invisible characters are removed, except the left-to-right and right-to-left marks that right-to-left text needs); ASCII only (also removes accents, writes EUR for the euro sign and ? for characters with no equivalent). Use ASCII only for systems that mangle accents.
- Clear the clipboard after it was typed successfully (default on): the clipboard is emptied after a successful run. It is kept after an abort, a failure, non-text content, or when something newer was copied while typing. Switch it off to paste the same content many times.
- Copied files are typed as: when files were copied in Explorer, Re:Type types their full paths, only their names, or nothing. Images and other content that is not text are never typed; a notice is shown.

";

const APPEARANCE: &str = "\
Appearance page
- Colour scheme: follow Windows (light or dark), light, or dark. It applies to this window and to the tray menu. When Windows high contrast is on, its colours are always used. Message boxes always use the Windows colours; Windows does not let programs change them. Dark controls need Windows 10 version 1903 or later.

";

const STARTUP: &str = "\
Startup page
- Start Re:Type when I sign in to Windows: adds a per-user entry that starts Re:Type at sign-in. No administrator rights are needed. Switching the option off removes the entry. If you move or rename the program file, Re:Type repairs the entry the next time you start it by hand. If you switched the entry off in Task Manager (Startup apps), Windows keeps it off whatever this option says; the page tells you when that is the case, and saving with the option on turns it back on.
- Always run Re:Type as administrator: when Re:Type is started without administrator rights, it asks Windows for them (the UAC consent prompt) and restarts itself. If you decline, Re:Type keeps running normally and shows a notice.
  Why you might want it: Windows does not let a normal program send keystrokes to a window that runs as administrator (for example a command prompt or an installer started as administrator). Without administrator rights Re:Type refuses and says so, instead of typing nothing silently. With them it types there, and Pick target and type can see clicks on those windows.
  WARNINGS:
  - An administrator program can change the whole system. Only switch this on if you need it, and only run a Re:Type program file that you trust and that only administrators can change.
  - Re:Type never starts elevated silently, on purpose: every elevated start needs your consent. Combined with Start at sign-in, the consent prompt therefore appears each time you sign in; declining keeps Re:Type running without administrator rights.
  - The consent prompt itself, the secure desktop and the Windows sign-in screen can never be typed into, with or without administrator rights.
  - When Re:Type runs as administrator, its window title and tray tooltip say so.
  - Switching the option on saves it; it applies at the next start. Re:Type offers to restart at once.

";

const TEMPLATES: &str = "\
Templates page
A template adds text and a few editing keys before or after the clipboard text, and has its own hotkey and an on/off switch. Use it to fill forms field by field. Templates are deliberately limited to text entry and navigation: they cannot press shortcuts, function keys or Esc.
- Add / Remove: create or delete a template.
- Name: any name; it is shown in the list.
- Enabled: only enabled templates have their hotkey registered.
- Hotkey: like the main hotkey, and different from every other hotkey. Check hotkey and Find free hotkey work as on the Hotkeys page.
- Pattern: plain text typed as written, with {tokens} for keys:
    {CLIPBOARD}  the clipboard text (required)
    {TAB} {SHIFT+TAB} {ENTER}  next field, previous field, new line
    {UP} {DOWN} {LEFT} {RIGHT} {HOME} {END}  move the cursor
    {BACKSPACE} {DEL}  delete a character
    {TAB 3}  repeat a key 3 times (1 to 5)
    {DELAY 200}  wait 200 ms (at most 1000)
    {{ + }}  a literal brace
  Example: ID-{CLIPBOARD}{TAB} types ID-, the text, then moves to the next field.
- Anything else, such as {CTRL+A} or {F5}, is refused when you save. A template from an older settings file that breaks the rules is switched off when Re:Type starts (it is not deleted) and a notice tells you why.
Warning: {ENTER} can submit a form and {TAB} moves the focus, so test a template in a harmless field first.

";

const ABOUT: &str = "\
About page
Shows the version, the author, the licence and the source code address. Re:Type is free software under the GNU General Public License, version 3 or (at your option) any later version, and comes with NO WARRANTY. The Website and Source code buttons open the fixed addresses in your browser.

";

const KEYBOARD: &str = "\
KEYBOARD AND SCREEN READERS
- Tab and Shift+Tab move through the controls. Every label, button and check box has an access key (the underlined letter; hold Alt).
- Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+PgDn and Ctrl+PgUp switch between the pages. F1 opens this Help page. Esc closes the window without saving.
- The explanation texts and the status line are read-only fields you can reach with Tab, so that a screen reader can read them. While a screen reader runs, the focus moves to the status line after Check, Find free and Save, so that the result is read out; Shift+Tab goes back.
- Tray icon without a mouse: Win+B, arrow keys to the icon, Enter opens the settings, the menu key or Shift+F10 opens the menu.

";

const LIMITS: &str = "\
KNOWN LIMITS
- Windows running as administrator: see the Startup page above.
- Portable use: Re:Type needs no installation. Put an empty file named portable (or an existing settings.json) next to re-type.exe, and it keeps its settings in that folder instead of your user profile.
- Re:Type needs Windows 10 or 11, 64-bit. It opens no network connections, keeps no copy of the typed text (the typing buffer is wiped after every run) and writes only a small log of events (never of text) next to the settings file.
- The settings file is a text file; the path is shown at the bottom of this window. If it is damaged, Re:Type uses the defaults and shows a notice; your old file is kept.
";

/// The help text in English, with `\n` line breaks.
pub fn english() -> String {
    let mut t = String::from(COMMON_START);
    for part in [HOTKEY, TYPING, CLIPBOARD, APPEARANCE, STARTUP, TEMPLATES, ABOUT, KEYBOARD, LIMITS] {
        t.push_str(part);
    }
    t
}

/// What a line of the help is, so that a window can format it (REQ-DOC-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Title,
    /// A heading in capitals.
    Section,
    /// A heading of warnings (shown in a warning colour).
    WarnHeading,
    /// The name of a settings page.
    Page,
    Body,
    /// Body text that continues an item (indented).
    Indented,
    /// A numbered step ("1. ...").
    Step,
    Bullet,
    SubBullet,
    /// A pattern token with its explanation.
    Code,
    /// A line that starts with "Warning:".
    Warning,
    /// A line that starts with "Tip:".
    Tip,
}

/// One non-blank line of the help: how to format it, and the English text that is its translation key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub kind: Kind,
    pub key: String,
}

/// The help split into formatted lines, in order.
pub fn blocks() -> Vec<Block> {
    let mut out = Vec::new();
    for (i, line) in english().split('\n').enumerate() {
        let body = line.trim_start_matches(' ');
        if body.is_empty() {
            continue;
        }
        let indent = line.len() - body.len();
        let upper = body.chars().any(|c| c.is_alphabetic()) && !body.chars().any(|c| c.is_lowercase());
        let kind = if i == 0 {
            Kind::Title
        } else if body.starts_with('{') {
            Kind::Code
        } else if body.starts_with("- ") {
            if indent >= 2 { Kind::SubBullet } else { Kind::Bullet }
        } else if body.starts_with("Warning:") {
            Kind::Warning
        } else if body.starts_with("Tip:") {
            Kind::Tip
        } else if body == "WARNINGS:" || body == "WARNINGS THAT APPLY EVERYWHERE" {
            Kind::WarnHeading
        } else if upper {
            Kind::Section
        } else if body.as_bytes()[0].is_ascii_digit() && body[1..].starts_with(". ") {
            Kind::Step
        } else if body.ends_with(" page") && !body.contains(':') {
            Kind::Page
        } else if indent > 0 {
            Kind::Indented
        } else {
            Kind::Body
        };
        out.push(Block { kind, key: body.to_string() });
    }
    out
}

/// The texts of the help that are translated, one per line (see `i18n::tr_lines`).
pub fn keys() -> Vec<String> {
    crate::i18n::line_keys(&english())
}

/// The complete help text in the language in use.
pub fn text() -> String {
    crate::i18n::tr_lines(&english()).replace("\r\n", "\n").replace('\n', "\r\n")
}

/// The English help text with CR LF line breaks (used by the tests, independent of the language).
#[cfg(test)]
fn text_en() -> String {
    english().replace("\r\n", "\n").replace('\n', "\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_settings_page_and_option_is_explained() {
        {
            let t = text_en();
            for page in ["Hotkeys page", "Typing page", "Clipboard page", "Appearance page", "Startup page", "About page"] {
                assert!(t.contains(page), "{page} missing");
            }
            for option in [
                "Delay before typing",
                "Delay between keystrokes",
                "Max wait for key release",
                "Longest clipboard text",
                "Newline is typed as",
                "Tab is typed as",
                "Spaces per tab",
                "Drop trailing line breaks",
                "Special characters",
                "Clear the clipboard",
                "Copied files are typed as",
                "Colour scheme",
                "Start Re:Type when I sign in",
                "Always run Re:Type as administrator",
                "Pick target and type",
                "Enable \"Pick target and type\"",
                "Shortcut key (optional)",
                "Type clipboard now",
            ] {
                assert!(t.contains(option), "{option} missing");
            }
            assert!(t.contains("WARNINGS"));
        }
    }

    #[test]
    fn line_breaks_are_crlf_only() {
        let t = text_en();
        assert!(!t.replace("\r\n", "").contains('\n'));
        assert!(!t.replace("\r\n", "").contains('\r'));
    }

    #[test]
    fn template_help_matches_the_rules() {
        // the token list in the help must stay in step with the checker
        let t = text_en();
        assert!(t.contains("1 to 5") && t.contains("at most 1000"));
        assert_eq!(crate::pattern::TEMPLATE_MAX_REPEAT, 5);
        assert_eq!(crate::pattern::TEMPLATE_MAX_DELAY_MS, 1000);
    }

    #[test]
    fn blocks_cover_every_line_and_find_the_structure() {
        let b = blocks();
        let keys: Vec<&String> = b.iter().map(|x| &x.key).collect();
        for k in keys_of_english() {
            assert!(keys.contains(&&k), "{k}");
        }
        assert_eq!(b[0].kind, Kind::Title);
        let kind_of = |start: &str| b.iter().find(|x| x.key.starts_with(start)).unwrap().kind;
        assert_eq!(kind_of("HOW TO USE IT"), Kind::Section);
        assert_eq!(kind_of("WARNINGS THAT"), Kind::WarnHeading);
        assert_eq!(kind_of("Hotkeys page"), Kind::Page);
        assert_eq!(kind_of("3. Press"), Kind::Step);
        assert_eq!(kind_of("- Hotkey:"), Kind::Bullet);
        assert_eq!(kind_of("{CLIPBOARD}"), Kind::Code);
        assert_eq!(kind_of("Warning: {ENTER}"), Kind::Warning);
        assert_eq!(kind_of("Tip:"), Kind::Tip);
        assert_eq!(b.iter().filter(|x| x.kind == Kind::SubBullet).count(), 5);
    }

    fn keys_of_english() -> Vec<String> {
        crate::i18n::line_keys(&english())
    }
}
