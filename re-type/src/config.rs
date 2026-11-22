// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Configuration: model, lenient loading, validation, atomic saving
//! (REQ-DATA-001, REQ-FUNC-002, REQ-FUNC-004, REQ-FUNC-006, REQ-SAFE-001).

use crate::actions::{NewlineMode, TabMode, TextOptions};
use crate::hotkey::Hotkey;
use crate::pattern;
use crate::pattern::Segment;
use crate::safechars::SafeMode;
use crate::typing::{FilesMode, RunOptions};
use crate::APP_DIR;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Default contains Ctrl and V plus two more modifiers (REQ-FUNC-002).
pub const DEFAULT_HOTKEY: &str = "ctrl+alt+shift+v";
pub const MAIN_ID: u32 = 1;
pub const PICK_ID: u32 = 2;
pub const TEMPLATE_ID_BASE: u32 = 100;
pub const CONFIG_VERSION: u32 = 1;
/// Name of the settings file, and of the file that older versions used.
pub const SETTINGS_FILE: &str = "settings.json";
pub const LEGACY_SETTINGS_FILE: &str = "config.json";

/// (min, max) accepted for each numeric option.
pub const INITIAL_DELAY_RANGE: (u32, u32) = (0, 5_000);
pub const CHAR_DELAY_RANGE: (u32, u32) = (0, 1_000);
pub const RELEASE_TIMEOUT_RANGE: (u32, u32) = (0, 10_000);
pub const MAX_CHARS_RANGE: (u32, u32) = (1, 1_000_000);
pub const TAB_SPACES_RANGE: (u32, u32) = (1, 16);

/// Colour scheme of the settings window and the tray menu (REQ-UX-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    /// Follow the Windows setting "Choose your default app mode".
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
    pub fn parse_name(s: &str) -> Option<Theme> {
        Theme::ALL.into_iter().find(|t| t.as_str() == s)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    pub name: String,
    pub enabled: bool,
    pub hotkey: String,
    pub pattern: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub hotkey: String,
    pub initial_delay_ms: u32,
    pub char_delay_ms: u32,
    pub release_timeout_ms: u32,
    pub max_chars: u32,
    pub text: TextOptions,
    pub files: FilesMode,
    /// Empty the clipboard after its content was typed successfully (REQ-SEC-005).
    pub clear_clipboard: bool,
    pub theme: Theme,
    /// Language of the user interface: `auto` (follow Windows) or a code such as `it`.
    pub language: String,
    /// Start Re:Type when the user signs in to Windows (per-user Run entry).
    pub start_at_logon: bool,
    /// Ask Windows for administrator rights at start (UAC consent) and restart elevated.
    pub elevated: bool,
    /// "Pick target and type" is available (tray command and optional shortcut).
    pub pick_enabled: bool,
    /// Optional global shortcut of the pick mode; empty means none.
    pub pick_hotkey: String,
    pub templates: Vec<Template>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            hotkey: DEFAULT_HOTKEY.into(),
            initial_delay_ms: 150,
            char_delay_ms: 5,
            release_timeout_ms: 2_000,
            max_chars: 20_000,
            text: TextOptions::default(),
            files: FilesMode::Paths,
            clear_clipboard: true,
            theme: Theme::System,
            language: "auto".into(),
            start_at_logon: false,
            elevated: false,
            pick_enabled: true,
            pick_hotkey: String::new(),
            templates: vec![
                Template {
                    name: "Clipboard + Tab".into(),
                    enabled: false,
                    hotkey: "ctrl+alt+shift+1".into(),
                    pattern: "{CLIPBOARD}{TAB}".into(),
                },
                Template {
                    name: "Clipboard + Enter".into(),
                    enabled: false,
                    hotkey: "ctrl+alt+shift+2".into(),
                    pattern: "{CLIPBOARD}{ENTER}".into(),
                },
            ],
        }
    }
}

/// A hotkey the application has to register, with what it types.
#[derive(Clone, Debug)]
pub struct Binding {
    pub id: u32,
    pub hotkey: Hotkey,
    pub segments: Vec<Segment>,
    pub label: String,
}

fn clamp_u32(v: &Value, range: (u32, u32), default: u32, name: &str, warn: &mut Vec<String>) -> u32 {
    match v.as_u64().or_else(|| v.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64)) {
        Some(n) => (n.min(u64::from(range.1)) as u32).max(range.0),
        None => {
            warn.push(format!("{name} is not a number, default used"));
            default
        }
    }
}

