# Re:Type – Tests

Run the automated tests on any operating system:

```
cargo test
```

## Automated tests (version 1.0.0): 85 tests, all passing

| Area | Tests | What is covered |
|---|---|---|
| `config` | 14 | loading, defaults, validation and limits of `settings.json`, corrupt and partial files |
| `actions` | 11 | building the key actions from text and templates |
| `typing` | 11 | the typing run with a fake backend: modes, abort, length limit, clipboard handling |
| `i18n` | 9 | language files: loading, limits, rejected control characters and changed `{...}` tokens |
| `highlight` | 8 | shortcut and pattern marking, key order, local key names |
| `pattern` | 6 | template parser and its limits |
| `safechars` | 6 | safe-character mapping |
| `help` | 4 | Help text structure |
| `hotkey` | 4 | parsing and printing of key combinations, suggestions |
| `secure` | 3 | zeroing of the typing buffer |
| `about`, `log` | 2 + 2 | About text, log writing |
| `tests/i18n_catalogs.rs` | 2 | every built-in language has every text, Help, About and key name |
| `tests/ids.rs` | 1 | control identifiers are unique |
| `tests/licence.rs` | 2 | licence headers and third-party notices |

## Missing automated tests

- The whole Windows layer (`platform/windows`: clipboard, keystroke injection, global hotkeys, tray, settings window, shortcut and Help fields, target picker, start with Windows, theme) has no automated tests. It was checked by hand on Windows 11 (build 26300) with scripted screenshots; the results are not repeatable by `cargo test`.
- No end-to-end test that types into a real application.
- No tests for display scales above 100 %, elevated target windows and the UAC case, restarting Explorer (tray icon), hotkeys already taken by another program, keyboard layouts with AltGr, or screen readers.
- No test of a signed build.
- No coverage measurement: the table above counts tests per module, it does not measure covered lines.
