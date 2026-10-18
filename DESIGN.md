---
name: Yaca
description: A desktop coding agent on a quiet graphite working surface, with one teal accent for what is yours or needs you.
colors:
  bg: "#f3f4f1"
  surface: "#ffffff"
  sunk: "#f7f8f6"
  thumb: "#e7ece9"
  line: "#d6dbd8"
  line-strong: "#a3adaa"
  ink: "#1b2024"
  muted: "#5c646b"
  accent: "#0f6b6a"
  accent-ink: "#0b5352"
  accent-soft: "#ddeeec"
  on-accent: "#ffffff"
  ok: "#2e6b45"
  warn: "#7a5a12"
  err: "#a0302a"
  note-bg: "#fbf6e8"
  note-line: "#e6d9b6"
  add: "#e3f1e6"
  del: "#f7e0dc"
  night-bg: "#15181b"
  night-surface: "#1e2327"
  night-sunk: "#1a1f23"
  night-thumb: "#262d32"
  night-line: "#343c42"
  night-line-strong: "#5a656c"
  night-ink: "#e7ebe9"
  night-muted: "#9aa3a8"
  night-accent: "#23807e"
  night-accent-ink: "#8ed8d6"
  night-accent-soft: "#1d3b3b"
  night-ok: "#6cc08c"
  night-warn: "#e0b65a"
  night-err: "#f0867e"
  night-note-bg: "#33301f"
  night-note-line: "#5a5030"
  night-add: "#1f3a27"
  night-del: "#3d2421"
typography:
  body:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
    fontFeature: "tabular-nums"
  display:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "15px"
    fontWeight: 600
  hero:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "30px"
    fontWeight: 600
    lineHeight: 1.3
  ui:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.55
  label:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "12.5px"
    fontWeight: 600
  mono:
    fontFamily: "'JetBrains Mono', ui-monospace, Consolas, monospace"
    fontSize: "12.5px"
    fontWeight: 400
    lineHeight: 1.6
  mono-code:
    fontFamily: "'JetBrains Mono', ui-monospace, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.6
  mono-meta:
    fontFamily: "'JetBrains Mono', ui-monospace, Consolas, monospace"
    fontSize: "11.5px"
    fontWeight: 500
rounded:
  s: "6px"
  m: "8px"
  l: "10px"
  xl: "12px"
spacing:
  h-top: "48px"
  h-bar: "40px"
  h-row: "34px"
  h-button: "32px"
  h-small-button: "28px"
  h-row-button: "24px"
  h-action-content: "380px"
  diff-line: "22px"
  gutter: "52px"
  measure: "820px"
components:
  button:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    typography: "{typography.ui}"
    rounded: "{rounded.m}"
    height: "{spacing.h-button}"
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.on-accent}"
    typography: "{typography.ui}"
    rounded: "{rounded.m}"
    height: "{spacing.h-button}"
  button-small:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.m}"
    height: "{spacing.h-small-button}"
  composer:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.xl}"
    width: "{spacing.measure}"
  permission-request:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.xl}"
  card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.l}"
  well:
    backgroundColor: "{colors.sunk}"
    textColor: "{colors.ink}"
    typography: "{typography.mono}"
    rounded: "{rounded.s}"
  notice:
    backgroundColor: "{colors.note-bg}"
    textColor: "{colors.ink}"
    rounded: "{rounded.m}"
  diff-add:
    backgroundColor: "{colors.add}"
    textColor: "{colors.ink}"
    typography: "{typography.mono-code}"
  diff-del:
    backgroundColor: "{colors.del}"
    textColor: "{colors.ink}"
    typography: "{typography.mono-code}"
---

# Design System: Yaca

The source of truth is `src/ui/tokens.css`. This file describes it; if they
disagree, the code is current and this file is stale. Components never carry a
raw colour: they read tokens, and the theme changes the tokens.

## Overview

**Creative North Star: "Graphite and teal."** A quiet working surface for a tool
that stays open all day: a cool grey ground, white panels with a hairline border,
one teal accent. The interface is a pure function of the core's event stream and
never lies about state. Nothing decorates; everything on screen is a fact about
the session.

Density is a principle. The feed is 14px, mono output 12–12.5px, a diff line 22px
(20px in compact mode). Motion is limited to state pulses: a running tool pulses,
a live mark sends out a ring, and `prefers-reduced-motion` stops the pulse.

Two editions with identical token names: light (default) and dark. Components do
not know which one is active. In code the theme values are `paper` (light),
`night` (dark) and `system`; these are legacy names from the earlier paper-and-red
system and stay as stored values.

## Colors

One accent, one neutral family, three semantic hues. Hue carries meaning only
where it is the sole carrier of that meaning's *kind*; everything else is
graphite.

### Accent
- **Teal** (`accent`, `accent-ink`, `accent-soft`): what is yours or needs you:
  the primary button, an open permission request, the active chat, focus rings,
  text selection (28% mix). `on-accent` is the text on an accent fill.
  In dark the accent fill is `#23807e` and the readable teal text is `#8ed8d6`.

