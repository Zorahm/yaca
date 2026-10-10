---
name: Yaca
description: A desktop coding agent whose session window reads as a galley proof, with one red pencil.
colors:
  paper: "#f3eee2"
  paper-raised: "#fbf8f1"
  paper-sunken: "#e8e1d2"
  line: "#d6ccba"
  line-strong: "#857a69"
  ink: "#1b1a17"
  ink-muted: "#5e564a"
  red: "#c8321f"
  red-ink: "#ab2917"
  green: "#2f6b45"
  rail-fill: "#e8e1d2"
  rail-fill-deep: "#dcd3c0"
  rail-line: "#cdc2ad"
  on-rail-muted: "#5e564a"
  syntax-literal: "#7a4a14"
  syntax-comment: "#645c4e"
  night-paper: "#16130f"
  night-paper-raised: "#211d18"
  night-paper-sunken: "#0d0b09"
  night-line: "#3a332a"
  night-line-strong: "#857a6b"
  night-ink: "#f2ece2"
  night-ink-muted: "#a89d8c"
  night-red: "#ff5a3c"
  night-red-ink: "#ff7a61"
  night-green: "#79b98c"
  night-rail-fill: "#1b1814"
  night-rail-fill-deep: "#2a251f"
  night-rail-line: "#3a332a"
  night-on-rail-muted: "#a89d8c"
  night-syntax-literal: "#d9b27c"
  night-syntax-comment: "#9a8f7d"
typography:
  human:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "15px"
    fontWeight: 500
    lineHeight: 1.5
  title:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "15px"
    fontWeight: 600
    lineHeight: 1.4
  heading:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "14px"
    fontWeight: 700
    letterSpacing: "0"
  body:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "13.5px"
    fontWeight: 400
    lineHeight: 1.6
    fontFeature: "tabular-nums"
  ui:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "13px"
    fontWeight: 400
  small:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "12.5px"
    fontWeight: 400
  label:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "12px"
    fontWeight: 600
  caption:
    fontFamily: "Onest, system-ui, -apple-system, 'Segoe UI', sans-serif"
    fontSize: "11.5px"
    fontWeight: 400
    lineHeight: "21px"
  mono:
    fontFamily: "'JetBrains Mono', ui-monospace, Consolas, monospace"
    fontSize: "12.5px"
    fontWeight: 400
  mono-diff:
    fontFamily: "'JetBrains Mono', ui-monospace, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: "22px"
  mono-output:
    fontFamily: "'JetBrains Mono', ui-monospace, Consolas, monospace"
    fontSize: "11.5px"
    fontWeight: 400
    lineHeight: "19px"
  mono-figures:
    fontFamily: "'JetBrains Mono', ui-monospace, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 500
  mono-meta:
    fontFamily: "'JetBrains Mono', ui-monospace, Consolas, monospace"
    fontSize: "10.5px"
    fontWeight: 400
rounded:
  s: "3px"
  m: "6px"
spacing:
  h-top: "44px"
  h-bar: "40px"
  h-row: "32px"
  h-button: "30px"
  h-small-button: "28px"
  h-row-button: "24px"
  diff-line: "22px"
  gutter: "52px"
  route-x: "30px"
  route-gutter: "56px"
  feed-inset: "18px"
  block-gap: "7px"
  human-lead: "30px"