impl Config {
    /// Builds a config from parsed JSON. Missing, wrong-typed or out-of-range values fall back to
    /// the default or are clamped; each case adds a warning. Never fails (REQ-DATA-001).
    pub fn from_value(v: &Value) -> (Config, Vec<String>) {
        let mut c = Config::default();
        let mut w = Vec::new();
        let Some(obj) = v.as_object() else {
            w.push("configuration is not a JSON object, defaults used".into());
            return (c, w);
        };
        if let Some(h) = obj.get("hotkey") {
            match h.as_str() {
                Some(s) => c.hotkey = s.trim().to_string(),
                None => w.push("hotkey is not text, default used".into()),
            }
        }
        if let Some(t) = obj.get("appearance").and_then(Value::as_object) {
            if let Some(s) = t.get("language") {
                match s.as_str().map(|x| x.trim().to_ascii_lowercase()) {
                    Some(l) if l == "auto" || crate::i18n::valid_code(&l) => c.language = l,
                    _ => w.push("appearance.language is invalid, default used".into()),
                }
            }
            if let Some(s) = t.get("theme") {
                match s.as_str().and_then(Theme::parse_name) {
                    Some(m) => c.theme = m,
                    None => w.push("appearance.theme is invalid, default used".into()),
                }
            }
        }
        if let Some(t) = obj.get("typing").and_then(Value::as_object) {
            let d = Config::default();
            let mut num = |key: &str, range, default, out: &mut u32| {
                if let Some(v) = t.get(key) {
                    *out = clamp_u32(v, range, default, &format!("typing.{key}"), &mut w);
                }
            };
            num("initial_delay_ms", INITIAL_DELAY_RANGE, d.initial_delay_ms, &mut c.initial_delay_ms);
            num("char_delay_ms", CHAR_DELAY_RANGE, d.char_delay_ms, &mut c.char_delay_ms);
            num("release_timeout_ms", RELEASE_TIMEOUT_RANGE, d.release_timeout_ms, &mut c.release_timeout_ms);
            num("max_chars", MAX_CHARS_RANGE, d.max_chars, &mut c.max_chars);
            num("tab_spaces", TAB_SPACES_RANGE, d.text.tab_spaces, &mut c.text.tab_spaces);
            if let Some(s) = t.get("newline") {
                match s.as_str().and_then(NewlineMode::parse_name) {
                    Some(m) => c.text.newline = m,
                    None => w.push("typing.newline is invalid, default used".into()),
                }
            }
            if let Some(s) = t.get("tab") {
                match s.as_str().and_then(TabMode::parse_name) {
                    Some(m) => c.text.tab = m,
                    None => w.push("typing.tab is invalid, default used".into()),
                }
            }
            if let Some(s) = t.get("safe_chars") {
                match s.as_str().and_then(SafeMode::parse_name) {
                    Some(m) => c.text.safe = m,
                    None => w.push("typing.safe_chars is invalid, default used".into()),
                }
            }
            if let Some(s) = t.get("files") {
                match s.as_str().and_then(FilesMode::parse_name) {
                    Some(m) => c.files = m,
                    None => w.push("typing.files is invalid, default used".into()),
                }
            }
            if let Some(b) = t.get("clear_clipboard") {
                match b.as_bool() {
                    Some(b) => c.clear_clipboard = b,
                    None => w.push("typing.clear_clipboard is not true/false, default used".into()),
                }
            }
            if let Some(b) = t.get("strip_trailing_newline") {
                match b.as_bool() {
                    Some(b) => c.text.strip_trailing_newline = b,
                    None => w.push("typing.strip_trailing_newline is not true/false, default used".into()),
                }
            }
        }
        if let Some(t) = obj.get("startup").and_then(Value::as_object) {
            for (key, out) in [("at_logon", &mut c.start_at_logon), ("elevated", &mut c.elevated)] {
                if let Some(b) = t.get(key) {
                    match b.as_bool() {
                        Some(b) => *out = b,
                        None => w.push(format!("startup.{key} is not true/false, default used")),
                    }
                }
            }
        }
        if let Some(t) = obj.get("pick_target").and_then(Value::as_object) {
            if let Some(b) = t.get("enabled") {
                match b.as_bool() {
                    Some(b) => c.pick_enabled = b,
                    None => w.push("pick_target.enabled is not true/false, default used".into()),
                }
            }
            if let Some(h) = t.get("hotkey") {
                match h.as_str() {
                    Some(h) => c.pick_hotkey = h.trim().to_string(),
                    None => w.push("pick_target.hotkey is not text, default used".into()),
                }
            }
        }
        if let Some(list) = obj.get("templates") {
            match list.as_array() {
                None => w.push("templates is not a list, defaults used".into()),
                Some(items) => {
                    c.templates.clear();
                    for (i, item) in items.iter().enumerate() {
                        let Some(o) = item.as_object() else {
                            w.push(format!("template #{} ignored (not an object)", i + 1));
                            continue;
                        };
                        let text = |k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                        let name = text("name");
                        let mut t = Template {
                            name: if name.trim().is_empty() { format!("Template {}", i + 1) } else { name },
                            enabled: o.get("enabled").and_then(Value::as_bool).unwrap_or(false),
                            hotkey: text("hotkey").trim().to_string(),
                            pattern: text("pattern"),
                        };
                        // a template that goes beyond text entry is switched off, not deleted
                        if t.enabled {
                            if let Ok(segs) = pattern::parse(&t.pattern) {
                                if let Err(e) = pattern::check_template(&segs) {
                                    t.enabled = false;
                                    w.push(format!("template '{}' was switched off: {e}", t.name));
                                }
                            }
                        }
                        c.templates.push(t);
                    }
                }
            }
        }
        (c, w)
    }

