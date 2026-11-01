# Bugs found while driving Yaca by hand

Found on 2026-10-29 by running the installed build
(`%LOCALAPPDATA%\yaca\yaca-desktop.exe`, not a build of the working tree) through
Computer Use: a general run, a close/reopen, and a larger agent task on
`mimo-v2.6-flash` in a fresh project folder. Causes are not investigated; the
"Suspected" lines are guesses, not findings.

## Settings and state that do not survive a restart

1. **Model choice is lost.** Picked `mimo-v2.6-flash` on the home screen; after a
   restart the home screen shows Claude Haiku 5.5 again. The model bound to an
   already-started session is kept; only the default for a new chat is lost.
2. **Effort is lost or inconsistent.** Home screen showed `Medium`; after a restart
   it showed `Extra` with no user action. Not saved, or read from a different
   place than it is written to.
3. **Window size, position and maximized state are lost.** Every start opens a
   small window at a different position.
4. **Session title is lost when the turn was interrupted.** The header had
   "Python CLI todo manager with tests"; after closing mid-turn and reopening, header and
   sidebar both said "Новая сессия". A session that finished its turn kept its
   generated title across a restart, so the title is probably persisted only at
   turn end.

## Providers

5. **Every provider change needs a manual "Опросить".** Changing the base URL,
   protocol, custom headers or the key does not refresh the model list; the user has
   to press "Опросить" by hand each time, and until then the list shows models for
   the old settings. The page text says the list is polled as soon as the key is
   saved, so the behaviour is narrower than the UI promises. Expected: any edit that
   changes how the provider is reached re-polls on its own (debounced), with a visible
   running/ok/failed state, and the old list stays until the new answer arrives.
6. **"Опросить" in the providers list opened the editor instead of polling**
   (seen once on DeepSeek). Might be a layout shift after a new row (MiMo)
   appeared; not reproduced on purpose.

## Window and chrome

7. **Maximize button does nothing on click.** Double-clicking the title bar does
   maximize the window.

## Feed and sidebar

8. **Sidebar title lags the header.** While a session runs, the header already shows
   the generated title and the sidebar still says "Новая сессия".
9. **Empty feed after sending.** In the first run the feed stayed empty for 10+ s
   after sending (no user message, no activity indicator) with the stop button
   already active. The second run showed the user message immediately, so it is
   intermittent.
10. **Plan block does not advance.** The task list kept the first item ("RED: add
    command test fails") marked current after the agent said cycle 1 was green. May be
    the model not updating it rather than the UI.

## Not UI, noted for context

- The agent's `Read` refused a path outside the project (the skill file under
  `~/.claude/plugins/...`) and it carried on; skills from the user's plugin cache are
  loaded, which pulled in the TDD skill for a trivial task.
- `mimo-v2.6-flash` spent about 4 minutes thinking before its first tool call on
  that task.
- The folder picker's path field is not recognised as a file-name box by Computer
  Use, so typing a path there is blocked; a test limitation, not an app bug.

## Checked and fine

- Settings pages (providers, tools, appearance, notifications and language) open and
  return without problems.
- Closing the app leaves no Yaca processes behind, also when closed mid-turn.
- A session interrupted mid-turn is restored whole with an "Остановлено" marker.
- The chosen project folder persists.
- Polling a provider without a key reports `HTTP 401 ... key not set` instead of
  pretending to succeed.