components:
  button:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink}"
    typography: "{typography.ui}"
    rounded: "{rounded.m}"
    padding: "0 14px"
    height: "{spacing.h-button}"
  button-hover:
    backgroundColor: "{colors.paper-sunken}"
    textColor: "{colors.ink}"
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.paper}"
    typography: "{typography.ui}"
    rounded: "{rounded.m}"
    padding: "0 14px"
    height: "{spacing.h-button}"
  button-small:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink}"
    typography: "{typography.small}"
    rounded: "{rounded.m}"
    padding: "0 10px"
    height: "{spacing.h-small-button}"
  button-row:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.m}"
    padding: "0 8px"
    height: "{spacing.h-row-button}"
  send:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.paper}"
    rounded: "{rounded.m}"
    size: "{spacing.h-small-button}"
  send-disabled:
    backgroundColor: "{colors.paper-sunken}"
    textColor: "{colors.ink-muted}"
  send-stop:
    backgroundColor: "{colors.paper-sunken}"
    textColor: "{colors.ink}"
  send-stop-hover:
    backgroundColor: "{colors.rail-fill-deep}"
    textColor: "{colors.ink}"
  keycap:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink-muted}"
    typography: "{typography.mono-meta}"
    rounded: "{rounded.s}"
    padding: "0 4px"
  chip-attachment:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink-muted}"
    rounded: "{rounded.s}"
    padding: "1px 7px"
  chip-project:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink-muted}"
    rounded: "{rounded.s}"
    padding: "0 6px"
  human-message:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink}"
    typography: "{typography.human}"
    rounded: "{rounded.m}"
    padding: "9px 13px"
  human-action:
    textColor: "{colors.ink-muted}"
    rounded: "{rounded.s}"
    size: "24px"
  human-action-hover:
    backgroundColor: "{colors.paper-sunken}"
    textColor: "{colors.ink}"
  composer:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.m}"
    padding: "10px 12px 8px"
    width: "820px"
  permission-request:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink}"
    typography: "{typography.title}"
    rounded: "{rounded.m}"
    padding: "14px 16px"
  permission-detail:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink}"
    typography: "{typography.mono}"
    rounded: "{rounded.s}"
    padding: "10px 12px"
  rollback-confirm:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.m}"
    padding: "12px 14px"
  error-block:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.red-ink}"
    rounded: "{rounded.m}"
    padding: "12px 14px"
  summary-box:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink}"
    typography: "{typography.ui}"
    rounded: "{rounded.s}"
    padding: "12px 14px"
  action-row:
    textColor: "{colors.ink}"
    typography: "{typography.ui}"
    padding: "0 8px 0 5px"
    height: "{spacing.h-row}"
  action-row-open:
    backgroundColor: "{colors.paper-raised}"
  output:
    backgroundColor: "{colors.paper-sunken}"
    textColor: "{colors.ink}"
    typography: "{typography.mono-output}"
    rounded: "{rounded.s}"
    padding: "8px 10px"
  diff:
    backgroundColor: "{colors.paper-sunken}"
    textColor: "{colors.ink}"
    typography: "{typography.mono-diff}"
    rounded: "{rounded.s}"
  code-inline:
    backgroundColor: "{colors.paper-sunken}"
    textColor: "{colors.ink}"
    typography: "{typography.mono}"
    rounded: "{rounded.s}"
    padding: "1px 4px"
  top-bar:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    typography: "{typography.heading}"
    padding: "0 10px"
    height: "{spacing.h-top}"
  notification-bar:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink}"
    typography: "{typography.small}"
    padding: "10px 18px"
    height: "{spacing.h-bar}"
  sidebar:
    backgroundColor: "{colors.rail-fill}"
    textColor: "{colors.ink}"
    width: "272px"
  sidebar-row:
    textColor: "{colors.ink}"
    typography: "{typography.ui}"
    rounded: "{rounded.m}"
    padding: "0 10px"
    height: "{spacing.h-small-button}"
  sidebar-row-hover:
    backgroundColor: "{colors.rail-fill-deep}"
  session-row-current:
    backgroundColor: "{colors.rail-fill-deep}"
    textColor: "{colors.ink}"
    rounded: "{rounded.m}"
  menu:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink}"
    typography: "{typography.ui}"
    rounded: "{rounded.m}"
    padding: "6px"
  menu-item-hover:
    backgroundColor: "{colors.paper-sunken}"
    textColor: "{colors.ink}"
  panel:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    typography: "{typography.heading}"
    width: "55%"
  panel-row:
    textColor: "{colors.ink}"
    padding: "0 18px"
    height: "{spacing.h-row}"
  panel-row-selected:
    backgroundColor: "{colors.paper-raised}"
    textColor: "{colors.ink}"
  field:
    backgroundColor: "{colors.paper-sunken}"
    textColor: "{colors.ink}"
    typography: "{typography.small}"
    rounded: "{rounded.s}"
    padding: "0 10px"
    height: "{spacing.h-small-button}"
  segment:
    textColor: "{colors.ink-muted}"
    typography: "{typography.mono}"
    rounded: "{rounded.s}"
    padding: "0 14px"
    height: "{spacing.h-small-button}"
  segment-pressed:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.paper}"
---

# Design System: Yaca