    pub fn to_value(&self) -> Value {
        let mut root = Map::new();
        root.insert("version".into(), json!(crate::config::CONFIG_VERSION));
        root.insert("hotkey".into(), json!(self.hotkey));
        root.insert(
            "typing".into(),
            json!({
                "initial_delay_ms": self.initial_delay_ms,
                "char_delay_ms": self.char_delay_ms,
                "release_timeout_ms": self.release_timeout_ms,
                "max_chars": self.max_chars,
                "newline": self.text.newline.as_str(),
                "tab": self.text.tab.as_str(),
                "tab_spaces": self.text.tab_spaces,
                "strip_trailing_newline": self.text.strip_trailing_newline,
                "safe_chars": self.text.safe.as_str(),
                "files": self.files.as_str(),
                "clear_clipboard": self.clear_clipboard,
            }),
        );
        root.insert("appearance".into(), json!({ "theme": self.theme.as_str(), "language": self.language }));
        root.insert("pick_target".into(), json!({ "enabled": self.pick_enabled, "hotkey": self.pick_hotkey }));
        root.insert("startup".into(), json!({ "at_logon": self.start_at_logon, "elevated": self.elevated }));
        root.insert(
            "templates".into(),
            Value::Array(
                self.templates
                    .iter()
                    .map(|t| json!({"name": t.name, "enabled": t.enabled, "hotkey": t.hotkey, "pattern": t.pattern}))
                    .collect(),
            ),
        );
        Value::Object(root)
    }

    pub fn run_options(&self) -> RunOptions {
        RunOptions {
            text: self.text,
            initial_delay_ms: self.initial_delay_ms,
            char_delay_ms: self.char_delay_ms,
            release_timeout_ms: self.release_timeout_ms,
            max_chars: self.max_chars as usize,
            files: self.files,
            clear_clipboard: self.clear_clipboard,
        }
    }

