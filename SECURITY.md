# Security policy

## Reporting a vulnerability

Please report security problems privately, with GitHub's private vulnerability reporting: open the **Security** tab of the repository, choose **Report a vulnerability** and fill in the form. Only the maintainer sees the report. Do not open a public issue or pull request for a security problem.

Say what you found, how to reproduce it, which version you tested (the About page of the application shows it) and what you think the impact is. You can expect an answer within a few days; this is a one-person project, so a fix may take longer. The report stays private until a fix is released, and you are credited in the release notes if you wish.

**Do not send real secrets.** Use invented text in your reproduction, never passwords, tokens or other people's data.

## What is in scope

- Typed text or clipboard content that is kept after a typing run (in memory longer than needed, in a file, in the log, in the settings) or that reaches anything other than the target window.
- Any network connection: the application makes none.
- A crafted `settings.json` or language file that runs code, crashes the program, or makes it type something the user did not choose.
- Ways to type into a window the user did not select, or to bypass the Escape key abort, the length limit, or the limits on templates.
- Problems with the optional administrator mode or the start-at-sign-in entry: starting elevated without the user's consent, or an entry that can be abused to start another program.
- A released executable or package that does not match the published source.

## What is out of scope

- An attacker who already has the unlocked computer, administrator rights, or can read the memory of the running program.
- Other programs reading the clipboard before Re:Type empties it, or Windows clipboard history (Win+V), which stores what is copied.
- Applications that record or intercept simulated keystrokes.
- Findings that need a modified build.
- The "unknown publisher" warning on unsigned builds.

## Supported versions

Only the latest release receives fixes.