## Overview

**Creative North Star: "The Proof"**

The session window is a galley proof (Корректура): one column of type on warm paper with a ruled margin down its left, and a corrector's red pencil. What the agent did is type on the page: lines of report with a small grey mark in the margin. What the person said stands opposite it, as a slip of lighter paper at the right edge of the column, set heavier and a step larger so a long session can be scanned by its questions. The place where the agent waits for an answer is the thing ruled in red.

The system refuses the category default it replaced: a stack of rounded cards on a dark grey ground with one accent spent on everything. Two earlier worlds were rejected by the user as carrying too many colours; restraint in hue is a standing preference and is recorded below as a rule, not a mood.

Density is for an all-day tool: 13.5px body, 32px rows, 22px diff lines, a 44px top bar. Motion is limited to state: a pulse, a spinning arc, a ring going out from the mark that is live. There are no transitions; a panel appears without animation.

**Key Characteristics:**
- One column of type with a 1px margin rule and 14px marks on it; the person's messages are slips at its right edge.
- Warm paper in three steps, ink in two, one red, one green.
- The primary action is ink-filled, never red; nothing turns red under the pointer.
- One sans for every voice; mono only for code and measurements.
- Two radii, 3px and 6px. No capsules.
- Flat. A shadow exists only under something that lies over the page.
- Two editions under the same token names: Paper (light, the default) and Night.

**Scope of this record.** Designed in this build: the session window (sidebar, top bar, feed, notification bars, composer, menus) and the slide-over panels (file, processes, checkpoints, providers). The home screen is recoloured through the tokens only; it inherits the palette, type and radii and is not yet designed, so no rule in this file was derived from it. The Night edition is fully defined under `data-theme="dark"`, but nothing in the build sets that attribute yet: the switch arrives with the settings window.

## Colors

Warm paper, black type, one red pencil, green for what was added. Values live in the frontmatter; the names are the build's own (`--paper`, `--ink`, `--red`, `--green`). Night uses the same names with other values, recorded here as `night-*`.

### Primary
- **Ink** (`ink`): body type, the primary button fill, the send button, the pressed segment, the focus ring, the caret, the busy dot, the word "running". Ink does the work an accent does elsewhere: the thing you press is black.
- **Red Pencil** (`red-ink`): the pencil as type or as a rule. The border of an open permission request and of the rollback confirmation; a failure's title and its cross in the margin; the failed verb in an action row and the failed note in a group's line; failed command output; the dot on a group that holds a failure; removed lines and `-` counts; the rejected-file badge; the status ring while the agent waits or has failed; the mode chip while permission bypass is on; a load error in the sidebar or a panel; the state of a failed process; "no key" and the list of problems in the provider form.
- **Red Pencil, fill** (`red`): the same pencil as a small fill, in two places only: the margin mark of an open request and the 6px dot of a chat waiting for an answer. It never carries text and is never set as text; the fill does not reach 4.5:1 as type, `red-ink` does.

### Secondary
- **Proof Green** (`green`): one job, added lines: the `+` sign and ground of an added diff line, `+N` counts, the created-file badge.

### Neutral
- **Paper** (`paper`): the sheet: feed ground, top bar, composer surround, and the frame of a slide-over panel, which is the same sheet as the window.
- **Raised Slip** (`paper-raised`): what lies on the sheet and is lighter than it: the person's message, buttons, composer, an opened action row, a selected row in a panel, the permission request, notification bars, menus.
- **Sunken Paper** (`paper-sunken`): wells: command output, diff ground, inline code, a form field in a panel; and the hover of a button, a menu item or a small icon action.
- **Hairline** (`line`): 1px rules between rows, dividers, quiet borders, the border of the person's message.
- **Strong Hairline** (`line-strong`): borders of things you operate (buttons, composer, fields, menu), the left edge of a panel, the scrollbar thumb, the idle status ring. A border, never text.
- **Muted Ink** (`ink-muted`): secondary type, margin marks, placeholders, folded lines of report, the tick of a finished task, the word "finished".
- **Rail** (`rail-fill`, `rail-fill-deep`, `rail-line`, `on-rail-muted`): the sidebar is the same paper one step darker. Hover takes one step deeper (`rail-fill-deep`); the current chat takes the same step plus a 1px `rail-line` inset and a heavier title.
- **Sepia and grey of the set page** (`syntax-literal`, `syntax-comment`): inside code only. Keywords, functions and tags are `ink`; types, operators and properties are `ink-muted`; string and number literals are sepia; comments are grey. The comment grey measures 5.70:1 on paper, 6.22:1 on the raised slip and 5.07:1 in a well (Night: 5.82, 5.26, 6.17). On a tinted diff line in Paper it is under 4.5:1 (4.51 and 4.43 at the 9% tint, 4.22 and 4.10 at 14%); that is open, not accepted.

