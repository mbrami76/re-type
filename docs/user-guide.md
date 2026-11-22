# Re:Type – User guide

The same text is available inside the program: open **Settings** and choose the **Help** page (or press F1; or use **Help** in the tray menu).

## Using it

Start `re-type.exe`. An icon appears in the system tray. Double-click it, or right-click it and choose **Settings…**. The right-click menu also pauses the hotkeys and quits the application.

The default hotkey is **Ctrl+Alt+Shift+V**. Hold the keys, release them, and the text is typed. Press **Esc** while it types to abort.

### Settings

The window has a list of sections on the left (Hotkeys, Typing, Clipboard, Appearance, Startup, Templates, Help, About) and the options of the selected section on the right. Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+PgDn and Ctrl+PgUp move through the sections from anywhere. The button **settings.json** at the bottom opens the folder that holds the settings file.

- **Hotkeys**: type a combination such as `ctrl+alt+shift+v`. **Check** tells you whether Windows lets the application use it. **Find free** proposes a free Ctrl combination for the same key. The same page switches **Pick target and type** on or off and gives it an optional shortcut of its own (default: none; the tray command is hidden while it is off).
- **Delay before typing**: a short wait so the target window is ready.
- **Delay between keystrokes**: raise it if the target drops characters.
- **Max wait for key release**: the application waits until you let go of the hotkey so held modifiers do not turn the typed text into shortcuts.
- **Longest clipboard text**: longer text is refused before anything is typed.
- **Newline / Tab** (clipboard text only): type them as Enter, Shift+Enter, Space, Tab key, spaces, or skip them. **Default: skipped**, so a copied text can never press Enter or Tab on its own. Line breaks and tabs that you write in a template are always typed as Enter and Tab.
- **Drop trailing newlines**: ignore line breaks at the end of the text, for example from a copied line.

### Templates

A template adds text and a few editing keys before or after the clipboard text, and has its own hotkey and an on/off switch. Templates are deliberately limited to text entry and navigation: they cannot press shortcuts, function keys or Esc. The pattern is plain text with `{...}` tokens:

| Token | Meaning |
|---|---|
| `{CLIPBOARD}` | the clipboard text (required) |
| `{TAB}` `{SHIFT+TAB}` `{ENTER}` | move to the next or previous field, new line |
| `{UP}` `{DOWN}` `{LEFT}` `{RIGHT}` `{HOME}` `{END}` | move the cursor |
| `{BACKSPACE}` `{DEL}` | delete a character |
| `{TAB 3}` | repeat a key 3 times (1 to 5) |
| `{DELAY 200}` | wait 200 ms (at most 1000) |
| `{{` `}}` | literal `{` and `}` |

Everything else is typed as written. Examples:

- `{CLIPBOARD}{TAB}`: type the text, then move to the next field.
- `ID-{CLIPBOARD}{ENTER}`: type a prefix, the text, then Enter.

A template that uses anything else (for example `{CTRL+A}` or `{F5}`) is refused when you save. If an older settings file contains one, it is switched off when loaded (not deleted) and a notice tells you why.

### Startup and administrator rights (Startup page)

- **Start Re:Type when I sign in to Windows**: adds a per-user entry (no administrator rights needed). Off removes it. If you move the program file, Re:Type repairs the entry the next time you start it. If you switched the entry off in Task Manager (Startup apps), Windows keeps it off; the page says so, and saving with the option on turns it back on.
- **Always run Re:Type as administrator**: at start, Re:Type asks Windows for administrator rights (UAC consent prompt) and restarts itself. If you decline, it keeps running without them and shows a notice. When you switch the option on, Re:Type offers to restart at once. The window title and tray tooltip say "(administrator)" when it runs elevated.
- **Tray command *Restart as administrator***: restarts Re:Type with administrator rights for this run only, after the Windows consent prompt. It is shown only while Re:Type runs without these rights. Nothing is saved; if you decline, Re:Type keeps running as before.
- **Warnings**: an administrator program can change the whole system, so switch this on only if you need to type into windows that run as administrator, and only run a Re:Type file that you trust and that only administrators can change. Re:Type never starts elevated silently (no scheduled task with highest privileges), so together with *Start at sign-in* the consent prompt appears at every sign-in. The consent prompt, the secure desktop and the sign-in screen can never be typed into.
- Unsigned builds show "Unknown publisher" in the consent prompt (yellow banner); signing the exe fixes that.

### Help

The **Help** page of the settings window (also F1, or the tray command **Help**) explains every option and mode with the warnings, in a read-only text field you can scroll with the keyboard.

### Appearance and accessibility

- **Language** (Appearance page): Automatic (follows the Windows display language, default), English, Italiano, Français, Deutsch, Español or Português. The tray menu changes at once; the settings window changes when you open it again. Help and About are English only. All languages except English and Italian are not yet checked by native speakers (see the README); when one of them is in use the About page says so and has a button to report problems on the project's issue page. To add or replace a language, put a file `xx.json` (for example `nl.json` or `pt-br.json`) in the folder `lang` next to `settings.json`; the easiest way is to copy the English-keyed `it.json` from the source (`re-type/lang`) and translate the values. A language file is data only and cannot run code; entries with anything unusual (control characters, text-direction marks, different `{...}` tokens, too long a text) are ignored and the English text is shown. The log (`re-type.log`) says what was ignored.