### Neutrals (cool graphite)
- `bg` the window ground; `surface` a raised panel on it; `sunk` a well inside a
  panel (output, code, inline code); `thumb` image and attachment placeholders.
- `line` hairline borders; `line-strong` borders that must read (scrollbars,
  inputs).
- `ink` body text; `muted` secondary text, 5.9:1 on `bg`, so it may carry real
  text and not just captions.

### Semantic
- `ok` green, `warn` amber, `err` muted brick red. `note-bg`/`note-line` is the
  warm paper of a notice bar. `add`/`del` wash added and removed diff lines.

### Context window parts
`--ctx-messages`, `-system`, `-tools`, `-mcp-tools`, `-mcp-notes`,
`-instructions`, `-memory`, `-skills`: the one place hue names a category.
Eight segments sit side by side in a bar four pixels wide, so only hue tells
neighbours apart. The legend under the bar names every one, so hue is never the
only carrier.

### Syntax
Graphite with teal and amber, nothing louder: keywords and tags in `accent-ink`,
strings and numbers in `warn`, types, operators and properties in `muted`,
functions in `ink`, comments a step lighter than `muted`. Class mapping lives
in `src/ui/syntax.css`.

### Named Rules
**One accent.** Teal is the only decorative hue. A second accent, or any
multi-colour treatment, is rejected.
**Failures are brick, not red.** `err` is muted on purpose; the interface is
not shouting at someone who is working.
**No raw hex in components.** Use a token. A value that has no token means the
token is missing; add it to both editions.

## Typography

- **Sans:** Onest for every voice (`--sans`; `--display` is the same family, heavier).
- **Mono:** JetBrains Mono for code, output, paths and figures (`--mono`).
- Body is 14px/1.5 with tabular figures. Feed and menu text is 13px/1.55. Titles
  are 15px at 600. The home hero is 30px at 600. Mono: 12.5px in blocks, 12px in
  code and diff, 11.5px for meta (keys, durations, counts).
- Font weights stay between 400 and 600. Numbers are tabular everywhere.

## Layout

A grid shell: sidebar (projects and sessions), then the session window; the
sidebar can be hidden, and the window stays pinned to the `1fr` track. The window
is a grid of top bar (48px), feed (`1fr`), then notification bars and the composer
(`auto` rows, so a bar appearing does not move the feed).

The conversation column is `--measure`, 820px: messages, notices, the composer
and its row share one edge. Row height is 34px, bars 40px, buttons 32px (28px
small, 24px inside a row). An opened action row scrolls past 380px. The diff
gutter is 52px with line numbers, 14px without (`showLineNumbers` off).

Slide-over panels (processes, file diff, checkpoints, providers, context, browser)
sit beside the feed, and lie over it when the window is too narrow to seat them.
The window is frameless on Windows with its own title bar.

## Elevation & Depth

Flat by default: depth is a hairline border plus the step from `bg` to `surface`.
Two shadows only: `--shadow-pop` for menus and popovers, `--shadow-panel` for the
edge of a panel lying over the feed. Both are tinted from `ink`, stronger in dark.

## Shapes

Four radii: 6px for what is set into a panel (wells, chips, keycaps), 8px for what
can be pressed (buttons, rows, menus), 10px for a card, 12px for the composer and
a permission request. Scrollbars are 10px with an 8px-rounded thumb.

## Components

- **Buttons.** One `.btn`: `surface` fill, `ink` text, `line` border; hover goes
  to `sunk`. The primary button (`.btn-accent`) is the only accent fill. There is a single button
  across the window; the permission block is set apart by its border, not by a
  different button.
- **Human message.** A raised surface bubble in the feed; actions appear on hover.
- **Permission request.** A `surface` card with an accent border and numbered
  answers. There is one confirmation, before the write.
- **Action group and rows.** Collapsed rows of 34px; a pulsing mark while running,
  a tick when done. An opened row shows its content in a `sunk` well.
- **Wells.** Output, diff and inline code sit on `sunk`, mono, 6px radius.
- **Diff.** 22px lines (20px compact), `add`/`del` washes, 52px gutter.
- **Notices and bars.** `note-bg` with `note-line` for warnings; errors in `err`.
- **Composer.** `surface`, 12px radius, mode, model and effort menus, and the
  status ring of the context window; the ring opens a 340px context panel.
- **Menus.** `surface`, `shadow-pop`, 10px radius; the model list is 360px with a
  search field and scrolls at a fixed height.
- **Focus.** A 2px accent outline with 2px offset on everything interactive.
- **Preferences** applied as attributes on `<html>`: `data-theme="dark"` for
  night, `data-compact="on"`, `data-line-numbers="off"`.

## Do's and Don'ts

### Do
- Read tokens; add a token to both editions when one is missing.
- Keep teal for what is yours or needs you.
- Name every colour-coded thing in text as well as hue.
- Keep density: the feed is for scanning.
- Route every string through i18n keys; layouts must survive German and Russian
  lengths.

### Don't
- Don't add a second accent hue or decorative gradients.
- Don't animate anything except state pulses.
- Don't use pure black or raw hex in components.
- Don't rely on hue alone for meaning, except in the context bar with its legend.
- Don't add a control that does nothing.
