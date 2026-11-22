# Contributing

Thank you for your interest in Re:Type. Small, focused changes with tests are the easiest to review. For anything larger, open an issue first, so the idea can be discussed before you spend time on it.

Re:Type is an assistive tool: it types the clipboard text where pasting is not possible. It is meant to stay small, predictable and private, and that decides what is in scope.

## In scope

- Bug fixes, with a test that fails without the fix.
- Translations: one JSON file in `re-type/lang`, see the section on translations in `README.md`.
- Compatibility with Windows 10 and 11 (64-bit), with applications that block pasting, and with remote-desktop and virtual-machine consoles.
- Accessibility: keyboard use, screen readers, high contrast, dark mode, display scaling.
- Safer or clearer handling of unusual clipboard content, special characters, keyboard layouts and long texts.
- Documentation, Help text, and tests (the Windows layer has few; see `docs/testing.md`).
- A Linux version for KDE and GNOME (tray, same behaviour). Open an issue before starting: it needs an agreed design.

## Out of scope

These will be declined, whatever the quality of the change:

- Anything that uses the network: telemetry, update checks, cloud sync, online services.
- Keeping clipboard text, a typing history or any copy of typed text, in memory longer than a typing run, on disk or in the log.
- Mouse control, macros, scripting, or templates that can send shortcuts (Ctrl, Alt, Win combinations), function keys, Esc or other keys outside the list in `docs/requirements.md` (REQ-FUNC-005).
- Starting with administrator rights without the user's explicit consent for each start, or any way around the Windows consent prompt, the secure desktop or the sign-in screen.
- Anything that makes a translation file or a settings file able to run code.
- A new dependency without a clear need (the project uses `serde_json` and `windows-sys` only), an installer, or a runtime to install.

## Building and testing

You need Rust (stable, 1.75 or later). Run the commands in the `re-type` directory; `docs/building.md` has the details.

```
cargo test
cargo build --release
```

Before you send a change:

- `cargo test` passes, and a new behaviour has a test;
- if you changed texts shown in the application, add them to every language file in `re-type/lang` (the English text is the key; a catalogue test checks it) or say that the translations are missing;
- tests and examples use invented data only.

## Code style

Follow the code that is already there.

- Every source file starts with `// SPDX-License-Identifier: GPL-3.0-or-later` and `// Copyright (C) 2026 <your name>` (for a new file; keep the existing line in a file you change).
- Documentation and comments are in English. User-visible text is translatable.
- Comments say why, not what the next line does, and each unit cites the requirement IDs (`REQ-...`, see `docs/requirements.md`) it implements.
- Code that calls Windows stays in `re-type/src/platform/windows`; everything else must build and test on any operating system.

## Pull requests

One change per pull request, with a short description of the problem and how you tested it (Windows version, application used). Say so if you could not test on Windows. By contributing you agree that your work is published under GPL-3.0-or-later, the licence of the project (`LICENSE`).

## Security problems

Do not open a public issue for them: see `SECURITY.md`.