- **Colour scheme** (Appearance page): follow Windows (default), light, or dark. It applies to the settings window and to the tray menu. With Windows high contrast on, the high-contrast colours are always used. Message boxes always use the Windows colours (Windows does not let programs change them). Dark menus and controls need Windows 10 version 1903 or later; older builds show the light colours.
- **Keyboard**: Tab and Shift+Tab move through the controls; every label, button and check box has an Alt shortcut (underlined letter); Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+PgDn and Ctrl+PgUp switch pages; Esc closes the window without saving; the explanatory texts and the status line are focusable read-only text fields so that a screen reader can read them.
- **Screen readers**: the window uses standard Windows controls, each named by its label, and reports the result of Check, Find free and Save as a change of the status text. While a screen reader is running, the focus moves to the status field after Check, Find free and Save so that the result is read out; Shift+Tab goes back. It was tested with NVDA 2026.3.1 on Windows 10, not yet with JAWS or Narrator.
- **Tray icon without a mouse**: select the icon (Win+B, arrow keys), press Enter to open the settings, or the menu key or Shift+F10 to open the menu. The menu items have access keys.

## Pick target and type

Tray menu, **Pick target and type…**: click the field you want to type in. A frame follows the control under the pointer and the click is not passed on to the program. Esc or a right click cancels, and the mode ends after 30 seconds. The clipboard is then typed into the field you clicked, with the same options as the hotkey. Re:Type gives the field the keyboard focus directly; for windows that draw their own fields (web pages, WPF, UWP, Qt programs) and for controls that refuse the focus it clicks the picked point once to focus it, so pick a text field, not a button (buttons, static texts and taskbar items are refused). Nothing is remembered after the run.

## Tray menu

Right-click the tray icon (or use the menu key): **Type clipboard now** (types the clipboard into the window you were working in, and into the control that had the focus there, using the same options as the hotkey), **Pick target and type…** (see above), **Settings**, **Help**, **Pause hotkeys**, **Quit**. If Windows does not let Re:Type bring your window back to the front, nothing is typed and a notice is shown.

## Clipboard options

On the *Clipboard* page of the settings window:

- **Special characters** (default *Plain punctuation and spaces*): *Type as they are*, *Plain punctuation and spaces* (curly quotes, long dashes, ellipsis and special spaces become ASCII; invisible characters are removed, except the left-to-right and right-to-left marks) or *ASCII only* (also removes accents, writes EUR for the euro sign, `?` where there is no equivalent). Template text is never converted.
- **Copied files**: when files are copied in Explorer, Re:Type types their full paths, only their names, or nothing. Images and other content that is not text are never typed; a notice is shown.
- **Clear the clipboard after typing** (on by default): after a successful run the clipboard is emptied. It is kept after an abort, a failure, an image or other non-text content, or when something newer was copied while typing. Windows clipboard history (Win+V), cloud clipboard and clipboard managers may keep their own copy; Re:Type cannot remove it. Switch the option off to paste the same content repeatedly.
- **Clipboard history tip:** if Windows clipboard history is on (Win+V), open it, select the item you want, then press the hotkey to type it; you can switch between items this way. Re:Type empties only the current clipboard after typing (if the option above is on); the items kept in the history are not removed, so you can choose another one and type again.
- Braces in templates: write `{{` and `}}` for a literal `{` and `}`.

## Privacy and safety

- The application opens no network connections.
- Clipboard text is copied into a buffer only for the duration of a typing run. The buffer is overwritten with zeros when the run ends, whether it completes, is aborted, fails, or a panic occurs inside the run. The application does not store, log or write the text anywhere. The log records only events and counts.
- The text still exists in the Windows clipboard itself, and in the target program once typed. Re:Type cannot clear those.
- The application runs without administrator rights unless you switch on *Always run Re:Type as administrator* on the Startup page. Windows does not let a normal program type into a window that runs as administrator; Re:Type then shows a notice instead of typing.

## Files

Both are in `%APPDATA%\re-type\`:

- `settings.json`: settings (the button "settings.json" at the bottom of the settings window opens its folder; a file named config.json from older versions is renamed on first start). An unreadable file is renamed to `settings.json.bad` and defaults are used.
- `re-type.log`: events and errors, rotated at 256 KiB.


## Known limits

- Hotkey conflict detection only sees hotkeys that other programs registered with Windows. A shortcut handled inside a program (for example Ctrl+Shift+V in a browser) cannot be detected; it only applies while that program has the focus.
- Programs that read raw scan codes (some games, some remote-desktop clients) may ignore the typed characters.
- Typing is as fast as the target accepts it; use the keystroke delay for slow targets.
- Re:Type types into whatever has the keyboard focus. If the focus is not on a text field, the keystrokes act as commands (Tab and Enter included). Click into the target field before pressing the hotkey.
- In web browsers the Tab key moves the focus out of a text box. If your text contains tabs, set **Tab is typed as** to *Spaces* or *(skipped)*.
- **Windows running as administrator:** Windows does not let a normal program send keystrokes to an elevated window (for example Notepad started as administrator). Re:Type detects this, types nothing and shows a notice; start Re:Type as administrator to type there (checked on Windows 10: with Re:Type elevated, the hotkey, *Type clipboard now* and *Pick target and type* all worked in an elevated Notepad). In *Pick target and type* the click on an elevated window is not seen at all, so the mode ends after 30 seconds without typing.
- Dark mode, screen readers and keyboard use were checked on Windows 10 only, and screen readers only with NVDA (not JAWS or Narrator). Windows 11 and display scales other than 100 % are less tested.