    /// Everything wrong with this configuration, ready to show to the user. Empty means valid.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let mut seen: HashMap<Hotkey, String> = HashMap::new();
        let mut check = |text: &str, label: &str, errors: &mut Vec<String>| match Hotkey::parse(text) {
            Err(e) => errors.push(format!("{label}: {e}")),
            Ok(hk) => {
                if let Some(other) = seen.get(&hk) {
                    errors.push(format!("{label}: hotkey {hk} is already used by {other}"));
                } else {
                    seen.insert(hk, label.to_string());
                }
            }
        };
        check(&self.hotkey, "Main hotkey", &mut errors);
        if !self.pick_hotkey.is_empty() {
            if self.pick_enabled {
                check(&self.pick_hotkey, "Pick target shortcut", &mut errors);
            } else if let Err(e) = Hotkey::parse(&self.pick_hotkey) {
                errors.push(format!("Pick target shortcut: {e}"));
            }
        }
        for t in &self.templates {
            let label = format!("Template '{}'", t.name);
            match pattern::parse(&t.pattern) {
                Err(e) => errors.push(format!("{label}: {e}")),
                Ok(s) if s.is_empty() => errors.push(format!("{label}: the pattern is empty")),
                Ok(s) => {
                    // a disabled template may be unfinished; it is checked when it is switched on
                    if t.enabled {
                        if let Err(e) = pattern::check_template(&s) {
                            errors.push(format!("{label}: {e}"));
                        }
                    }
                }
            }
            if t.enabled {
                check(&t.hotkey, &label, &mut errors);
            } else if !t.hotkey.is_empty() {
                if let Err(e) = Hotkey::parse(&t.hotkey) {
                    errors.push(format!("{label}: {e}"));
                }
            }
        }
        errors
    }

    /// Hotkeys to register: the main one, plus every enabled template.
    pub fn bindings(&self) -> Result<Vec<Binding>, Vec<String>> {
        let errors = self.validate();
        if !errors.is_empty() {
            return Err(errors);
        }
        let mut out = vec![Binding {
            id: MAIN_ID,
            hotkey: Hotkey::parse(&self.hotkey).expect("validated"),
            segments: vec![Segment::Clipboard],
            label: "Main hotkey".into(),
        }];
        if self.pick_enabled && !self.pick_hotkey.is_empty() {
            out.push(Binding {
                id: PICK_ID,
                hotkey: Hotkey::parse(&self.pick_hotkey).expect("validated"),
                segments: vec![],
                label: "Pick target shortcut".into(),
            });
        }
        for (i, t) in self.templates.iter().enumerate().filter(|(_, t)| t.enabled) {
            out.push(Binding {
                id: TEMPLATE_ID_BASE + i as u32,
                hotkey: Hotkey::parse(&t.hotkey).expect("validated"),
                segments: pattern::parse(&t.pattern).expect("validated"),
                label: t.name.clone(),
            });
        }
        Ok(out)
    }
}

/// Name of the empty marker file that switches on portable mode.
pub const PORTABLE_MARKER: &str = "portable";

/// Folder of the settings, the log and the `lang` folder. Portable mode (REQ-PROC-004): when a file
/// named `portable` or a `settings.json` lies next to the program, everything is kept in the
/// program's own folder and nothing is written elsewhere (apart from the optional start-up
/// entry). Otherwise `%APPDATA%\re-type` is used.
pub fn config_dir_for(exe_dir: Option<&Path>, appdata: Option<PathBuf>) -> PathBuf {
    if let Some(d) = exe_dir {
        if d.join(PORTABLE_MARKER).is_file() || d.join(SETTINGS_FILE).is_file() {
            return d.to_path_buf();
        }
    }
    let base = appdata
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join(APP_DIR)
}

pub fn config_dir() -> PathBuf {
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    config_dir_for(exe_dir.as_deref(), std::env::var_os("APPDATA").map(PathBuf::from))
}

pub fn config_path() -> PathBuf {
    config_dir().join(SETTINGS_FILE)
}

/// Loads the configuration. A missing file gives the defaults. An unreadable file is renamed to
/// `<name>.bad` and the defaults are used (REQ-DATA-001).
pub fn load(path: &Path) -> (Config, Vec<String>) {
    // older versions called the file config.json: take it over once
    if !path.exists() {
        if let Some(legacy) = path.parent().map(|d| d.join(LEGACY_SETTINGS_FILE)).filter(|l| l.exists()) {
            let _ = fs::rename(&legacy, path);
        }
    }
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Config::default(), vec![]),
        Err(e) => return (Config::default(), vec![format!("cannot read {}: {e}", path.display())]),
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(v) => Config::from_value(&v),
        Err(e) => {
            let mut bad = path.as_os_str().to_owned();
            bad.push(".bad");
            let moved = fs::rename(path, &bad).is_ok();
            let note = if moved { "moved to settings.json.bad" } else { "could not be moved aside" };
            (Config::default(), vec![format!("configuration unreadable ({e}); {note}, defaults used")])
        }
    }
}