Mixes, not tokens: the margin rule is `color-mix(in srgb, line-strong 40%, line)`; row hover is `ink` at 4%; the wash under an open panel is `ink` at 9%; diff grounds are the sign colour at 14% over the well (9% inside an action row, 26% for a changed word), with the text left in `ink`.

### Named Rules
**The Red Pencil Rule.** Red is never decoration and never the brand. It marks only what needs the human's eye: something waiting for a decision, something that failed or is missing, a removed line, a waiting chat, a safeguard switched off. A red thing on screen with no question, failure or loss behind it is a bug.

**The Dry Pointer Rule.** Nothing turns red on hover. Red reports what already went wrong or waits for an answer, not what the pointer is over. A destructive action is named by its label.

**The Two Pencils Rule.** Hue carries meaning in exactly two colours, red and green. Everything else is weight, size and position. The sepia in code is typesetting inside a code block, not a signal; it does not leave the block.

**The Ink Button Rule.** The primary button is ink on paper, and a pressed choice is ink-filled the same way. Pressing must never look like an alarm.

**The Red Is Also A Shape Rule.** No state rests on colour alone. A failure is a cross or a word, an open request is two rings with a live ring going out, a waiting status ring pulses, the current row is heavier.

## Typography

**Display Font:** Onest (the same family, heavier; `--display` is an alias of `--sans`)
**Body Font:** Onest (with system-ui, -apple-system, "Segoe UI", sans-serif); loaded at 400, 500, 600, 700
**Label/Mono Font:** JetBrains Mono (with ui-monospace, Consolas, monospace); loaded at 400, 500, 600

**Character:** One humanist sans speaks for the person, the agent and the chrome alike, so the voices differ by weight and size, the way a set page does. Mono appears only where characters must line up or are typed: code, paths, commands, protocol names, counts, timestamps.

### Hierarchy
- **Human** (500, 15px, 1.5): what the person asked. Max 68ch. The largest running text in the window.
- **Title** (600, 15px, 1.4): the question of a permission request, the header of the provider form. Balanced wrap in the request.
- **Heading** (700, 14px): the session title in the top bar, a panel's title, headings inside agent text. Letter-spacing 0.
- **Body** (400, 13.5px, 1.6): agent text. Max 74ch, `text-wrap: pretty`. The window's base is 13.5px/1.55 with tabular figures. The folded line of an action group, the task list and a provider's name are the same size.
- **UI** (400, 13px; 600 for a file name in a caption and for the primary button): buttons, action row captions, session titles, menu items, the compaction summary.
- **Small** (400, 12.5px): composer chips, notification bars, the folded reasoning line, form fields, the small button, a checkpoint's reason.
- **Label** (600, 12px, `ink-muted`): the caption of a form field and the head of a form's column. Sentence case, in the sans.
- **Caption** (400, 11.5px, 21px line in the feed): dividers, session excerpts, the state word of a process, the file badge; 600 for the day heading in the sidebar.
- **Mono** (400, 12.5px; 500 inside the permission detail and for a file path as a panel title): inline code, command targets, paths, the error detail, a process's command, a checkpoint's time, protocol names on a segment.
- **Mono Diff** (400, 12px, 22px line): diff lines. Every line is the same height by contract; 20px in compact mode.
- **Mono Output** (400, 11.5px, 19px line): command output, in the feed and in the process panel. The same size carries row meta in panels: a process's number and time, a provider's address, a model's id and figures.
- **Mono Figures** (500, 12px): `+`/`-` totals flush against the right edge.
- **Mono Meta** (400, 10.5px): keycaps, session timestamps, the open-chat count.

Reasoning, when opened, is `ink-muted` italic at 13px/1.6.

