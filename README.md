# Re:Type

[![Help needed: translations](https://img.shields.io/badge/help%20needed-translations-orange)](#help-needed-translations)

Re:Type types the text on the Windows clipboard into the field you are working in, as simulated keystrokes. It is an assistive tool for places where pasting is blocked or unreliable: remote consoles, virtual machines, password fields, installers and legacy applications.

Press a global hotkey and the clipboard text is typed, character by character, into whatever has the keyboard focus. Press **Esc** to stop.

![Re:Type icon](re-type/assets/re-type-256.png)

## Features

- **Hotkey typing** with a configurable global hotkey (default `Ctrl+Alt+Shift+V`), plus a tray command that types into the window you were working in.
- **Pick target and type**: click the exact field that should receive the text (optional shortcut, can be switched off).
- **Safe by default**: line breaks and tabs of the clipboard text are skipped, special characters are mapped to plain ASCII punctuation, the clipboard is emptied after a successful run, and text longer than a limit is refused.
- **Private**: no network access, no copy of the text is kept (the typing buffer is overwritten after every run), the log never contains typed text.
- **Templates**: add text and a few navigation keys (Tab, Enter, arrows, Home, End, Backspace, Delete) before or after the clipboard text, each template with its own hotkey. Templates cannot press shortcuts or function keys on purpose.
- **Windows integration**: tray icon, start at sign-in, optional run as administrator (with the Windows consent prompt), dark mode and high contrast, full keyboard and screen-reader use, in-app Help.

Status: version 1.0.0, Windows 10 and 11 (64-bit). Linux (KDE, GNOME) is planned. Languages: English, Italian, French, German, Spanish and Portuguese (see [Help needed: translations](#help-needed-translations)).

## Quick start

1. Build or download `re-type.exe` and start it. An icon appears in the system tray.
2. Copy some text, click the field where it should go, press `Ctrl+Alt+Shift+V`, and release the keys.
3. Right-click the tray icon for the other commands; open **Settings** for every option. The **Help** page explains each option and the warnings.

More: [user guide](docs/user-guide.md).

## Build and test

You need Rust (stable, 1.75 or later).

```
cd re-type
cargo test                                                    # unit tests, any operating system
cargo build --release                                         # target\release\re-type.exe
```

Windows executables can also be built from Linux with Zig as the linker; see [docs/building.md](docs/building.md).

## Documentation

| Document | Content |
|---|---|
| [docs/user-guide.md](docs/user-guide.md) | using the program, every option, known limits |
| [docs/requirements.md](docs/requirements.md) | requirements and assumptions |
| [docs/design.md](docs/design.md) | architecture (with diagrams), design choices, file map |
| [docs/testing.md](docs/testing.md) | automated tests and missing tests |
| [docs/building.md](docs/building.md) | building |
| [CONTRIBUTING.md](CONTRIBUTING.md) | what is in scope, how to contribute |
| [SECURITY.md](SECURITY.md) | reporting a vulnerability privately |
| [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) | libraries and their licences |

## Help needed: translations

The interface is available in six languages. Only English and Italian have been checked by a native speaker; all other translations were prepared without review and may contain mistakes or awkward wording. Corrections and new languages are very welcome: please use the [project's issues](https://github.com/mbrami76/re-type/issues). While one of those languages is in use, the About page of the program shows the same warning and a button that opens the issues page.

| Language | Code | Status |
|---|---|---|
| English | `en` (built in) | validated |
| Italiano | `it` | validated |
| Français | `fr` | **NOT validated** |
| Deutsch | `de` | **NOT validated** |
| Español | `es` | **NOT validated** |
| Português | `pt` | **NOT validated** |

**How to help.** A translation is one JSON file in [`re-type/lang`](re-type/lang): the keys are the English texts, the values are your translation. Copy `it.json` to your language code (for example `nl.json`), translate the values, keep the `{0}`-style placeholders and the `&` access-key marker (one per text, on a different letter for each control of the same page) unchanged, and open a pull request (or an issue if you prefer to send the text). `cargo test` checks that every text is present and that the file is valid.

To try a file without building, put it in the `lang` folder next to `settings.json` (`%APPDATA%\re-type\lang`) and choose the language on the Appearance page. A translation file is data only: it can never run code, and anything unusual in it (control characters, text-direction marks, changed `{...}` tokens, too long a text) is ignored. Details are in [docs/design.md](docs/design.md).

The Help and About pages are translated too; the GNU General Public License notice on the About page stays in English.

## Development

The development of this project included generative AI tools. The author reviews and tests everything that is published.

## Support

If you find this application useful, you can support my free and open-source work through Liberapay: <https://liberapay.com/mbrami76>

## Licence

Copyright (C) 2026 Michele Brami — <https://michelebrami.it>

Re:Type is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version. It is distributed in the hope that it will be useful, but **without any warranty**; without even the implied warranty of merchantability or fitness for a particular purpose. See [LICENSE](LICENSE).

Source code: <https://github.com/mbrami76/re-type>