/// Writes through a temporary file and renames, so a crash never leaves a half-written file.
pub fn save(path: &Path, cfg: &Config) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let text = serde_json::to_string_pretty(&cfg.to_value()).map_err(std::io::Error::other)?;
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("re-type-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn default_is_valid_and_has_ctrl_v_plus_modifiers() {
        let c = Config::default();
        assert!(c.validate().is_empty());
        let hk = Hotkey::parse(&c.hotkey).unwrap();
        assert_eq!(hk.vk, b'V' as u32);
        assert!(hk.mods & crate::hotkey::MOD_CONTROL != 0 && hk.mods.count_ones() >= 2);
    }

    #[test]
    fn round_trip() {
        let d = tmpdir("rt");
        let p = d.join("settings.json");
        let mut c = Config { char_delay_ms: 42, files: FilesMode::Names, ..Config::default() };
        c.text.newline = NewlineMode::Skip;
        c.text.safe = SafeMode::Ascii;
        c.theme = Theme::Dark;
        {
            c.templates[0].enabled = true;
            c.templates[0].pattern = "{HOME}x{CLIPBOARD}".into();
        }
        save(&p, &c).unwrap();
        let (back, warn) = load(&p);
        assert!(warn.is_empty(), "{warn:?}");
        assert_eq!(back, c);
        assert!(!p.with_extension("json.tmp").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let d = tmpdir("missing");
        let (c, w) = load(&d.join("nope.json"));
        assert_eq!(c, Config::default());
        assert!(w.is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn garbage_is_set_aside() {
        let d = tmpdir("bad");
        let p = d.join("settings.json");
        fs::write(&p, "{ not json").unwrap();
        let (c, w) = load(&p);
        assert_eq!(c, Config::default());
        assert_eq!(w.len(), 1);
        assert!(d.join("settings.json.bad").exists() && !p.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn wrong_values_are_clamped_or_defaulted_with_warnings() {
        let v = json!({
            "hotkey": 5,
            "typing": {"char_delay_ms": 999999, "initial_delay_ms": "x", "newline": "sideways",
                       "tab_spaces": 0, "strip_trailing_newline": "yes", "max_chars": -3, "safe_chars": "bogus", "files": 7},
            "appearance": {"theme": "neon"},
            "extra": 1
        });
        let (c, w) = Config::from_value(&v);
        let d = Config::default();
        assert_eq!(c.hotkey, d.hotkey);
        assert_eq!(c.char_delay_ms, CHAR_DELAY_RANGE.1);
        assert_eq!(c.initial_delay_ms, d.initial_delay_ms);
        assert_eq!(c.text.newline, d.text.newline);
        assert_eq!(c.text.tab_spaces, TAB_SPACES_RANGE.0);
        assert_eq!(c.max_chars, d.max_chars);
        assert_eq!(c.text.safe, d.text.safe);
        assert_eq!(c.files, d.files);
        assert_eq!(c.theme, d.theme);
        assert!(w.len() >= 8, "{w:?}");
    }

    #[test]
    fn template_beyond_text_entry_is_switched_off_not_deleted() {
        let v = json!({"templates": [
            {"name": "macro", "enabled": true, "hotkey": "ctrl+alt+shift+1", "pattern": "{CTRL+A}{CLIPBOARD}"},
            {"name": "fine", "enabled": true, "hotkey": "ctrl+alt+shift+2", "pattern": "{CLIPBOARD}{TAB}"}
        ]});
        let (c, w) = Config::from_value(&v);
        assert!(!c.templates[0].enabled);
        assert_eq!(c.templates[0].pattern, "{CTRL+A}{CLIPBOARD}");
        assert!(c.templates[1].enabled);
        assert!(w.iter().any(|m| m.contains("macro") && m.contains("switched off")), "{w:?}");
        assert!(c.validate().is_empty());
    }

    #[test]
    fn non_object_root() {
        let (c, w) = Config::from_value(&json!([1, 2]));
        assert_eq!(c, Config::default());
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn validation_reports_every_problem() {
        let mut c = Config { hotkey: "v".into(), ..Config::default() };
        assert_eq!(c.validate().len(), 1);
        {
            c.hotkey = "ctrl+alt+shift+1".into();
            c.templates[0].enabled = true; // same hotkey as main
            c.templates[1].enabled = true;
            c.templates[1].pattern = "{nokey}".into();
            let e = c.validate();
            assert_eq!(e.len(), 2, "{e:?}");
            assert!(c.bindings().is_err());
        }
    }

    #[test]
    fn bindings_cover_main_and_enabled_templates() {
        let mut c = Config::default();
        assert_eq!(c.bindings().unwrap().len(), 1);
        let _ = &mut c;
        {
            c.templates[1].enabled = true;
            let b = c.bindings().unwrap();
            assert_eq!(b.len(), 2);
            assert_eq!(b[1].id, TEMPLATE_ID_BASE + 1);
            assert_eq!(b[1].hotkey.to_string(), "ctrl+alt+shift+2");
        }
    }

    #[test]
    fn startup_options_round_trip_and_bad_values_fall_back() {
        let (c, w) = Config::from_value(&json!({"startup": {"at_logon": true, "elevated": true}}));
        assert!(c.start_at_logon && c.elevated && w.is_empty());
        let back = Config::from_value(&c.to_value()).0;
        assert!(back.start_at_logon && back.elevated);
        let (c, w) = Config::from_value(&json!({"startup": {"at_logon": "yes", "elevated": 1}}));
        assert!(!c.start_at_logon && !c.elevated);
        assert_eq!(w.len(), 2);
        let d = Config::default();
        assert!(!d.start_at_logon && !d.elevated);
    }

    #[test]
    fn pick_target_switch_and_shortcut() {
        let d = Config::default();
        assert!(d.pick_enabled && d.pick_hotkey.is_empty());
        assert_eq!(d.bindings().unwrap().iter().filter(|b| b.id == PICK_ID).count(), 0);
        let (mut c, w) = Config::from_value(&json!({"pick_target": {"enabled": true, "hotkey": " ctrl+alt+shift+p "}}));
        assert!(w.is_empty() && c.pick_hotkey == "ctrl+alt+shift+p");
        assert_eq!(c.bindings().unwrap().iter().filter(|b| b.id == PICK_ID).count(), 1);
        // switched off: no binding, and the text is kept
        c.pick_enabled = false;
        assert_eq!(c.bindings().unwrap().iter().filter(|b| b.id == PICK_ID).count(), 0);
        assert_eq!(Config::from_value(&c.to_value()).0.pick_hotkey, "ctrl+alt+shift+p");
        // same combination as the main hotkey is refused while on, a bad one is refused either way
        c.pick_enabled = true;
        c.pick_hotkey = c.hotkey.clone();
        assert!(c.validate().iter().any(|e| e.contains("already used")));
        c.pick_enabled = false;
        assert!(c.validate().is_empty());
        c.pick_hotkey = "nonsense".into();
        assert!(!c.validate().is_empty());
        let (c, w) = Config::from_value(&json!({"pick_target": {"enabled": "no", "hotkey": 5}}));
        assert!(c.pick_enabled && c.pick_hotkey.is_empty() && w.len() == 2);
    }

    #[test]
    fn portable_mode_follows_a_marker_or_a_settings_file_next_to_the_program() {
        let base = std::env::temp_dir().join(format!("re-type-portable-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let exe = base.join("exe");
        fs::create_dir_all(&exe).unwrap();
        let appdata = Some(base.join("appdata"));
        // nothing next to the program: the per-user folder
        assert_eq!(config_dir_for(Some(&exe), appdata.clone()), base.join("appdata").join(APP_DIR));
        assert_eq!(config_dir_for(None, appdata.clone()), base.join("appdata").join(APP_DIR));
        // marker file: the program's folder
        fs::write(exe.join(PORTABLE_MARKER), b"").unwrap();
        assert_eq!(config_dir_for(Some(&exe), appdata.clone()), exe);
        fs::remove_file(exe.join(PORTABLE_MARKER)).unwrap();
        // an existing settings file next to the program counts as well
        fs::write(exe.join(SETTINGS_FILE), b"{}").unwrap();
        assert_eq!(config_dir_for(Some(&exe), appdata), exe);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn language_setting_round_trips_and_rejects_odd_values() {
        let mut c = Config::default();
        assert_eq!(c.language, "auto");
        c.language = "pt-br".into();
        assert_eq!(Config::from_value(&c.to_value()).0.language, "pt-br");
        for bad in ["../etc", "it/x", "", "x", "it;rm", "1234"] {
            let v = serde_json::json!({"appearance": {"language": bad}});
            let (c, w) = Config::from_value(&v);
            assert_eq!(c.language, "auto", "{bad}");
            assert_eq!(w.len(), 1, "{bad}");
        }
    }

    #[test]
    fn an_old_config_json_is_taken_over_as_settings_json() {
        let d = std::env::temp_dir().join(format!("re-type-legacy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), r#"{"hotkey": "ctrl+alt+shift+k"}"#).unwrap();
        let (c, w) = load(&d.join("settings.json"));
        assert!(w.is_empty() && c.hotkey == "ctrl+alt+shift+k");
        assert!(d.join("settings.json").exists() && !d.join("config.json").exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