### Named Rules
**The One Voice Rule.** One sans for every voice. There is no display face; `--display` is Onest at 700.

**The Mono Measures Rule.** JetBrains Mono is for code and for things that are measured or typed: paths, commands, counts, sizes, keys. Never a costume for prose, names or labels: a state is a word, and a word is set in the sans.

**The Heavier Question Rule.** The person's message is one step larger and one step heavier than the agent's. Who is speaking is said by that weight, by the slip and by the mark in the margin; no speaker label is drawn. The names stay in the markup, visually hidden, for a screen reader, which sees none of the three.

## Layout

A two-column shell, `auto 1fr`: the sidebar at a fixed 272px on rail paper, and the session pinned to the second track so that hiding the sidebar leaves it the full width. The session is four rows: a 44px top bar, the feed, the notification bars (40px each, `auto`), the composer (`auto`), so a bar appearing never jerks the feed. The window is never narrower than 720px or shorter than 480px.

The feed is one centred column, max 856px including its margin, with an 18px inset each side. Its left carries the margin: a continuous 1px rule 30px in from the inset, and a 56px gutter before the text begins. Marks sit on the rule, centred on the first text line of their block, with paper under them so the rule stops at their edge. The agent's blocks start at the gutter; the person's message is pushed to the right edge of the column. At 900px and below the margin gives way: the rule moves to 10px and the gutter to 30px; the marks keep their place on it. That is the only breakpoint.

Rhythm is per block kind and lives inside each item's padding, because the feed is a virtual list with nowhere to keep a gap. Blocks of one agent turn sit tight (7px above and below); a human message opens a wide interval (30px above, 10px below); a permission request and an error take 12px either side; inside a block, parts are 6px apart. The composer is centred at max 820px.

A slide-over panel lies over the right of the window: 55% wide, never under 560px, from the bottom of the top bar to the bottom of the window (full height on the home screen, which has no top bar). Its head repeats the top bar's 44px and hairline; its content keeps an 18px side inset. One panel is open at a time.

Control heights are fixed and few: 30px button, 28px small button, sidebar row, field and segment, 24px in-row button and message action, 32px row in the feed and in a panel. An opened row's content stops at 380px and scrolls inside itself; an opened group never scrolls, the feed does.

## Elevation & Depth

Flat. Depth is three steps of paper (sunken, sheet, raised) and hairlines. The sidebar is a darker sheet beside the page, not a layer above it. Only two things lie over the page, and only they cast a shadow.

### Shadow Vocabulary
- **Pop** (`box-shadow: 0 8px 24px -6px rgb(27 26 23 / 0.22), 0 1px 2px rgb(27 26 23 / 0.12)`; Night: `0 10px 28px -6px rgb(0 0 0 / 0.6), 0 1px 2px rgb(0 0 0 / 0.5)`): under a menu and the jump-to-bottom button.
- **Panel** (`--shadow-panel`: `-18px 0 36px -22px rgb(27 26 23 / 0.45)`; Night: `-18px 0 36px -18px rgb(0 0 0 / 0.75)`): a soft shadow falling to the left of an open panel, beside its `line-strong` border.

An open panel also puts a wash over everything it makes unusable: `ink` at 9% across the whole window, sidebar included. The top bar is lifted above the wash and stays clean, because its buttons still work. Order from the back: page, jump button (5), menu (20), wash (29), panel (30), top bar (31).

The current chat's `inset 0 0 0 1px` in `rail-line` is a border drawn as a shadow so the row does not shift; it is not elevation.

### Named Rules
**The Floats Only Rule.** A shadow means "this is lying over the page and will go away". Nothing that sits in the flow of the feed or the frame has one.

**The Honest Wash Rule.** What cannot be used while a panel is open looks washed; what still works does not.

## Shapes

Two radii and no capsules. 3px is for what is set into the page or is small: inline code, output, diff, a changed word, keycaps, chips, badges, the permission detail, the summary box, fields, segments, small icon actions. 6px is for what can be pressed or holds something said or decided: buttons, composer, menus, sidebar rows, the person's message, the permission request, the rollback confirmation, the error block, the scrollbar thumb. A chip is a small slip with 3px corners. Marks and dots are circles.

Borders are 1px throughout. Opened lists are ruled, not boxed: a hairline above, a hairline below, hairlines between rows. A box (border on four sides) is reserved for a field you type in, a well of code or quoted text, a block that holds a decision or a failure, and the person's message.

Marks are drawn, not typed: one 14px box, one 1.5px stroke in `currentColor`, in a 19px circle of paper on the rule. Each kind is its own shape: an open ring for reading, a ring with a bar for the web, a filled dot for an edit (radius 2.75, 4 or 5.25 for under 10, under 100, or more changed lines), a chevron for a command, a ring with a filled centre where the person spoke, two rings where the agent waits, a short dash for a checkpoint or other divider, a cross for a failure. Plain prose, the task list and the plan leave no mark: a margin full of dots says nothing.

## Components

### Buttons
Flat slips of paper with a strong hairline; the one you are meant to press is black.
- **Shape:** gently squared (6px), 30px high, 14px side padding, 13px type.
- **Default:** `paper-raised` ground, 1px `line-strong` border, `ink` text. Hover: `paper-sunken` ground.
- **Primary:** `ink` ground and border, `paper` text, weight 600. Hover mixes the ink 84% toward paper. One per block.
- **Destructive:** the default button; on hover its border darkens to `ink`. Its label says what it destroys.
- **Sizes:** small (28px, 12.5px type, 10px padding) and in-row (24px, 12px type, 8px padding).
- **Disabled:** 40% opacity, `not-allowed`.
- **Focus:** a 2px `ink` outline at 2px offset on everything interactive. Ink, not red: focus is where you are, not what needs you. A text field shows focus as an `ink` border instead.
- **Send:** a 28px ink square with a 6px radius; `paper-sunken` with `ink-muted` when disabled. As the stop control it is `paper-sunken` with an `ink` glyph, and its ground steps to `rail-fill-deep` on hover.

### Keycaps, chips and badges
The digit and keys that answer a request sit inside the button: 10.5px mono, 1px `line` border, 3px radius, `paper` ground. On the primary button the keycap goes transparent with a `paper` border at 45% and `paper` text. An attached file is a chip: 12px mono in `ink-muted`, 1px `line` border, 3px radius, `paper-raised`; the project name in the top bar is the same slip in 12px sans. Composer chips (mode, model, effort) are borderless text: 12.5px `ink-muted`, `ink` on hover, `red-ink` while permission bypass is on. The state of a file in the file panel is a badge: a word in the sans at 11.5px, no capitals and no tracking, in a 3px outline at 40% of its own colour, `ink-muted` for modified or moved, `green` for created, `red-ink` for deleted.

### Human message (signature)
A slip of lighter paper at the right edge of the column: `paper-raised`, 1px `line` border, 6px radius, 9px/13px padding, text in Human at most 68ch wide. Attachments are chips above the text. It is the one block in the feed that is boxed without being a field, a well or a decision; the agent's text stays bare on the page. Its mark, the ring with a filled centre, stays on the rule at the left.

Under the slip, at its right edge, sit its actions: 24px squares with a 3px radius holding a 13px icon in `ink-muted`, 2px apart; hover is a `paper-sunken` ground with an `ink` icon. They appear when the message is hovered or one of them has focus, by opacity. Two work, copy and send again; copy shows a tick once done.

### Permission request (signature)
Ruled in red pencil: 1px `red-ink` border, 6px radius, `paper-raised` ground, 14px/16px padding. Four tiers, always in this order: the question (Title), the target in `ink-muted` 13px, the detail in mono 500 inside a hairline well that is never truncated, and the answers. There are three answers. One refusal stands alone at the left; its label ends in an ellipsis because it opens a second step, a field for the reason (Enter in the empty field refuses at once) above the scopes for a standing refusal. The two consents are one group at the right, the standing consent and then the ink-filled primary at the far edge; when the row does not fit, the consent group wraps as a whole, so a consent never lands beside the refusal. Each button prints the key that presses it (1, 2, 3). Esc refuses at once and is not printed on the refusal button, because that button does something else. Its margin mark is the two red rings with a live ring going out; once answered the mark goes grey.

The rollback confirmation is drawn the same way (red rule, 6px radius, 12px/14px padding, a 600 13.5px title, the file list in mono on a 20px line up to 240px) because it also waits on a decision. It appears where it was asked: under a checkpoint divider in the feed, or inside the checkpoint's row in the panel.

### Margin and marks (signature)
See Layout and Shapes. Marks are `ink-muted`; the person's mark is `ink`; a failure is `red-ink`; an open request is `red`. A refused, denied or stopped row keeps its mark at 45% opacity. A group that holds a failure but is not itself one carries a 6px `red-ink` dot at the mark's corner. A live mark sends out a ring (1.5s, `cubic-bezier(0.16, 1, 0.3, 1)`); with reduced motion the ring stands still at 50%.

### Action group and rows
A folded group is a line in the flow: 13.5px `ink-muted`, darkening to `ink` on hover, totals in Mono Figures at the right. Opened, it is a ruled list. A row is 32px with its mark, a 13px caption with the file name at 600, and hover as a 4% ink wash. An opened row takes `paper-raised` and hangs its content 28px in, under the caption. A failed row colours its verb `red-ink`, not the row.

### Wells: output, diff, inline code
`paper-sunken` ground, 1px `line` border, 3px radius. Output is Mono Output with 8px/10px padding and a 380px ceiling; failed output is `red-ink`. Inline code is `ink` on a sunken patch, never coloured. A diff is a grid of equal-height lines (gutter 52px, or 14px without numbers; sign 16px; text): added and removed lines tinted as described under Colors, context lines in `ink-muted`, a changed word on a denser tint with 3px corners, hunk headers as a `paper-raised` band between hairlines in 11.5px mono. Inside an opened row the diff shares the 380px ceiling and scrolls within itself; in the file panel it takes the panel's height.

### Error block
A failure is red type and a red cross in the margin, not a second red box: a hairline box (1px `line`, 6px radius, `paper-raised`, 12px/14px padding) with a 600 13.5px title in `red-ink`, the detail in 12.5px mono `ink`, and a row of default buttons.

### Dividers and the compaction summary
A checkpoint, a compaction and other turning points are a caption-sized line in `ink-muted` with a hairline running out to the right; their actions are the same grey text, `ink` and underlined on hover. The margin mark is a short dash. A compaction's summary opens beneath as a hairline box: 3px radius, `paper-raised`, 12px/14px padding, 13px/1.55.

### Notification bars
Stacked above the composer, 40px each: `paper-raised`, a hairline above, 12.5px type, a muted note that truncates, counts in mono, a small button for the action and a muted cross that goes `ink` on hover.

### Composer
A raised slip centred at max 820px: 1px `line-strong` border that turns `ink` on focus-within, 6px radius, 13.5px/1.5 text, growing to 264px. Under the field, a row of text chips, the status ring, and the send button.

### Status ring
A 16px ring, 1.5px stroke, the only work indicator: its fill is context used. Idle is `line-strong`; busy is `ink` with a quarter arc spinning (1.4s linear); waiting is `red-ink` with a slow pulse (2s); error is `red-ink`. A track at 35% opacity sits under each.

### Top bar
44px, `paper`, a hairline beneath. Session title in Heading. Icon buttons are 28px squares with a 6px radius: `ink-muted` at rest, `ink` on hover, `paper-raised` with a `line-strong` border when pressed, 35% when disabled. It stays above the wash of an open panel.

### Sidebar
272px of rail paper with a `rail-line` edge. Every non-session row is 28px with a 10px gap between icon and label; the new-chat row is outlined (`on-rail-muted` border on `paper-raised`), not filled. Project rows are weight 500; session rows are indented 14px and show a 13px title, a 10.5px mono timestamp and an 11.5px excerpt. A 6px dot marks an open chat: a hollow grey ring when idle, a pulsing `ink` dot when the agent works, a plain `red` dot, with no halo, when it waits for an answer. Secondary actions come out on hover and on focus by opacity, never by `display`, so the keyboard still reaches them.

### Menus
`paper-raised`, 1px `line-strong` border, 6px radius, 6px padding, the Pop shadow, min 260px. A menu whose items carry a sentence beneath the name is at least 340px and opens from the left edge of its chip. Items are 13px with 7px/10px padding; hover and the keyboard-selected item take `paper-sunken`. A destructive item looks like any other item and is told apart by its label.

### Slide-over panels
The same sheet as the window, not a lighter card: `paper` ground, a 1px `line-strong` left border, the Panel shadow, and the wash over the window behind it. The head is 44px with the title in Heading (a file path is set in 12.5px mono 500) and a hairline beneath.

- **Rows** (processes, checkpoints) use the feed's two steps: 32px high, 18px side padding, hover as a 4% ink wash, the selected or opened row on `paper-raised` with its leading text heavier (600 for a process's command, 500 for a checkpoint's time). No coloured bar at the edge. Provider rows are taller (7px/18px padding) with a 13.5px name over a line of 11.5px mono meta, and take the same hover.
- **Process state** is a word in the sans at 11.5px: `ink` while running, `ink-muted` when finished, `red-ink` only when failed. The command is 12.5px mono; number and elapsed time are 11.5px mono in `ink-muted`. Output of the selected process is a full-width well in Mono Output under a 40px head.
- **Provider form:** sections 16px apart inside 14px/18px padding, under a header in Title. A field's caption is Label, with no tracking. A field is a well: `paper-sunken`, 1px `line-strong` border, 3px radius, 28px high, 12.5px type, mono only when its content is a typed value; focus is an `ink` border; a read-only field is `ink-muted`. A choice between two protocols is a pair of segments, 28px with a 3px radius and a `line-strong` border, protocol names in 12.5px mono; hover is `paper-sunken`, the pressed one is ink-filled with `paper` text. Where a key comes from is a 12px word: `ink` when set, `red-ink` only when there is none. A model row is a raised slip (1px `line`, 6px radius, 6px/10px padding) on a fixed grid. Problems are listed in 12.5px mono `red-ink`. A note is a raised slip with a `line-strong` border in 12.5px `ink-muted`. The form ends with its buttons at the right.
- **Empty and error:** an empty panel is 13px `ink-muted` with 24px/18px padding; a load error is 12.5px mono `red-ink`.

## Do's and Don'ts

### Do:
- **Do** set what the agent did as type on paper. Reach for a box only when the block is a field, a well, holds a decision or a failure, or is the person's message.
- **Do** put the person's message on a raised slip at the right edge of the column, and keep its actions under it, hidden until hover or focus.
- **Do** give a block that did something a mark on the margin rule, drawn in the 14px box with a 1.5px stroke, and give it its own shape.
- **Do** make the primary action ink-filled (`ink` ground, `paper` text, 600) and place it at the right of its group; fill a pressed choice with ink the same way.
- **Do** use `red-ink` when red is type or a rule, and `red` only as a small fill with no text on it.
- **Do** name a destructive action in its label and leave its colour alone.
- **Do** say a state with a word in the sans, and pair every red or state colour with a shape, a weight or a motion that says the same thing.
- **Do** use the same two steps for any list row: a 4% ink wash on hover, `paper-raised` and more weight when selected. In the sidebar the steps are the rail's own.
- **Do** build a panel on the window's own `paper`, and wash what it makes unusable while leaving what still works clean.
- **Do** reference tokens by name so Paper and Night stay one system; derive tints with `color-mix` from `ink`, `red-ink` and `green` rather than adding tokens.
- **Do** keep hidden actions reachable: reveal with opacity, and keep the 2px ink focus ring.
- **Do** honour `prefers-reduced-motion`: pulses rest at a fixed opacity, the ring and the arc stand still.

### Don't:
- **Don't** use red for emphasis, branding, links, selection, focus, hover or a primary button. Red means the human's eye is needed.
- **Don't** add a third signalling hue: no blue for links or info, no amber for warnings, no coloured tool categories. Links are underlined ink.
- **Don't** spend green on success, completion, "running" or "done". Green means added.
- **Don't** turn the agent's blocks into a stack of rounded cards. One slip per question is the only card the feed has.
- **Don't** rule a block in red unless it is waiting on a decision. A failure is red type and a red cross, not a second red box.
- **Don't** round anything into a capsule. The radii are 3px and 6px.
- **Don't** draw a speaker label above a turn, or mark the selected row with a coloured bar down its edge.
- **Don't** put a shadow on anything in the flow of the page, or make a panel a lighter sheet than the window.
- **Don't** set prose, names, states or field captions in mono, or bring in a second sans or a display face.
- **Don't** colour inline code or identifiers; they are ink on a sunken patch.
- **Don't** add transitions or decorative motion. Motion reports state and nothing else.
- **Don't** treat the home screen as precedent for anything; it is recoloured, not designed.
